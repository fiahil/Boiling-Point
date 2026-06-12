//! The transport-agnostic bot: a decode → update-model → ask-strategy → encode
//! loop that plays one complete game and reports what it observed.
//!
//! The loop consumes only [`ServerMessage`]s and emits only [`ClientMessage`]s, so
//! the bot is, by construction, a real client (Constitution I/II). It plays every
//! phase — join, deal, all five rounds of waves (ingredient-or-pass + optional
//! spell), any Deathmatch, through `GameOver` — issuing only legal commits,
//! casts, and passes. Every inbound message is first run through
//! [`secret_audit`]: because a bot must never receive a server secret, each game
//! is also a live test of the no-leak contract (D2).
//!
//! Alongside play, the loop collects the boom2 balance observations the
//! constitution's §IV mandate asks of the revived harness: explosion rate,
//! detonator identities, Peek-fire counts, and all-pass freezes.

use rand::rngs::StdRng;

use boiling_point_protocol::vocab::{Color, ModifierKind, SpellKind};
use boiling_point_protocol::{ClientMessage, EmoteId, PlayerId, ServerMessage, codec};

use crate::HarnessError;
use crate::model::PlayerView;
use crate::strategy::{Strategy, WaveAction};
use crate::transport::BotConnection;

/// What a bot observed in one round, derived purely from broadcast messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundObservation {
    /// 1-based round number.
    pub round_number: u8,
    /// Whether the round exploded.
    pub exploded: bool,
    /// The pot's scored value — split by the detonators on an explosion, paid
    /// out to the dominant colour on a safe brew.
    pub pot_value: u32,
    /// The detonators, if the round exploded (the liable players).
    pub detonators: Vec<PlayerId>,
    /// Ingredients that ended up in the cauldron this round (the depile length).
    pub cards_in_pot: u32,
    /// Number of waves the round ran.
    pub waves: u32,
    /// Visible Peek casts this round (the Peek-economy signal).
    pub peek_casts: u32,
    /// All visible spell activations this round (Instants; primed Actives that
    /// never fire stay invisible by design).
    pub spell_casts: u32,
    /// Whether the round ended on an all-pass wave (the freeze / Vulture signal).
    pub ended_all_pass: bool,
    /// The modifier revealed at the start of this round, if any.
    pub modifier: Option<ModifierKind>,
}

impl RoundObservation {
    fn new(round_number: u8) -> Self {
        RoundObservation {
            round_number,
            exploded: false,
            pot_value: 0,
            detonators: Vec::new(),
            cards_in_pot: 0,
            waves: 0,
            peek_casts: 0,
            spell_casts: 0,
            ended_all_pass: false,
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
/// [`ServerMessage::PeekResult`] and the post-round [`ServerMessage::Depile`]
/// (which reveals it every round, boom and safe). Any other message that carries
/// a `boiling_point` on the wire is a breach of the secret-management contract.
/// Mirrors the protocol crate's own invariant, but enforced live against the
/// stream a bot actually consumes.
pub fn secret_audit(msg: &ServerMessage) -> Result<(), HarnessError> {
    let permitted = matches!(
        msg,
        ServerMessage::PeekResult { .. } | ServerMessage::Depile { .. }
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
/// a `GroupJoined`, if the transport sends one, confirms them.
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
    let mut last_wave_all_passed = false;
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
        view.observe(&msg);
        match msg {
            ServerMessage::WaveOpened {
                round_number,
                wave_number,
                ..
            } => {
                ensure_round(&mut rounds, &mut current, round_number);
                if wave_number == 1 {
                    last_wave_all_passed = false;
                    pending_commit = None;
                }
                // Respond while still in the round — gated on the pass lockout,
                // a server-confirmed signal. The hand is re-synced by the
                // per-wave `YourHand` refresh, so the view is the true hand.
                if !view.passed {
                    let decision = if view.ingredients.is_empty() {
                        crate::strategy::WaveDecision::pass()
                    } else {
                        strategy.decide(&view, &mut rng)
                    };
                    match decision.action {
                        WaveAction::Play { card, colorless } => {
                            conn.send(ClientMessage::CommitIngredient { card, colorless })
                                .await;
                            pending_commit = Some(card);
                        }
                        WaveAction::Pass => {
                            conn.send(ClientMessage::CommitPass).await;
                            view.passed = true;
                            pending_commit = None;
                        }
                    }
                    if let Some(cast) = decision.spell {
                        conn.send(ClientMessage::CastSpell {
                            spell: cast.spell,
                            target: cast.target,
                        })
                        .await;
                        // Optimistic removal — a primed Active is never echoed
                        // back; the next YourHand refresh re-syncs regardless.
                        view.remove_spell(cast.spell);
                    }
                    // Lock in to close the wave early in-process; over WebSocket
                    // the per-connection rate limit drops this and the wave
                    // closes on its timer — either way the committed action stands.
                    conn.send(ClientMessage::LockIn).await;
                    if let Some(emote) = strategy.emote(&view, palette, &mut rng) {
                        conn.send(ClientMessage::Emote { emote }).await;
                    }
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
            ServerMessage::WaveResolved { played, .. } => {
                if played.contains(&view.me)
                    && let Some(card) = pending_commit.take()
                {
                    view.remove_ingredient(card);
                }
                last_wave_all_passed = played.is_empty();
                if let Some(r) = current.as_mut() {
                    r.waves += 1;
                }
            }
            ServerMessage::ModifierRevealed {
                modifier,
                round_number,
            } => {
                ensure_round(&mut rounds, &mut current, round_number);
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
            ServerMessage::DeathmatchStarted { .. } => {
                // The post-game tiebreaker is headless (server-side): no bot input
                // is solicited and the outcome arrives via `GameOver`. Nothing to do.
            }
            // State-only messages were folded into the view by `observe` above.
            _ => {}
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
    use boiling_point_protocol::vocab::IngredientView;
    use uuid::Uuid;

    fn pid() -> PlayerId {
        PlayerId(Uuid::from_u128(1))
    }

    /// The permitted carriers pass the audit — including the SAFE depile, which
    /// reveals the boiling point every round in the v2 core.
    #[test]
    fn audit_allows_peek_and_every_depile() {
        assert!(secret_audit(&ServerMessage::PeekResult { boiling_point: 26 }).is_ok());
        assert!(
            secret_audit(&ServerMessage::Depile {
                reveals: vec![DepileEntry {
                    player: pid(),
                    ingredient: IngredientView {
                        color: Color::Ruby,
                        volatility: 1,
                        points: 1,
                    },
                    colorless: false,
                    wave_number: 1,
                    running_volatility: 1,
                    liable: false,
                }],
                exploded: false,
                boiling_point: 26,
                crossing_index: None,
            })
            .is_ok()
        );
    }

    /// A non-secret message that carries no boiling point passes.
    #[test]
    fn audit_allows_clean_broadcasts() {
        assert!(
            secret_audit(&ServerMessage::ScoreUpdate {
                scores: vec![PlayerScore {
                    player: pid(),
                    score: 3
                }]
            })
            .is_ok()
        );
        assert!(
            secret_audit(&ServerMessage::SpellCast {
                player: pid(),
                spell: SpellKind::Surge,
                color_target: None,
            })
            .is_ok()
        );
    }
}
