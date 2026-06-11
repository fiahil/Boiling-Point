//! The seat driver: one connection, one view, one brain, one game.
//!
//! A pure decode → audit → update-view → answer-frames loop. The seat acts only
//! when the server says it owes a decision (a [`ServerMessage::DecisionFrame`])
//! and submits only actions drawn from that frame's enumerated legal set,
//! routed through the host policy (D5) and the latency-budget race (D6) with
//! per-game fallback accounting. Every inbound message passes the live
//! secret-boundary audit first, so every game doubles as a no-leak test.

use std::collections::HashSet;
use std::time::Duration;

use boiling_point_protocol::vocab::{Color, SpellKind};
use boiling_point_protocol::{CardId, ClientMessage, PlayerId, ServerMessage};

use crate::ClientError;
use crate::brain::{Answer, Brain, BudgetConfig, Resolution, WaveAction, decide_with_budget};
use crate::observe::{GameObservation, RoundObservation};
use crate::policy::{HostPolicy, Policy};
use crate::transport::Connection;
use crate::view::{FrameContext, SeatView, secret_audit};

/// Per-seat behaviour configuration (host-owned; brains never see it).
#[derive(Debug, Clone)]
pub struct SeatConfig {
    /// Who answers which decision kind (D5).
    pub policy: HostPolicy,
    /// The latency-budget derivation (D6).
    pub budget: BudgetConfig,
    /// Keep the observed-events transcript (agent seats).
    pub record_transcript: bool,
    /// Send a heartbeat when the connection has been quiet this long
    /// (`None` for in-process play, where no idle timeout exists).
    pub heartbeat_quiet: Option<Duration>,
    /// The emote palette the brain may draw table-talk from (emote ids are
    /// server content config; the operator supplies them — empty ⇒ silent).
    pub emote_palette: Vec<boiling_point_protocol::EmoteId>,
}

impl Default for SeatConfig {
    fn default() -> Self {
        SeatConfig {
            policy: HostPolicy::default(),
            budget: BudgetConfig::default(),
            record_transcript: false,
            heartbeat_quiet: None,
            emote_palette: Vec::new(),
        }
    }
}

/// What one seat's game produced: the balance observation plus the
/// decision/fallback accounting (D6's per-game fallback rate).
#[derive(Debug, Clone, Default)]
pub struct SeatOutcome {
    /// The per-game balance observation.
    pub observation: GameObservation,
    /// Decisions the seat answered (frames acted on).
    pub decisions: u32,
    /// Decisions answered by the fallback because the brain missed its budget.
    pub fallback_budget: u32,
    /// Decisions answered by the fallback because the brain's answer was
    /// outside the frame's legal set.
    pub fallback_illegal: u32,
    /// Errors the server sent this seat (a frame-driven seat expects none).
    pub errors: Vec<String>,
}

impl SeatOutcome {
    /// Total fallback commits (budget misses + illegal answers).
    pub fn fallbacks(&self) -> u32 {
        self.fallback_budget + self.fallback_illegal
    }
}

