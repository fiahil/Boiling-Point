//! The seat's player-visible view model — structurally secret-free (D2).
//!
//! The world as *this player* may know it, assembled solely from received
//! [`ServerMessage`]s. There is deliberately **no field** for an opponent's
//! hand, any deck, primed Actives, or the cauldron's absolute volatility — the
//! only way such data could enter is a message that carries it, and none does.
//! The single exception is the boiling point, which a player legitimately
//! learns in exactly two ways (a `PeekResult` from their own Peek, or the
//! post-round depile that reveals it every round); it enters only through
//! [`SeatView::disclose_boiling_point`], so leakage is a structural
//! impossibility rather than a matter of discipline.
//!
//! The view also keeps the running **transcript** of events the seat
//! legitimately observed (when enabled) — the agent brain's prompt memory.
//! Transcript lines are rendered from the same messages the view folds in, so
//! the prompt can never know more than the view does.

use boiling_point_protocol::ServerMessage;
use boiling_point_protocol::frame::PendingDecision;
use boiling_point_protocol::server::{
    Contribution, PlayerBrewer, PlayerPublic, PlayerRecipe, PlayerScore,
};
use boiling_point_protocol::vocab::{
    Brewer, Color, HandIngredient, HandSpell, IngredientView, ModifierKind, Recipe, SpellKind,
};
use boiling_point_protocol::{CardId, PlayerId};

/// The latest pending decision this seat owes, with its wave context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameContext {
    /// 1-based round number the decision belongs to.
    pub round_number: u8,
    /// 1-based wave number within the round.
    pub wave_number: u8,
    /// Remaining decision budget in milliseconds, when a timer applies.
    pub timer_ms: Option<u32>,
    /// The pending decision and its enumerated legal actions.
    pub decision: PendingDecision,
}

/// The player-visible game state a brain reasons over.
#[derive(Debug, Clone)]
pub struct SeatView {
    /// This seat's own stable id.
    pub me: PlayerId,
    /// This seat's assigned colour.
    pub my_color: Color,
    /// Everyone at the table (public info only — never hand contents).
    pub players: Vec<PlayerPublic>,
    /// Every player's public Brewer identity (empty until the pre-game pick
    /// closes; `boom2-brewers`).
    pub brewers: Vec<PlayerBrewer>,
    /// Every player's public recipe (empty until the pre-game draft closes;
    /// `boom2-apothecary`). Buckets and reserves only — never realized cards.
    pub recipes: Vec<PlayerRecipe>,
    /// This seat's own private ingredient hand (topped up each wave).
    pub ingredients: Vec<HandIngredient>,
    /// This seat's own private spell hand (the hoard).
    pub spells: Vec<HandSpell>,
    /// Current 1-based round number.
    pub round_number: u8,
    /// Current 1-based wave number within the round.
    pub wave_number: u8,
    /// Total ingredients now in the cauldron (public count, never identities).
    pub cauldron_card_count: u8,
    /// Per-player contributed-card counts in the current pot (the key political signal).
    pub contributions: Vec<Contribution>,
    /// Current cumulative scores.
    pub scores: Vec<PlayerScore>,
    /// Modifiers active this round (cumulative from round 2).
    pub active_modifiers: Vec<ModifierKind>,
    /// Ingredients Exposed to the whole table this round: (owner, card, colorless).
    pub exposed_this_round: Vec<(PlayerId, IngredientView, bool)>,
    /// Net visible volatility-spell pressure this round: +1 per observed Surge,
    /// −1 per observed Dampen (the public *delta* signal — the absolute total
    /// is never on the wire).
    pub volatility_spell_pressure: i32,
    /// The 1-based wave number an observed Quench shields, if one is pending.
    pub quench_shielded_wave: Option<u8>,
    /// Whether this seat has passed (is locked out) this round.
    pub passed: bool,
    /// The boiling point — `Some` ONLY after a sanctioned disclosure (a Peek
    /// this seat cast, or the post-round depile). Never otherwise.
    boiling_point: Option<u8>,
    /// Whether to keep the running transcript (agent seats only; bots skip the
    /// string work).
    record_transcript: bool,
    /// Events the seat legitimately observed, oldest first — the agent brain's
    /// prompt memory.
    transcript: Vec<String>,
}

