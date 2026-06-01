//! The transport-agnostic bot: a decode → update-model → ask-strategy → encode
//! loop that plays one complete game and reports what it observed.
//!
//! The loop consumes only [`ServerMessage`]s and emits only [`ClientMessage`]s, so
//! the bot is, by construction, a real client (Constitution I/II). It plays every
//! phase — join, deal, all five rounds of waves, any Deathmatch, through
//! `GameOver` — issuing only legal commits and passes. Every inbound message is
//! first run through [`secret_audit`]: because a bot must never receive a server
//! secret, each game is also a live test of the no-leak contract (D2).

use rand::rngs::StdRng;

use boiling_point_protocol::vocab::{Color, ModifierKind};
use boiling_point_protocol::{codec, ClientMessage, EmoteId, PlayerId, ServerMessage};

use crate::model::PlayerView;
use crate::strategy::{Strategy, WaveAction};
use crate::transport::BotConnection;
use crate::HarnessError;

/// What a bot observed in one round, derived purely from broadcast messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundObservation {
    /// 1-based round number.
    pub round_number: u8,
    /// Whether the round exploded.
    pub exploded: bool,
    /// The pot's scored value — lost by all on an explosion, paid out on a safe brew.
    pub pot_value: u32,
    /// Cards that ended up in the cauldron this round (the depile length).
    pub cards_in_pot: u32,
    /// Number of waves the round ran.
    pub waves: u32,
    /// The modifier revealed at the start of this round, if any.
    pub modifier: Option<ModifierKind>,
}

impl RoundObservation {
    fn new(round_number: u8) -> Self {
        RoundObservation {
            round_number,
            exploded: false,
            pot_value: 0,
            cards_in_pot: 0,
            waves: 0,
            modifier: None,
        }
    }
}

/// Everything one bot saw across a complete game — the unit the runner aggregates.
#[derive(Debug, Clone)]
pub struct GameObservation {
    /// The observing bot's player id (server-assigned over WebSocket).
    pub me: PlayerId,
    /// The observing bot's seat colour.
    pub my_color: Color,
    /// Per-round observations, in order.
    pub rounds: Vec<RoundObservation>,
    /// The game's winner(s).
    pub winners: Vec<PlayerId>,
    /// Final cumulative scores.
    pub final_scores: Vec<(PlayerId, i32)>,
    /// Whether the game actually reached `GameOver` (false ⇒ it ended abnormally).
    pub completed: bool,
}

/// Audit one inbound message for a leaked server secret.
///
/// The protocol permits the boiling point on exactly two messages — a private
/// [`ServerMessage::PeekResult`] and an *exploded* [`ServerMessage::Depile`]. Any
/// other message that carries a non-null `boiling_point` on the wire is a breach
/// of the secret-management contract. Mirrors the protocol crate's own invariant,
/// but enforced live against the stream a bot actually consumes.
pub fn secret_audit(msg: &ServerMessage) -> Result<(), HarnessError> {
    let permitted = matches!(
        msg,
        ServerMessage::PeekResult { .. } | ServerMessage::Depile { exploded: true, .. }
    );
    if permitted {
        return Ok(());
    }
    let json = codec::encode_json(msg)
        .map_err(|e| HarnessError::SecretLeak(format!("could not audit message: {e}")))?;
    if json.contains("\"boiling_point\"") && !json.contains("\"boiling_point\":null") {
        return Err(HarnessError::SecretLeak(format!(
            "a non-secret message disclosed the boiling point: {json}"
        )));
    }
    Ok(())
}