/// Play one complete game over `conn` (already entered), answering decision
/// frames through `policy`/`brain` with `fallback` as the timeliness floor.
pub async fn run_seat<C: Connection>(
    mut conn: C,
    me: PlayerId,
    my_color: Color,
    brain: &mut dyn Brain,
    fallback: &mut dyn Brain,
    cfg: &SeatConfig,
) -> Result<SeatOutcome, ClientError> {
    let mut view = SeatView::new(me, my_color);
    if cfg.record_transcript {
        view = view.with_transcript();
    }
    let mut out = SeatOutcome::default();
    let mut rounds: Vec<RoundObservation> = Vec::new();
    let mut current: Option<RoundObservation> = None;
    let mut acted: Option<(u8, u8)> = None;
    let mut pending_commit: Option<CardId> = None;
    let mut last_wave_all_passed = false;
    // Who passed in earlier waves of the open round (fold-to-safety candidates).
    let mut folded_this_round: HashSet<PlayerId> = HashSet::new();

    // Bank the open round's observation when a new round begins.
    fn roll_round(
        rounds: &mut Vec<RoundObservation>,
        current: &mut Option<RoundObservation>,
        n: u8,
    ) {
        match current {
            Some(r) if r.round_number == n => {}
            _ => {
                if let Some(done) = current.take() {
                    rounds.push(done);
                }
                *current = Some(RoundObservation::new(n));
            }
        }
    }

    loop {
        let msg = match cfg.heartbeat_quiet {
            Some(quiet) => match tokio::time::timeout(quiet, conn.recv()).await {
                Ok(msg) => msg,
                Err(_) => {
                    // Quiet line: keep the connection alive, then keep listening.
                    conn.send(&ClientMessage::Heartbeat).await?;
                    continue;
                }
            },
            None => conn.recv().await,
        };
        let Some(msg) = msg else { break };

        secret_audit(&msg)?;
        view.observe(&msg);

        match msg {
            ServerMessage::DecisionFrame {
                round_number,
                wave_number,
                timer_ms,
                decision,
            } => {
                // A refresh of an already-answered decision never re-acts.
                if acted == Some((round_number, wave_number)) {
                    continue;
                }
                acted = Some((round_number, wave_number));

                let frame = FrameContext {
                    round_number,
                    wave_number,
                    timer_ms,
                    decision,
                };
                let budget = cfg.budget.budget_for(&frame);
                let (answer, resolution) = match cfg.policy.route(&frame.decision) {
                    Policy::Scripted(scripted) if scripted.is_legal(&frame.decision) => {
                        (scripted, Resolution::Brain)
                    }
                    // An illegal script degrades exactly like an illegal answer.
                    Policy::Scripted(_) => {
                        let answer = fallback.decide(&view, &frame).await;
                        (answer, Resolution::FallbackIllegal)
                    }
                    Policy::Delegated => {
                        decide_with_budget(brain, fallback, &view, &frame, budget).await
                    }
                };
                out.decisions += 1;
                match resolution {
                    Resolution::Brain => {}
                    Resolution::FallbackBudget => out.fallback_budget += 1,
                    Resolution::FallbackIllegal => out.fallback_illegal += 1,
                }

                // Submit, then sync the view with our own action.
                for msg in answer.to_messages() {
                    conn.send(&msg).await?;
                }
                let Answer::WaveCommit { action, spell } = answer;
                match action {
                    WaveAction::Play { card, .. } => pending_commit = Some(card),
                    WaveAction::Pass => {
                        view.passed = true;
                        pending_commit = None;
                    }
                }
                if let Some(cast) = spell {
                    // Optimistic removal; the next YourHand refresh re-syncs.
                    view.remove_spell(cast.spell);
                }
                if let Some(emote) = brain.emote(&view, &cfg.emote_palette) {
                    conn.send(&ClientMessage::Emote { emote }).await?;
                }
            }
            ServerMessage::WaveOpened {
                round_number,
                wave_number,
                ..
            } => {
                roll_round(&mut rounds, &mut current, round_number);
                if wave_number == 1 {
                    last_wave_all_passed = false;
                    pending_commit = None;
                    folded_this_round.clear();
                }
            }
            ServerMessage::SpellCast { spell, .. } => {
                if let Some(r) = current.as_mut() {
                    r.spell_casts += 1;
                    if spell == SpellKind::Peek {
                        r.peek_casts += 1;
                    }
                }
            }
            ServerMessage::WaveResolved { played, passed, .. } => {
                if played.contains(&me)
                    && let Some(card) = pending_commit.take()
                {
                    view.ingredients.retain(|c| c.id != card);
                }
                folded_this_round.extend(passed.iter().copied());
                last_wave_all_passed = played.is_empty();
                if let Some(r) = current.as_mut() {
                    r.waves += 1;
                }
            }
            ServerMessage::ModifierRevealed {
                modifier,
                round_number,
            } => {
                roll_round(&mut rounds, &mut current, round_number);
                if let Some(r) = current.as_mut() {
                    r.modifier = Some(modifier);
                }
            }
            ServerMessage::Depile {
                reveals, exploded, ..
            } => {
                if let Some(r) = current.as_mut() {
                    r.cards_in_pot = reveals.len() as u32;
                    r.exploded = exploded;
                    r.ended_all_pass = !exploded && last_wave_all_passed;
                    if !exploded {
                        // Folding paid off: everyone who passed early watched
                        // the pot settle safely.
                        r.folded_safe = folded_this_round.iter().copied().collect();
                        r.folded_safe.sort_by_key(|p| p.0);
                    }
                }
            }
            ServerMessage::RoundScored {
                color_points,
                awards,
                ..
            } => {
                if let Some(r) = current.as_mut() {
                    let paid: u32 = awards
                        .iter()
                        .filter(|a| a.score > 0)
                        .map(|a| a.score as u32)
                        .sum();
                    let by_color: u32 = color_points.iter().map(|(_, p)| *p).sum();
                    r.pot_value = if paid > 0 { paid } else { by_color };
                }
            }
            ServerMessage::Explosion {
                pot_value,
                detonators,
                ..
            } => {
                if let Some(r) = current.as_mut() {
                    r.pot_value = pot_value;
                    r.exploded = true;
                    r.detonators = detonators;
                }
            }
            ServerMessage::Error { code, message } => {
                out.errors.push(format!("{code:?}: {message}"));
            }
            ServerMessage::GameOver {
                final_scores,
                winners,
            } => {
                if let Some(done) = current.take() {
                    rounds.push(done);
                }
                out.observation = GameObservation {
                    me: Some(view.me),
                    my_color: Some(view.my_color),
                    rounds: std::mem::take(&mut rounds),
                    winners,
                    final_scores: final_scores
                        .into_iter()
                        .map(|s| (s.player, s.score))
                        .collect(),
                    completed: true,
                };
                return Ok(out);
            }
            _ => {}
        }
    }

    // The stream ended before GameOver: report what was seen, uncompleted.
    if let Some(done) = current.take() {
        rounds.push(done);
    }
    out.observation = GameObservation {
        me: Some(view.me),
        my_color: Some(view.my_color),
        rounds,
        completed: false,
        ..GameObservation::default()
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use async_trait::async_trait;
    use boiling_point_protocol::frame::{PendingDecision, PlayableIngredient};
    use boiling_point_protocol::vocab::{HandIngredient, IngredientView};
    use uuid::Uuid;

    use crate::brain::Answer;
    use crate::view::FrameContext;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    /// A scripted inbound stream that captures everything the seat sends.
    struct MockConn {
        inbox: VecDeque<ServerMessage>,
        sent: tokio::sync::mpsc::UnboundedSender<ClientMessage>,
    }

    impl Connection for MockConn {
        async fn recv(&mut self) -> Option<ServerMessage> {
            self.inbox.pop_front()
        }
        async fn send(&mut self, msg: &ClientMessage) -> Result<(), ClientError> {
            let _ = self.sent.send(msg.clone());
            Ok(())
        }
    }

    /// A brain that counts how often it is consulted and always plays card 1.
    struct Counting(Arc<AtomicU32>);
    #[async_trait]
    impl Brain for Counting {
        fn name(&self) -> String {
            "counting".into()
        }
        async fn decide(&mut self, _v: &SeatView, _f: &FrameContext) -> Answer {
            self.0.fetch_add(1, Ordering::SeqCst);
            Answer::WaveCommit {
                action: WaveAction::Play {
                    card: CardId(1),
                    colorless: false,
                },
                spell: None,
            }
        }
    }

    fn frame_msg() -> ServerMessage {
        ServerMessage::DecisionFrame {
            round_number: 1,
            wave_number: 1,
            timer_ms: Some(10_000),
            decision: PendingDecision::WaveCommit {
                playable: vec![PlayableIngredient {
                    ingredient: HandIngredient {
                        id: CardId(1),
                        view: IngredientView {
                            color: Color::Ruby,
                            volatility: 2,
                            points: 1,
                        },
                    },
                    colorless_allowed: true,
                }],
                can_pass: true,
                spells: vec![],
            },
        }
    }

    async fn run_with_policy(policy: HostPolicy) -> (SeatOutcome, Vec<ClientMessage>, u32, u32) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let conn = MockConn {
            inbox: VecDeque::from([
                ServerMessage::WaveOpened {
                    round_number: 1,
                    wave_number: 1,
                    timer_ms: 10_000,
                    final_wave: false,
                },
                frame_msg(),
            ]),
            sent: tx,
        };
        let brain_calls = Arc::new(AtomicU32::new(0));
        let fallback_calls = Arc::new(AtomicU32::new(0));
        let mut brain = Counting(brain_calls.clone());
        let mut fallback = Counting(fallback_calls.clone());
        let cfg = SeatConfig {
            policy,
            ..SeatConfig::default()
        };
        let outcome = run_seat(conn, pid(1), Color::Ruby, &mut brain, &mut fallback, &cfg)
            .await
            .expect("seat runs");
        let mut sent = Vec::new();
        while let Ok(m) = rx.try_recv() {
            sent.push(m);
        }
        (
            outcome,
            sent,
            brain_calls.load(Ordering::SeqCst),
            fallback_calls.load(Ordering::SeqCst),
        )
    }

    /// Delegated frames go to the brain; its legal answer is submitted.
    #[tokio::test]
    async fn delegated_frames_consult_the_brain() {
        let (outcome, sent, brain_calls, fallback_calls) =
            run_with_policy(HostPolicy::default()).await;
        assert_eq!(brain_calls, 1);
        assert_eq!(fallback_calls, 0);
        assert_eq!(outcome.decisions, 1);
        assert_eq!(outcome.fallbacks(), 0);
        assert!(matches!(
            sent.as_slice(),
            [
                ClientMessage::CommitIngredient {
                    card: CardId(1),
                    colorless: false
                },
                ClientMessage::LockIn
            ]
        ));
    }

    /// A scripted decision is answered by the host: the brain never sees it
    /// (D5), and the scripted answer goes out verbatim.
    #[tokio::test]
    async fn scripted_frames_bypass_the_brain() {
        let policy = HostPolicy {
            wave_commit: Policy::Scripted(Answer::pass()),
        };
        let (outcome, sent, brain_calls, fallback_calls) = run_with_policy(policy).await;
        assert_eq!(brain_calls, 0, "the brain must never see a scripted kind");
        assert_eq!(fallback_calls, 0);
        assert_eq!(outcome.decisions, 1);
        assert!(matches!(
            sent.as_slice(),
            [ClientMessage::CommitPass, ClientMessage::LockIn]
        ));
    }

    /// An illegal script degrades to the fallback, counted as such.
    #[tokio::test]
    async fn illegal_script_falls_back() {
        let policy = HostPolicy {
            wave_commit: Policy::Scripted(Answer::WaveCommit {
                action: WaveAction::Play {
                    card: CardId(999),
                    colorless: false,
                },
                spell: None,
            }),
        };
        let (outcome, sent, brain_calls, fallback_calls) = run_with_policy(policy).await;
        assert_eq!(brain_calls, 0);
        assert_eq!(fallback_calls, 1);
        assert_eq!(outcome.fallback_illegal, 1);
        assert!(matches!(
            sent.first(),
            Some(ClientMessage::CommitIngredient { .. })
        ));
    }

    /// A frame refresh for an already-answered decision is not re-acted on.
    #[tokio::test]
    async fn refreshed_frames_do_not_double_act() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let conn = MockConn {
            inbox: VecDeque::from([frame_msg(), frame_msg()]),
            sent: tx,
        };
        let calls = Arc::new(AtomicU32::new(0));
        let mut brain = Counting(calls.clone());
        let mut fallback = Counting(Arc::new(AtomicU32::new(0)));
        let cfg = SeatConfig::default();
        let outcome = run_seat(conn, pid(1), Color::Ruby, &mut brain, &mut fallback, &cfg)
            .await
            .expect("seat runs");
        assert_eq!(outcome.decisions, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let mut sent = Vec::new();
        while let Ok(m) = rx.try_recv() {
            sent.push(m);
        }
        assert_eq!(sent.len(), 2, "one commit + one lock-in only: {sent:?}");
    }
}