impl SeatView {
    /// A fresh view for a seat that knows its own identity and colour (always
    /// confirmed/overwritten by `GroupJoined` where the transport sends one).
    pub fn new(me: PlayerId, my_color: Color) -> Self {
        SeatView {
            me,
            my_color,
            players: Vec::new(),
            brewers: Vec::new(),
            recipes: Vec::new(),
            ingredients: Vec::new(),
            spells: Vec::new(),
            round_number: 0,
            wave_number: 0,
            cauldron_card_count: 0,
            contributions: Vec::new(),
            scores: Vec::new(),
            active_modifiers: Vec::new(),
            exposed_this_round: Vec::new(),
            volatility_spell_pressure: 0,
            quench_shielded_wave: None,
            passed: false,
            boiling_point: None,
            record_transcript: false,
            transcript: Vec::new(),
        }
    }

    /// Enable the running observed-events transcript (the agent brain's memory).
    pub fn with_transcript(mut self) -> Self {
        self.record_transcript = true;
        self
    }

    /// The transcript of events this seat legitimately observed, oldest first.
    pub fn transcript(&self) -> &[String] {
        &self.transcript
    }

    /// Start-of-round reset: the round's transient knowledge clears; cumulative
    /// state (scores, modifiers, hands) is untouched.
    fn begin_round(&mut self) {
        self.passed = false;
        self.exposed_this_round.clear();
        self.volatility_spell_pressure = 0;
        self.quench_shielded_wave = None;
        self.cauldron_card_count = 0;
        self.boiling_point = None;
    }

    /// Record the sanctioned disclosure of the boiling point — the ONLY route
    /// by which the value enters this model.
    pub fn disclose_boiling_point(&mut self, value: u8) {
        self.boiling_point = Some(value);
    }

    /// The boiling point if (and only if) it has been disclosed to this seat.
    pub fn known_boiling_point(&self) -> Option<u8> {
        self.boiling_point
    }

    /// Forget a spell this seat cast (its own action; the next `YourHand`
    /// refresh re-syncs regardless — a primed Active is never echoed back).
    pub fn remove_spell(&mut self, spell: CardId) {
        if let Some(pos) = self.spells.iter().position(|s| s.id == spell) {
            self.spells.remove(pos);
        }
    }

    /// A player's display name, for transcript lines ("?" until the roster is known).
    fn name_of(&self, id: PlayerId) -> String {
        self.players
            .iter()
            .find(|p| p.id == id)
            .map(|p| {
                if p.id == self.me {
                    format!("{} (you)", p.display_name)
                } else {
                    p.display_name.clone()
                }
            })
            .unwrap_or_else(|| "?".into())
    }

    /// A player's public Brewer, once the pick has closed.
    pub fn brewer_of(&self, id: PlayerId) -> Option<Brewer> {
        self.brewers
            .iter()
            .find(|b| b.player == id)
            .map(|b| b.brewer)
    }

    /// A player's public recipe, once the draft has closed.
    pub fn recipe_of(&self, id: PlayerId) -> Option<&Recipe> {
        self.recipes
            .iter()
            .find(|r| r.player == id)
            .map(|r| &r.recipe)
    }

    /// This seat's current cumulative score, if known.
    pub fn my_score(&self) -> Option<i32> {
        self.scores
            .iter()
            .find(|s| s.player == self.me)
            .map(|s| s.score)
    }

    /// The highest score currently held by any *opponent*, if known.
    pub fn best_opponent_score(&self) -> Option<i32> {
        self.scores
            .iter()
            .filter(|s| s.player != self.me)
            .map(|s| s.score)
            .max()
    }

    /// A coarse public estimate of the cauldron's volatility: card count times
    /// an assumed mean per-card volatility, plus the observed spell pressure at
    /// an assumed magnitude. Pure heuristics over public signals.
    pub fn estimated_volatility(&self) -> f64 {
        const ASSUMED_MEAN_VOLATILITY: f64 = 2.4;
        const ASSUMED_SPELL_DELTA: f64 = 3.0;
        self.cauldron_card_count as f64 * ASSUMED_MEAN_VOLATILITY
            + self.volatility_spell_pressure as f64 * ASSUMED_SPELL_DELTA
    }

    /// Append a transcript line (when recording is enabled).
    fn log(&mut self, line: impl FnOnce(&SeatView) -> String) {
        if self.record_transcript {
            let rendered = line(self);
            self.transcript.push(rendered);
        }
    }