/// Play one complete game over `conn`, deciding each wave through `strategy` and
/// the seeded `rng`, and return the per-game observation.
///
/// `me`/`my_color` seed the bot's identity (a player always knows their own seat);
/// a `RoomJoined`, if the transport sends one, confirms them.
pub async fn run_bot<C: BotConnection>(
    mut conn: C,
    me: PlayerId,
    my_color: Color,
    strategy: &dyn Strategy,
    mut rng: StdRng,
    palette: &[EmoteId],
) -> Result<GameObservation, HarnessError> {
    let mut view = PlayerView::new(me, my_color);
    let mut rounds: Vec<RoundObservation> = Vec::new();
    let mut current: Option<RoundObservation> = None;
    let mut pending_commit: Option<boiling_point_protocol::CardId> = None;
    let mut winners: Vec<PlayerId> = Vec::new();
    let mut final_scores: Vec<(PlayerId, i32)> = Vec::new();
    let mut completed = false;

    // Roll over to a new round's observation, banking the previous one.
    let ensure_round =
        |rounds: &mut Vec<RoundObservation>, current: &mut Option<RoundObservation>, n: u8| {
            let fresh = match current {
                Some(r) if r.round_number == n => return,
                Some(_) => true,
                None => true,
            };
            if fresh {
                if let Some(done) = current.take() {
                    rounds.push(done);
                }
                *current = Some(RoundObservation::new(n));
            }
        };

    while let Some(msg) = conn.recv().await {
        secret_audit(&msg)?;
        match msg {
            ServerMessage::RoomJoined {
                your_player_id,
                your_color,
                players,
                ..
            } => {
                view.me = your_player_id;
                view.my_color = your_color;
                view.players = players;
            }
            ServerMessage::GameStarting { players, .. } => {
                view.players = players;
            }
            ServerMessage::YourHand { cards } => {
                view.begin_round(cards);
                pending_commit = None;
            }
            ServerMessage::ModifierRevealed {
                modifier,
                round_number,
            } => {
                view.active_modifiers.push(modifier);
                ensure_round(&mut rounds, &mut current, round_number);
                if let Some(r) = current.as_mut() {
                    r.modifier = Some(modifier);
                }
            }
            ServerMessage::WaveOpened {
                round_number,
                wave_number,
                ..
            } => {
                ensure_round(&mut rounds, &mut current, round_number);
                view.round_number = round_number;
                view.wave_number = wave_number;
                // Respond while still in the round — gated on the pass lockout, a
                // server-confirmed signal, NOT on hand emptiness. A Recall can return
                // a card to our hand server-side with no message disclosing it, so our
                // model hand can read empty while the server still expects us to act;
                // staying silent would stall the wave for its whole timer. If our
                // model hand is empty we cannot commit a card we cannot see, so we
                // pass — but we always lock in, so the wave closes promptly.
                if !view.passed {
                    let action = if view.hand.is_empty() {
                        WaveAction::Pass
                    } else {
                        strategy.decide(&view, &mut rng)
                    };
                    match action {
                        WaveAction::Play(card) => {
                            conn.send(ClientMessage::CommitCard { card }).await;
                            pending_commit = Some(card);
                        }
                        WaveAction::Pass => {
                            conn.send(ClientMessage::CommitPass).await;
                            view.passed = true;
                            pending_commit = None;
                        }
                    }
                    // Lock in to close the wave early in-process; over WebSocket the
                    // per-connection rate limit drops this and the wave closes on its
                    // timer — either way the committed action stands.
                    conn.send(ClientMessage::LockIn).await;
                    if let Some(emote) = strategy.emote(&view, palette, &mut rng) {
                        conn.send(ClientMessage::Emote { emote }).await;
                    }
                }
            }
            ServerMessage::WaveResolved {
                played,
                passed,
                cauldron_card_count,
                contributions,
            } => {
                view.cauldron_card_count = cauldron_card_count;
                view.contributions = contributions;
                if played.contains(&view.me)
                    && let Some(card) = pending_commit.take()
                {
                    view.remove_from_hand(card);
                }
                if passed.contains(&view.me) {
                    view.passed = true;
                }
                if let Some(r) = current.as_mut() {
                    r.waves += 1;
                }
            }
            ServerMessage::Exposed { card } => view.exposed_this_round.push(card),
            ServerMessage::SomeonePeeked => {}
            ServerMessage::PeekResult { boiling_point } => {
                // Sanctioned disclosure: a Peek this bot played.
                view.disclose_boiling_point(boiling_point);
            }
            ServerMessage::Depile {
                reveals,
                exploded,
                boiling_point,
                ..
            } => {
                if let Some(r) = current.as_mut() {
                    r.cards_in_pot = reveals.len() as u32;
                    r.exploded = exploded;
                }
                if exploded {
                    // Sanctioned disclosure: the boiling point shown on an explosion.
                    if let Some(bp) = boiling_point {
                        view.disclose_boiling_point(bp);
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
            ServerMessage::Explosion { pot_value, .. } => {
                if let Some(r) = current.as_mut() {
                    r.pot_value = pot_value;
                    r.exploded = true;
                }
            }
            ServerMessage::ScoreUpdate { scores } => {
                view.scores = scores;
            }
            ServerMessage::GameOver {
                final_scores: fs,
                winners: w,
            } => {
                if let Some(done) = current.take() {
                    rounds.push(done);
                }
                winners = w;
                final_scores = fs.into_iter().map(|s| (s.player, s.score)).collect();
                completed = true;
                break;
            }
            ServerMessage::PlayerConnectionChanged { player, connected } => {
                if let Some(p) = view.players.iter_mut().find(|p| p.id == player) {
                    p.connected = connected;
                }
            }
            ServerMessage::StateSnapshot {
                scores,
                active_modifiers,
                contributions,
                your_hand,
                ..
            } => {
                // Reconnection rehydrate (unused in batch play, handled for safety):
                // refresh visible state without touching the pass lockout.
                view.scores = scores;
                view.active_modifiers = active_modifiers;
                view.contributions = contributions;
                view.hand = your_hand;
            }
            ServerMessage::DeathmatchStarted { .. } => {
                // The post-game tiebreaker is headless (server-side): no bot input
                // is solicited and the outcome arrives via `GameOver`. Nothing to do.
            }
            ServerMessage::DeckReshuffled | ServerMessage::EmoteBroadcast { .. } => {}
            ServerMessage::Heartbeat => {}
            ServerMessage::Error { .. } => {} // a rejected action is a no-op for the bot
        }
    }

    Ok(GameObservation {
        me: view.me,
        my_color: view.my_color,
        rounds,
        winners,
        final_scores,
        completed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::server::{DepileEntry, PlayerScore};
    use boiling_point_protocol::vocab::CardView;
    use uuid::Uuid;

    fn pid() -> PlayerId {
        PlayerId(Uuid::from_u128(1))
    }

    /// The permitted carriers pass the audit.
    #[test]
    fn audit_allows_peek_and_exploded_depile() {
        assert!(secret_audit(&ServerMessage::PeekResult { boiling_point: 11 }).is_ok());
        assert!(secret_audit(&ServerMessage::Depile {
            reveals: vec![],
            exploded: true,
            boiling_point: Some(9),
            crossing_index: Some(0),
        })
        .is_ok());
    }

    /// A non-secret message that carries no boiling point passes.
    #[test]
    fn audit_allows_clean_safe_depile() {
        assert!(secret_audit(&ServerMessage::Depile {
            reveals: vec![DepileEntry {
                player: pid(),
                card: CardView {
                    color: Color::Ruby,
                    volatility: 1,
                    points: 1,
                    effect: None,
                },
                running_volatility: 1,
            }],
            exploded: false,
            boiling_point: None,
            crossing_index: None,
        })
        .is_ok());
        assert!(secret_audit(&ServerMessage::ScoreUpdate {
            scores: vec![PlayerScore {
                player: pid(),
                score: 3
            }]
        })
        .is_ok());
    }

    /// A crafted leak — a "safe" depile that nonetheless carries the boiling point
    /// — is caught. This is the contract the harness enforces on every message.
    #[test]
    fn audit_catches_a_leaky_safe_depile() {
        let leaky = ServerMessage::Depile {
            reveals: vec![],
            exploded: false,
            boiling_point: Some(10), // a leak: disclosed without an explosion
            crossing_index: None,
        };
        assert!(matches!(
            secret_audit(&leaky),
            Err(HarnessError::SecretLeak(_))
        ));
    }
}