    /// Fold one inbound message into the view: the pure state part plus the
    /// transcript line it legitimately discloses.
    pub fn observe(&mut self, msg: &ServerMessage) {
        match msg {
            ServerMessage::GroupJoined {
                your_player_id,
                your_color,
                players,
                ..
            } => {
                self.me = *your_player_id;
                self.my_color = *your_color;
                self.players = players.clone();
            }
            ServerMessage::GameStarting { players, .. } => {
                self.players = players.clone();
                self.log(|v| {
                    let roster: Vec<String> = v
                        .players
                        .iter()
                        .map(|p| format!("{} ({:?})", p.display_name, p.color))
                        .collect();
                    format!("Game starting. Table: {}.", roster.join(", "))
                });
            }
            ServerMessage::YourHand {
                ingredients,
                spells,
            } => {
                self.ingredients = ingredients.clone();
                self.spells = spells.clone();
            }
            ServerMessage::WaveOpened {
                round_number,
                wave_number,
                final_wave,
                ..
            } => {
                if *wave_number == 1 {
                    self.begin_round();
                    self.log(|_| format!("--- Round {round_number} begins ---"));
                }
                self.round_number = *round_number;
                self.wave_number = *wave_number;
                if self.quench_shielded_wave.is_some_and(|w| w < *wave_number) {
                    self.quench_shielded_wave = None;
                }
                let final_note = if *final_wave { " (final wave)" } else { "" };
                self.log(|_| {
                    format!("Round {round_number}, wave {wave_number} opens{final_note}.")
                });
            }
            ServerMessage::SpellCast {
                player,
                spell,
                color_target,
            } => {
                match spell {
                    SpellKind::Surge => self.volatility_spell_pressure += 1,
                    SpellKind::Dampen => self.volatility_spell_pressure -= 1,
                    SpellKind::Quench => self.quench_shielded_wave = Some(self.wave_number + 1),
                    _ => {}
                }
                let (player, spell, color_target) = (*player, *spell, *color_target);
                self.log(|v| {
                    let target = color_target
                        .map(|c| format!(" aimed at {c:?}"))
                        .unwrap_or_default();
                    format!("{} cast {spell:?}{target}.", v.name_of(player))
                });
            }
            ServerMessage::WaveResolved {
                played,
                passed,
                cauldron_card_count,
                contributions,
            } => {
                self.cauldron_card_count = *cauldron_card_count;
                self.contributions = contributions.clone();
                if passed.contains(&self.me) {
                    self.passed = true;
                }
                let (played, passed, count) =
                    (played.clone(), passed.clone(), *cauldron_card_count);
                self.log(|v| {
                    let names = |ids: &[PlayerId]| -> String {
                        if ids.is_empty() {
                            "nobody".into()
                        } else {
                            ids.iter()
                                .map(|p| v.name_of(*p))
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    };
                    format!(
                        "Wave resolved: played {}; passed {}. Cauldron now holds {count} cards.",
                        names(&played),
                        names(&passed),
                    )
                });
            }
            ServerMessage::ModifierRevealed {
                modifier,
                round_number,
            } => {
                self.active_modifiers.push(*modifier);
                let (modifier, round_number) = (*modifier, *round_number);
                self.log(|_| format!("Round {round_number} modifier revealed: {modifier:?}."));
            }
            ServerMessage::Exposed {
                player,
                ingredient,
                colorless,
            } => {
                self.exposed_this_round
                    .push((*player, *ingredient, *colorless));
                let (player, ing, colorless) = (*player, *ingredient, *colorless);
                self.log(|v| {
                    let mode = if colorless { ", played colorless" } else { "" };
                    format!(
                        "Expose revealed {}'s pot card: {:?} volatility {} points {}{mode}.",
                        v.name_of(player),
                        ing.color,
                        ing.volatility,
                        ing.points,
                    )
                });
            }
            ServerMessage::PeekResult { boiling_point } => {
                // Sanctioned disclosure: a Peek this seat cast.
                self.disclose_boiling_point(*boiling_point);
                let bp = *boiling_point;
                self.log(|_| format!("Your Peek: the boiling point is exactly {bp}."));
            }
            ServerMessage::AssayResult { dominant, lead } => {
                let (dominant, lead) = (*dominant, *lead);
                self.log(|_| match dominant {
                    Some(c) => format!("Your Assay: {c:?} dominates the pot by {lead} points."),
                    None => "Your Assay: no colored points in the pot yet.".into(),
                });
            }
            ServerMessage::Depile {
                reveals,
                exploded,
                boiling_point,
                ..
            } => {
                // Sanctioned disclosure: the post-round reveal (every round).
                self.disclose_boiling_point(*boiling_point);
                let (exploded, bp, cards) = (*exploded, *boiling_point, reveals.len());
                self.log(|_| {
                    let outcome = if exploded {
                        "EXPLODED"
                    } else {
                        "settled safely"
                    };
                    format!(
                        "Depile: the pot ({cards} cards) {outcome}; the boiling point was {bp}."
                    )
                });
            }
            ServerMessage::RoundScored { awards, .. } => {
                let awards = awards.clone();
                self.log(|v| {
                    let lines: Vec<String> = awards
                        .iter()
                        .filter(|a| a.score != 0)
                        .map(|a| format!("{} {:+}", v.name_of(a.player), a.score))
                        .collect();
                    format!(
                        "Safe brew scored: {}.",
                        if lines.is_empty() {
                            "no points moved".into()
                        } else {
                            lines.join(", ")
                        }
                    )
                });
            }
            ServerMessage::Explosion {
                pot_value,
                detonators,
                ..
            } => {
                let (pot_value, detonators) = (*pot_value, detonators.clone());
                self.log(|v| {
                    let who: Vec<String> = detonators.iter().map(|p| v.name_of(*p)).collect();
                    format!(
                        "EXPLOSION: pot value {pot_value} split by the detonators: {}.",
                        who.join(", ")
                    )
                });
            }
            ServerMessage::ScoreUpdate { scores } => {
                self.scores = scores.clone();
                let scores = scores.clone();
                self.log(|v| {
                    let lines: Vec<String> = scores
                        .iter()
                        .map(|s| format!("{} {}", v.name_of(s.player), s.score))
                        .collect();
                    format!("Scores: {}.", lines.join(", "))
                });
            }
            ServerMessage::PlayerConnectionChanged { player, connected } => {
                if let Some(p) = self.players.iter_mut().find(|p| p.id == *player) {
                    p.connected = *connected;
                }
            }
            ServerMessage::BrewersRevealed { brewers } => {
                self.brewers = brewers.clone();
                let brewers = brewers.clone();
                self.log(|v| {
                    let lines: Vec<String> = brewers
                        .iter()
                        .map(|b| format!("{} is the {:?}", v.name_of(b.player), b.brewer))
                        .collect();
                    format!("Brewers revealed: {}.", lines.join("; "))
                });
            }
            ServerMessage::RecipesRevealed { recipes } => {
                self.recipes = recipes.clone();
                let recipes = recipes.clone();
                self.log(|v| {
                    let lines: Vec<String> = recipes
                        .iter()
                        .map(|r| {
                            let pantry: Vec<&str> =
                                r.recipe.pantry.iter().map(|b| b.name()).collect();
                            let grimoire: Vec<&str> =
                                r.recipe.grimoire.iter().map(|b| b.name()).collect();
                            format!(
                                "{} took {} / {}",
                                v.name_of(r.player),
                                pantry.join("+"),
                                grimoire.join("+"),
                            )
                        })
                        .collect();
                    format!("Recipes revealed: {}.", lines.join("; "))
                });
            }
            ServerMessage::StateSnapshot {
                round_number,
                players,
                brewers,
                recipes,
                scores,
                active_modifiers,
                contributions,
                your_ingredients,
                your_spells,
                ..
            } => {
                // Reconnection rehydrate: refresh visible state without touching
                // the pass lockout.
                self.round_number = *round_number;
                self.players = players.clone();
                self.brewers = brewers.clone();
                self.recipes = recipes.clone();
                self.scores = scores.clone();
                self.active_modifiers = active_modifiers.clone();
                self.contributions = contributions.clone();
                self.ingredients = your_ingredients.clone();
                self.spells = your_spells.clone();
                self.log(|_| "Reconnected: state restored from the server snapshot.".into());
            }
            ServerMessage::DeathmatchStarted { participants } => {
                let participants = participants.clone();
                self.log(|v| {
                    let who: Vec<String> = participants.iter().map(|p| v.name_of(*p)).collect();
                    format!("Deathmatch tiebreak between: {}.", who.join(", "))
                });
            }
            ServerMessage::GameOver { winners, .. } => {
                let winners = winners.clone();
                self.log(|v| {
                    let who: Vec<String> = winners.iter().map(|p| v.name_of(*p)).collect();
                    format!("Game over. Winner(s): {}.", who.join(", "))
                });
            }
            ServerMessage::EmoteBroadcast { .. }
            | ServerMessage::GroupSearching { .. }
            | ServerMessage::StandingsUpdate { .. }
            | ServerMessage::LeftGroup
            | ServerMessage::Error { .. }
            | ServerMessage::DecisionFrame { .. }
            // Account/rating readouts (`boom2-identity`) carry no game state the
            // view models; the seat captures the rating onto its outcome directly.
            | ServerMessage::AccountEstablished { .. }
            | ServerMessage::AccountDeleted
            | ServerMessage::RatingUpdate { .. }
            | ServerMessage::Heartbeat => {}
        }
    }
}

/// Audit one inbound message for a leaked server secret (the live boundary
/// check every seat runs on every message). The protocol permits the boiling
/// point on exactly two messages: a private `PeekResult` and the post-round
/// `Depile`. Anything else carrying one is a server-side breach.
pub fn secret_audit(msg: &ServerMessage) -> Result<(), crate::ClientError> {
    if matches!(
        msg,
        ServerMessage::PeekResult { .. } | ServerMessage::Depile { .. }
    ) {
        return Ok(());
    }
    let json = boiling_point_protocol::codec::encode_json(msg)
        .map_err(|e| crate::ClientError::SecretLeak(format!("could not audit message: {e}")))?;
    if json.contains("\"boiling_point\"") && !json.contains("\"boiling_point\":null") {
        return Err(crate::ClientError::SecretLeak(format!(
            "a non-secret message disclosed the boiling point: {json}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::server::DepileEntry;
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    /// The boiling point enters the view through exactly its two sanctioned
    /// routes, and a round reset forgets it.
    #[test]
    fn boiling_point_has_only_sanctioned_routes() {
        let mut v = SeatView::new(pid(1), Color::Ruby);
        assert_eq!(v.known_boiling_point(), None);
        v.observe(&ServerMessage::PeekResult { boiling_point: 27 });
        assert_eq!(v.known_boiling_point(), Some(27));
        // A new round forgets the old disclosure.
        v.observe(&ServerMessage::WaveOpened {
            round_number: 2,
            wave_number: 1,
            timer_ms: 1000,
            final_wave: false,
        });
        assert_eq!(v.known_boiling_point(), None);
        v.observe(&ServerMessage::Depile {
            reveals: vec![],
            exploded: false,
            boiling_point: 31,
            crossing_index: None,
        });
        assert_eq!(v.known_boiling_point(), Some(31));
    }

    /// The audit flags any other message shape that smuggles the value.
    #[test]
    fn secret_audit_accepts_sanctioned_and_would_flag_others() {
        assert!(secret_audit(&ServerMessage::PeekResult { boiling_point: 9 }).is_ok());
        assert!(
            secret_audit(&ServerMessage::Depile {
                reveals: vec![DepileEntry {
                    player: pid(1),
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
                boiling_point: 9,
                crossing_index: None,
            })
            .is_ok()
        );
        assert!(
            secret_audit(&ServerMessage::ScoreUpdate { scores: vec![] }).is_ok(),
            "clean broadcasts pass"
        );
    }

    /// The public Brewer assignments enter through the reveal (and a snapshot)
    /// and are queryable per player.
    #[test]
    fn brewers_arrive_via_the_reveal() {
        use boiling_point_protocol::server::PlayerBrewer;
        use boiling_point_protocol::vocab::Brewer;
        let mut v = SeatView::new(pid(1), Color::Ruby).with_transcript();
        assert_eq!(v.brewer_of(pid(2)), None);
        v.observe(&ServerMessage::BrewersRevealed {
            brewers: vec![
                PlayerBrewer {
                    player: pid(1),
                    brewer: Brewer::Forager,
                },
                PlayerBrewer {
                    player: pid(2),
                    brewer: Brewer::Lurker,
                },
            ],
        });
        assert_eq!(v.brewer_of(pid(1)), Some(Brewer::Forager));
        assert_eq!(v.brewer_of(pid(2)), Some(Brewer::Lurker));
        assert!(
            v.transcript().last().unwrap().contains("Brewers revealed"),
            "the reveal lands in the transcript"
        );
    }

    /// The transcript records only what the seat observed, in order, and stays
    /// empty unless enabled.
    #[test]
    fn transcript_is_opt_in_and_ordered() {
        let mut silent = SeatView::new(pid(1), Color::Ruby);
        silent.observe(&ServerMessage::WaveOpened {
            round_number: 1,
            wave_number: 1,
            timer_ms: 1000,
            final_wave: false,
        });
        assert!(silent.transcript().is_empty());

        let mut v = SeatView::new(pid(1), Color::Ruby).with_transcript();
        v.observe(&ServerMessage::WaveOpened {
            round_number: 1,
            wave_number: 1,
            timer_ms: 1000,
            final_wave: false,
        });
        v.observe(&ServerMessage::PeekResult { boiling_point: 22 });
        let t = v.transcript();
        assert!(t[0].contains("Round 1 begins"));
        assert!(t.last().unwrap().contains("boiling point is exactly 22"));
    }
}
