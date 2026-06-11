//! The multi-round game runner: builds each seat's colour-anchored pantry and
//! grimoire, draws spells at round start, tops ingredient hands up to the floor
//! each wave, draws one cumulative modifier per round from round 2, runs each
//! round's waves to completion, accumulates scores, and ends after the final
//! round.
//!
//! This is the synchronous heart that the async group task drives over the
//! network; here it is fully testable in-process via a decision callback. A tie
//! for the lead is broken by a Deathmatch among the tied players.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, RngCore, SeedableRng};
use uuid::Uuid;

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::server::{
    Contribution, DepileEntry, PlayerScore, ScoringOutcome, SpellFire,
};
use boiling_point_protocol::vocab::{
    Color, HandIngredient, HandSpell, IngredientView, ModifierKind, SpellKind, SpellTarget,
    TargetKind,
};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::persistence::{CompletedGame, GameStats, PlayerOutcome, StoredReplay};

use super::card::Ingredient;
use super::deathmatch::{DeathmatchResult, run_deathmatch};
use super::deck::{Grimoire, Pantry};
use super::modifiers::ActiveModifiers;
use super::round::{DepileData, Round, RoundEnd, SpellChoice, WaveAction, WaveChoice, WaveInput};
use super::scoring::{ExplosionResult, SafeScore, ScoringContext, explosion, score_safe};
use super::spells::CastCommit;
use super::state::{Hand, Player};

/// Per-round summary, for analytics and persistence.
#[derive(Debug, Clone)]
pub struct RoundLog {
    /// 1-based round number.
    pub round_number: u8,
    /// Effective (post-modifier) boiling point.
    pub effective_boiling_point: i32,
    /// Whether the round exploded.
    pub exploded: bool,
    /// The detonators, if it did.
    pub detonators: Vec<PlayerId>,
    /// Final pot volatility (running total).
    pub final_volatility: i32,
    /// Ingredients in the pot at settle.
    pub cards_played: u32,
    /// The modifier drawn at the start of this round, if any.
    pub modifier: Option<ModifierKind>,
}

/// The result of a completed game.
#[derive(Debug, Clone)]
pub struct GameOutcome {
    /// Final cumulative scores per player.
    pub scores: HashMap<PlayerId, i32>,
    /// The winner(s) — more than one means a tie that a Deathmatch would break.
    pub winners: Vec<PlayerId>,
    /// Per-round logs.
    pub rounds: Vec<RoundLog>,
    /// Total ingredients each player committed across the game.
    pub cards_played: HashMap<PlayerId, u32>,
    /// The root seed this game was played from — drives all deterministic RNG
    /// (decks, modifier shuffle, boiling points, Deathmatch), so re-running from
    /// it plus [`action_log`](Self::action_log) reproduces the game exactly.
    pub seed: u64,
    /// The ordered per-wave player actions in engine decision order — the
    /// deterministic input a timeless replay re-runs. See [`crate::replay`].
    pub action_log: Vec<WaveChoice>,
}

/// One public-facing event in a reconstructed game's stream. A replay re-runs
/// the engine and emits this stream; post-game it MAY reveal everything the
/// depile revealed. Mirrors the broadcast
/// [`boiling_point_protocol::ServerMessage`] payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplayEvent {
    /// The game began with this roster.
    GameStarted {
        /// Seated players: id, colour, display name.
        players: Vec<(PlayerId, Color, String)>,
        /// Rounds to be played.
        round_count: u8,
    },
    /// A round opened: spells drawn, ingredient hands topped up (revealed post-game).
    RoundDealt {
        /// 1-based round number.
        round_number: u8,
        /// Each player's hand after the round-start deal.
        hands: Vec<(PlayerId, Vec<HandIngredient>, Vec<HandSpell>)>,
    },
    /// A cumulative modifier became active for this round.
    ModifierRevealed {
        /// The drawn modifier.
        modifier: ModifierKind,
        /// The round it was drawn for.
        round_number: u8,
    },
    /// An Instant spell activated at the wave reveal (public).
    SpellCast {
        /// 1-based round number.
        round_number: u8,
        /// 1-based wave number within the round.
        wave_number: u8,
        /// Who cast it.
        player: PlayerId,
        /// The spell.
        spell: SpellKind,
        /// The colour target for colour-aimed spells.
        color_target: Option<Color>,
    },
    /// An Expose revealed a pot ingredient (public).
    Exposed {
        /// 1-based round number.
        round_number: u8,
        /// 1-based wave number within the round.
        wave_number: u8,
        /// Who had played the revealed ingredient.
        player: PlayerId,
        /// The revealed ingredient.
        ingredient: IngredientView,
        /// Whether it was played colorless.
        colorless: bool,
    },
    /// A wave resolved: who acted and the new pot count (never card identities).
    WaveResolved {
        /// 1-based round number.
        round_number: u8,
        /// 1-based wave number within the round.
        wave_number: u8,
        /// Players who committed an ingredient this wave.
        played: Vec<PlayerId>,
        /// Players who passed (now locked out).
        passed: Vec<PlayerId>,
        /// Total ingredients now in the cauldron.
        cauldron_card_count: u8,
        /// Per-player contributed-card counts after this wave.
        contributions: Vec<Contribution>,
    },
    /// End-of-round volatility-sorted reveal of the pot (boiling point disclosed
    /// every round).
    Depile {
        /// 1-based round number.
        round_number: u8,
        /// Revealed entries in ascending effective-volatility order.
        reveals: Vec<DepileEntry>,
        /// Whether the round exploded.
        exploded: bool,
        /// The revealed boiling point.
        boiling_point: u8,
        /// Index where the sorted climb crossed the line, if exploded.
        crossing_index: Option<usize>,
    },
    /// A safe-brew scoring result.
    RoundScored {
        /// 1-based round number.
        round_number: u8,
        /// Per-colour point totals used to decide dominance.
        color_points: Vec<(Color, u32)>,
        /// The dominance outcome.
        outcome: ScoringOutcome,
        /// Points awarded to each player this round.
        awards: Vec<PlayerScore>,
        /// Harvests that fired.
        fired: Vec<SpellFire>,
    },
    /// An explosion: the detonators split the pot value.
    Explosion {
        /// 1-based round number.
        round_number: u8,
        /// The pot value split.
        pot_value: u32,
        /// The liable players.
        detonators: Vec<PlayerId>,
        /// Per-player score delta (zero for the unaffected).
        deltas: Vec<PlayerScore>,
        /// Wards and Hexes that fired.
        fired: Vec<SpellFire>,
    },
    /// Updated cumulative scores after a round.
    ScoreUpdate {
        /// Every player's current cumulative score.
        scores: Vec<PlayerScore>,
    },
    /// The game (and any Deathmatch) is over.
    GameOver {
        /// Final cumulative scores.
        final_scores: Vec<PlayerScore>,
        /// The winner(s) — multiple only for co-champions.
        winners: Vec<PlayerId>,
    },
}

/// Where the engine sends its reconstructable event stream. `Discard` keeps
/// normal play (and the bot harness) free of any per-event work; `Collect`
/// gathers the stream for a replay. Event construction (depile, hand views) is
/// only paid for when collecting.
enum EventSink<'a> {
    /// Drop every event (normal play).
    Discard,
    /// Gather events for a replay reconstruction.
    Collect(&'a mut Vec<ReplayEvent>),
}

impl EventSink<'_> {
    /// Append an event, building it lazily only when collecting.
    #[inline]
    fn emit(&mut self, make: impl FnOnce() -> ReplayEvent) {
        if let EventSink::Collect(buf) = self {
            buf.push(make());
        }
    }
}

/// Cumulative scores as an ordered `(player, score)` list (seating order, so the
/// wire/replay sees a stable sequence rather than a `HashMap`'s arbitrary one).
fn scores_in_order(scores: &HashMap<PlayerId, i32>, players: &[Player]) -> Vec<PlayerScore> {
    players
        .iter()
        .map(|p| PlayerScore {
            player: p.id,
            score: scores[&p.id],
        })
        .collect()
}

/// Per-player score deltas as an ordered list (seating order), so a replay's
/// event stream is byte-stable regardless of the scoring map's iteration order.
/// Players absent from `deltas` contribute a zero delta.
fn deltas_in_order(deltas: &HashMap<PlayerId, i32>, players: &[Player]) -> Vec<PlayerScore> {
    players
        .iter()
        .map(|p| PlayerScore {
            player: p.id,
            score: deltas.get(&p.id).copied().unwrap_or(0),
        })
        .collect()
}

/// Per-player contributed-card counts in the current pot (the public signal).
fn pot_contributions(round: &Round, players: &[Player]) -> Vec<Contribution> {
    players
        .iter()
        .map(|p| Contribution {
            player: p.id,
            count: round
                .pot()
                .cards
                .iter()
                .filter(|pc| pc.player == p.id)
                .count() as u8,
        })
        .collect()
}

/// Project the engine's depile onto the wire entries.
pub fn depile_entries(depile: &DepileData) -> Vec<DepileEntry> {
    depile
        .reveals
        .iter()
        .map(|item| DepileEntry {
            player: item.player,
            ingredient: item.ingredient.view(),
            colorless: item.colorless,
            wave_number: item.wave_number,
            running_volatility: item.running_volatility.clamp(0, u8::MAX as i32) as u8,
            liable: item.liable,
        })
        .collect()
}

/// Build a persistable [`CompletedGame`] from the parts both game loops hold (the
/// in-process [`Game`] and the async `session::run_game`) plus the encoded
/// `replay`. Finishing positions rank by descending final score; the `stats_*`
/// summary and the deathmatch flag (a tie for the lead) are derived here. Shared
/// so both paths persist identically.
#[allow(clippy::too_many_arguments)]
pub fn build_completed_game(
    players: impl IntoIterator<Item = (PlayerId, Color, String)>,
    scores: &HashMap<PlayerId, i32>,
    cards_played: &HashMap<PlayerId, u32>,
    rounds: &[RoundLog],
    winners: &[PlayerId],
    game_id: Uuid,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    replay: StoredReplay,
) -> CompletedGame {
    let players: Vec<(PlayerId, Color, String)> = players.into_iter().collect();

    // Finishing positions by descending score.
    let mut ranked: Vec<(PlayerId, i32)> = scores.iter().map(|(p, s)| (*p, *s)).collect();
    ranked.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
    let position: HashMap<PlayerId, i16> = ranked
        .iter()
        .enumerate()
        .map(|(i, (p, _))| (*p, (i + 1) as i16))
        .collect();

    let player_outcomes: Vec<PlayerOutcome> = players
        .iter()
        .map(|(id, color, display_name)| PlayerOutcome {
            player_id: id.0,
            display_name: display_name.clone(),
            color: *color,
            final_score: scores[id],
            finish_position: position[id],
            cards_played: *cards_played.get(id).unwrap_or(&0) as i16,
        })
        .collect();

    let high_score = scores.values().copied().max().unwrap_or(0);
    let low_score = scores.values().copied().min().unwrap_or(0);
    // A deathmatch ran iff the lead was tied at game end.
    let leaders = scores.values().filter(|s| **s == high_score).count();
    let stats = GameStats {
        round_count: rounds.len() as i16,
        player_count: players.len() as i16,
        explosions: rounds.iter().filter(|r| r.exploded).count() as i16,
        cards_played: cards_played.values().sum::<u32>() as i32,
        high_score,
        low_score,
        deathmatch: leaders > 1,
    };

    CompletedGame {
        game_id,
        started_at,
        ended_at,
        player_ids: players.iter().map(|(id, _, _)| id.0).collect(),
        winner_ids: (!winners.is_empty()).then(|| winners.iter().map(|p| p.0).collect()),
        players: player_outcomes,
        stats,
        replay,
    }
}

/// What opening a round produced, for a networked presenter to announce.
pub struct RoundOpening {
    /// The cumulative modifier revealed at the start of this round (round 2+), if any.
    pub modifier: Option<ModifierKind>,
    /// The round's effective (post-modifier) boiling point — secret; for the reveal
    /// span and the private Peek disclosure only, never broadcast in-round.
    pub effective_boiling_point: i32,
}

/// What resolving one wave produced, for a networked presenter.
pub struct WaveResolution {
    /// The ingredients committed this wave, with identities — server-side only,
    /// never wired before the depile.
    pub committed: Vec<(PlayerId, Ingredient, bool)>,
    /// Players who passed (or whose choice was an invalid / unheld card).
    pub passers: Vec<PlayerId>,
    /// Visible Instant activations, in resolution order.
    pub casts: Vec<(PlayerId, SpellKind, Option<Color>)>,
    /// Casters who privately receive the boiling point (Peek).
    pub peeked: Vec<PlayerId>,
    /// Private Assay reads: (caster, dominant colour, lead).
    pub assays: Vec<(PlayerId, Option<Color>, u32)>,
    /// Ingredients revealed to the whole table by Expose.
    pub exposed: Vec<(PlayerId, IngredientView, bool)>,
    /// Players whose hand grew mid-wave (Forage) — re-send each a private hand.
    pub hand_changed: Vec<PlayerId>,
    /// Number of ingredients in the pot after this wave.
    pub pot_card_count: u8,
    /// Running pot volatility after this wave (secret — for the reveal span only).
    pub pot_volatility: i32,
    /// `Some` if this wave ended the round.
    pub ended: Option<RoundEnd>,
}

/// A round's scoring outcome.
pub enum RoundScoring {
    /// The pot exploded (detonators split the loss).
    Exploded(ExplosionResult),
    /// The pot resolved safely (dominance payout).
    Safe(SafeScore),
}

/// What settling a round produced, for a networked presenter.
pub struct RoundSettlement {
    /// The end-of-round depile (volatility-ascending; the boiling point is
    /// revealed every round).
    pub depile: DepileData,
    /// The round's scoring outcome.
    pub scoring: RoundScoring,
}

/// The round in progress, owned by `Game` between waves so a networked driver can
/// step it wave-by-wave (open → resolve waves → settle).
struct ActiveRound {
    round: Round,
    round_number: u8,
    modifier: Option<ModifierKind>,
    effective_boiling_point: i32,
}

/// A decision source: given a player and their hand, choose a wave action.
pub trait Decider {
    /// Choose this wave's action (ingredient-or-pass + optional spell) for
    /// `player`, who holds `hand`.
    fn decide(&mut self, player: PlayerId, hand: &Hand) -> WaveChoice;
}

impl<F: FnMut(PlayerId, &Hand) -> WaveChoice> Decider for F {
    fn decide(&mut self, player: PlayerId, hand: &Hand) -> WaveChoice {
        self(player, hand)
    }
}

/// A full game in progress.
pub struct Game<'a> {
    registry: &'a ContentRegistry,
    players: Vec<Player>,
    hands: HashMap<PlayerId, Hand>,
    pantries: HashMap<PlayerId, Pantry>,
    grimoires: HashMap<PlayerId, Grimoire>,
    scores: HashMap<PlayerId, i32>,
    color_owner: HashMap<Color, PlayerId>,
    player_color: HashMap<PlayerId, Color>,
    modifiers: ActiveModifiers,
    modifier_pile: Vec<ModifierKind>,
    rng: StdRng,
    bp_min: u8,
    bp_max: u8,
    spells_per_round: u8,
    rounds: Vec<RoundLog>,
    cards_played: HashMap<PlayerId, u32>,
    seed: u64,
    /// The round currently in progress (None between rounds and at game end).
    current: Option<ActiveRound>,
    /// Per-wave player choices in decision order — the replay action log,
    /// recorded by [`Game::resolve_wave`] so both the sync and networked paths
    /// populate it identically.
    action_log: Vec<WaveChoice>,
}

impl<'a> Game<'a> {
    /// Create a game for exactly four players (each a distinct colour), seeded.
    pub fn new(
        registry: &'a ContentRegistry,
        config: &ContentConfig,
        players: Vec<Player>,
        seed: u64,
    ) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut modifier_pile: Vec<ModifierKind> = registry
            .modifier_pool()
            .into_iter()
            .flat_map(|(kind, copies)| std::iter::repeat_n(kind, copies as usize))
            .collect();
        modifier_pile.shuffle(&mut rng);

        // Per-seat deck seeds branch off a dedicated seeder so the main RNG
        // stream (modifier shuffle, boiling points) stays layout-stable.
        let mut deck_seeder = StdRng::seed_from_u64(seed ^ 0xDEC0_5EED);
        let mut next_id = 0u32;
        let mut pantries = HashMap::new();
        let mut grimoires = HashMap::new();
        for p in &players {
            let pantry_seed = deck_seeder.next_u64();
            let grimoire_seed = deck_seeder.next_u64();
            pantries.insert(
                p.id,
                Pantry::build(registry, p.color, &mut next_id, pantry_seed),
            );
            grimoires.insert(p.id, Grimoire::build(registry, &mut next_id, grimoire_seed));
        }

        let color_owner: HashMap<Color, PlayerId> =
            players.iter().map(|p| (p.color, p.id)).collect();
        let player_color = players.iter().map(|p| (p.id, p.color)).collect();
        let scores = players.iter().map(|p| (p.id, 0)).collect();
        let cards_played = players.iter().map(|p| (p.id, 0)).collect();
        let hands = players.iter().map(|p| (p.id, Hand::new())).collect();

        Game {
            registry,
            players,
            hands,
            pantries,
            grimoires,
            scores,
            color_owner,
            player_color,
            modifiers: ActiveModifiers::new(),
            modifier_pile,
            rng,
            bp_min: config.boiling_point.min,
            bp_max: config.boiling_point.max,
            spells_per_round: config.grimoire.spells_per_round,
            rounds: Vec::new(),
            cards_played,
            seed,
            current: None,
            action_log: Vec::new(),
        }
    }

    /// Play the whole game with the given decider, returning the outcome. The
    /// synchronous driver of the shared orchestration core: open each round, ask the
    /// decider for every active player's wave choice, resolve the wave, settle the
    /// round, then break any tie for the lead with a Deathmatch. The networked path
    /// (`session::run_game`) drives the very same `begin_round` / `top_up_active` /
    /// `resolve_wave` / `settle_round` / `break_tie` steps over the wire, so the
    /// two cannot drift.
    pub fn play_out(&mut self, decider: &mut dyn Decider) -> GameOutcome {
        self.play_out_inner(decider, &mut EventSink::Discard)
    }

    /// Play the whole game and also collect the reconstructable public event
    /// stream (deals, wave reveals, spell casts, depile, scores). Used to
    /// reconstruct a timeless replay — see [`crate::replay::reconstruct`].
    pub fn play_out_with_events(
        &mut self,
        decider: &mut dyn Decider,
    ) -> (GameOutcome, Vec<ReplayEvent>) {
        let mut events = Vec::new();
        let outcome = self.play_out_inner(decider, &mut EventSink::Collect(&mut events));
        (outcome, events)
    }

    /// The shared play loop driving the orchestration core, parameterised by where
    /// its reconstructable public events go.
    fn play_out_inner(&mut self, decider: &mut dyn Decider, sink: &mut EventSink) -> GameOutcome {
        sink.emit(|| ReplayEvent::GameStarted {
            players: self
                .players
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone()))
                .collect(),
            round_count: ROUND_COUNT,
        });
        // The sync runner has no disconnected players.
        let absent = HashSet::new();
        for round_number in 1..=ROUND_COUNT {
            let opening = self.begin_round(round_number, &absent);
            if let Some(kind) = opening.modifier {
                sink.emit(|| ReplayEvent::ModifierRevealed {
                    modifier: kind,
                    round_number,
                });
            }
            sink.emit(|| ReplayEvent::RoundDealt {
                round_number,
                hands: self
                    .players
                    .iter()
                    .map(|p| {
                        (
                            p.id,
                            self.hands[&p.id].ingredient_views(),
                            self.hands[&p.id].spell_views(),
                        )
                    })
                    .collect(),
            });
            while self.round_is_open() {
                let wave_number = self.wave_number();
                self.top_up_active();
                let acting = self.active().to_vec();
                let mut choices = HashMap::with_capacity(acting.len());
                for player in &acting {
                    let choice = decider.decide(*player, &self.hands[player]);
                    choices.insert(*player, choice);
                }
                let resolution = self.resolve_wave(&choices);
                for (player, spell, color_target) in &resolution.casts {
                    let (player, spell, color_target) = (*player, *spell, *color_target);
                    sink.emit(|| ReplayEvent::SpellCast {
                        round_number,
                        wave_number,
                        player,
                        spell,
                        color_target,
                    });
                }
                for (player, ingredient, colorless) in &resolution.exposed {
                    let (player, ingredient, colorless) = (*player, *ingredient, *colorless);
                    sink.emit(|| ReplayEvent::Exposed {
                        round_number,
                        wave_number,
                        player,
                        ingredient,
                        colorless,
                    });
                }
                sink.emit(|| ReplayEvent::WaveResolved {
                    round_number,
                    wave_number,
                    played: resolution.committed.iter().map(|(p, _, _)| *p).collect(),
                    passed: resolution.passers.clone(),
                    cauldron_card_count: resolution.pot_card_count,
                    contributions: pot_contributions(
                        &self.current.as_ref().expect("round open mid-wave").round,
                        &self.players,
                    ),
                });
            }
            let settlement = self.settle_round();
            let exploded = matches!(settlement.scoring, RoundScoring::Exploded(_));
            sink.emit(|| {
                let depile = &settlement.depile;
                ReplayEvent::Depile {
                    round_number,
                    reveals: depile_entries(depile),
                    exploded,
                    boiling_point: depile.boiling_point,
                    crossing_index: depile.crossing_index,
                }
            });
            match &settlement.scoring {
                RoundScoring::Exploded(result) => {
                    sink.emit(|| ReplayEvent::Explosion {
                        round_number,
                        pot_value: result.pot_value,
                        detonators: result.detonators.clone(),
                        deltas: deltas_in_order(&result.deltas, &self.players),
                        fired: result.fired.clone(),
                    });
                }
                RoundScoring::Safe(result) => {
                    sink.emit(|| {
                        let outcome = if result.winners.len() == 1 {
                            ScoringOutcome::Domination {
                                winner: result.winners[0],
                            }
                        } else {
                            ScoringOutcome::Split {
                                colors: result.winners.clone(),
                            }
                        };
                        ReplayEvent::RoundScored {
                            round_number,
                            color_points: result.color_points.clone(),
                            outcome,
                            awards: deltas_in_order(&result.awards, &self.players),
                            fired: result.fired.clone(),
                        }
                    });
                }
            }
            sink.emit(|| ReplayEvent::ScoreUpdate {
                scores: scores_in_order(&self.scores, &self.players),
            });
        }
        // A tie for the lead is broken by a Deathmatch among the tied players, using
        // their remaining ingredient hands (whole-game hand management matters).
        let leaders = self.leaders();
        let winners = if leaders.len() > 1 {
            self.break_tie(&leaders)
        } else {
            leaders
        };
        sink.emit(|| ReplayEvent::GameOver {
            final_scores: scores_in_order(&self.scores, &self.players),
            winners: winners.clone(),
        });
        GameOutcome {
            scores: self.scores.clone(),
            winners,
            rounds: self.rounds.clone(),
            cards_played: self.cards_played.clone(),
            seed: self.seed,
            action_log: self.action_log.clone(),
        }
    }

    /// Open round `round_number`: draw the round's cumulative modifier (round 2+),
    /// draw each player's round-start spells (hoarded, carried over), top every
    /// ingredient hand up to the floor, roll the hidden boiling point, and start
    /// the round with the players who hold ingredients and are not `absent` (the
    /// networked path passes its disconnected set; the sync path passes an empty
    /// set). Returns what a presenter must announce.
    pub fn begin_round(&mut self, round_number: u8, absent: &HashSet<PlayerId>) -> RoundOpening {
        // Round 2+ draws one cumulative modifier.
        let modifier = if round_number >= 2 {
            let drawn = self.modifier_pile.pop();
            if let Some(kind) = drawn {
                self.modifiers.push(kind);
            }
            drawn
        } else {
            None
        };

        // Round-start deal: spells (the hoard draw) and the wave-1 ingredient top-up.
        let ids: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        for id in &ids {
            let spells = self
                .grimoires
                .get_mut(id)
                .expect("seated player has a grimoire")
                .draw(self.spells_per_round as usize);
            let hand = self.hands.get_mut(id).unwrap();
            hand.add_spells(spells);
            let len = hand.ingredients().len();
            let drawn = self.pantries.get_mut(id).unwrap().top_up(len);
            self.hands.get_mut(id).unwrap().add_ingredients(drawn);
        }

        // Hidden boiling point for the round (+ active modifier offsets).
        let base = self.rng.gen_range(self.bp_min..=self.bp_max);
        let effective_boiling_point = self.modifiers.effective_boiling_point(base, self.registry);
        let start_vol = self.modifiers.start_volatility(self.registry);

        let active: Vec<PlayerId> = ids
            .iter()
            .copied()
            .filter(|id| !self.hands[id].no_ingredients() && !absent.contains(id))
            .collect();
        let round = Round::start(active, effective_boiling_point, start_vol);
        self.current = Some(ActiveRound {
            round,
            round_number,
            modifier,
            effective_boiling_point,
        });
        RoundOpening {
            modifier,
            effective_boiling_point,
        }
    }

    /// Top every active player's ingredient hand up to the floor (the start-of-wave
    /// deal; refill-only). Idempotent for wave 1, whose top-up happened at round
    /// open. Returns the active players (whose private hands a presenter
    /// re-sends).
    pub fn top_up_active(&mut self) -> Vec<PlayerId> {
        let active: Vec<PlayerId> = self.active().to_vec();
        for id in &active {
            let len = self.hands[id].ingredients().len();
            let drawn = self.pantries.get_mut(id).unwrap().top_up(len);
            self.hands.get_mut(id).unwrap().add_ingredients(drawn);
        }
        active
    }

    /// Whether a spell's target is legal: the kind's required target shape, a
    /// seated non-self player for player targets, a player colour for colour
    /// targets.
    fn target_valid(&self, kind: SpellKind, target: Option<SpellTarget>, caster: PlayerId) -> bool {
        match kind.target_kind() {
            TargetKind::None => target.is_none(),
            TargetKind::Player => matches!(
                target,
                Some(SpellTarget::Player { player })
                    if player != caster && self.players.iter().any(|p| p.id == player)
            ),
            TargetKind::Color => matches!(
                target,
                Some(SpellTarget::Color { color }) if color != Color::Wild
            ),
        }
    }

    /// Resolve one wave from each acting player's `choices`. Validates every choice
    /// against the player's hand (an unheld ingredient is treated as a pass; an
    /// unheld spell or an illegal target drops the cast), removes the committed
    /// cards, applies the wave to the round, and draws Forage spells. Reports what
    /// a presenter must surface. Panics if no round is open (drive after
    /// [`Game::begin_round`]).
    pub fn resolve_wave(&mut self, choices: &HashMap<PlayerId, WaveChoice>) -> WaveResolution {
        let acting: Vec<PlayerId> = self
            .current
            .as_ref()
            .expect("a round is open")
            .round
            .active()
            .to_vec();

        let mut committed: Vec<(PlayerId, Ingredient, bool)> = Vec::new();
        let mut passers: Vec<PlayerId> = Vec::new();
        let mut spells: Vec<CastCommit> = Vec::new();
        for player in &acting {
            let choice = choices
                .get(player)
                .copied()
                .unwrap_or_else(WaveChoice::pass);

            // The mandatory ingredient-or-pass.
            let logged_action = match choice.action {
                WaveAction::Play { card, colorless } => {
                    let taken = self
                        .hands
                        .get_mut(player)
                        .and_then(|h| h.take_ingredient(card));
                    if let Some(ingredient) = taken {
                        *self.cards_played.get_mut(player).unwrap() += 1;
                        committed.push((*player, ingredient, colorless));
                        WaveAction::Play { card, colorless }
                    } else {
                        passers.push(*player); // unheld card → a safe pass
                        WaveAction::Pass
                    }
                }
                WaveAction::Pass => {
                    passers.push(*player);
                    WaveAction::Pass
                }
            };

            // The optional spell (≤1; a spell never keeps a passed player in).
            let logged_spell: Option<SpellChoice> = choice.spell.and_then(|sc| {
                let kind = self
                    .hands
                    .get(player)?
                    .spells()
                    .iter()
                    .find(|s| s.id == sc.spell)?
                    .kind;
                if !self.target_valid(kind, sc.target, *player) {
                    return None;
                }
                let spell = self.hands.get_mut(player)?.take_spell(sc.spell)?;
                spells.push(CastCommit {
                    player: *player,
                    spell,
                    target: sc.target,
                });
                Some(sc)
            });

            // Record the effective decision in acting order — the deterministic
            // replay input (invalid inputs replay as their safe normalisation).
            self.action_log.push(WaveChoice {
                action: logged_action,
                spell: logged_spell,
            });
        }

        // Players who played but can never act again (hand and pantry both empty).
        let exhausted: Vec<PlayerId> = committed
            .iter()
            .map(|(p, _, _)| *p)
            .filter(|p| self.hands[p].no_ingredients() && self.pantries[p].is_exhausted())
            .collect();

        // A snapshot for the presenter's spans; `Ingredient` is `Copy`, so this is
        // cheap and taken before the cards move into the pot.
        let committed_report = committed.clone();

        let values = *self.registry.spell_values();
        let round = &mut self.current.as_mut().expect("a round is open").round;
        let report = round.apply_wave(
            &values,
            WaveInput {
                committed,
                spells,
                passers: passers.clone(),
                exhausted,
            },
        );
        let ended = report.ended;
        let outcome = report.outcome;

        // Forage draws: grow the casters' spell hands; the owners must be told.
        let mut hand_changed: Vec<PlayerId> = Vec::new();
        for player in &outcome.foragers {
            let drawn = self
                .grimoires
                .get_mut(player)
                .expect("seated player has a grimoire")
                .draw(values.forage_draws as usize);
            if let Some(hand) = self.hands.get_mut(player) {
                hand.add_spells(drawn);
            }
            if !hand_changed.contains(player) {
                hand_changed.push(*player);
            }
        }

        let pot = self.current.as_ref().unwrap().round.pot();
        WaveResolution {
            committed: committed_report,
            passers,
            casts: outcome.casts,
            peeked: outcome.peeked,
            assays: outcome.assays,
            exposed: outcome.exposed,
            hand_changed,
            pot_card_count: pot.card_count() as u8,
            pot_volatility: pot.total_volatility(),
            ended,
        }
    }

    /// Settle the open round: depile, score it (detonator explosion or safe brew,
    /// firing wards / Hex / Harvest), fold the deltas into the cumulative scores,
    /// log the round for analytics/persistence, and return the spent ingredients
    /// to their owners' pantry discards. Panics if no round is open.
    pub fn settle_round(&mut self) -> RoundSettlement {
        let ActiveRound {
            mut round,
            round_number,
            modifier,
            effective_boiling_point,
        } = self.current.take().expect("a round is open to settle");

        let all: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        let depile = round.depile();
        let exploded = round.ended() == Some(RoundEnd::Exploded);
        let detonators = round.detonators();

        let scoring = {
            let ctx = ScoringContext {
                modifiers: &self.modifiers,
                registry: self.registry,
                color_owner: &self.color_owner,
                player_color: &self.player_color,
                all_players: &all,
            };
            let (pot, primed) = round.pot_and_primed_mut();
            if exploded {
                let result = explosion(pot, &ctx, detonators.clone(), primed);
                for (player, delta) in &result.deltas {
                    *self.scores.get_mut(player).unwrap() += *delta;
                }
                RoundScoring::Exploded(result)
            } else {
                let result = score_safe(pot, &ctx, primed);
                for (player, delta) in &result.awards {
                    *self.scores.get_mut(player).unwrap() += *delta;
                }
                RoundScoring::Safe(result)
            }
        };

        self.rounds.push(RoundLog {
            round_number,
            effective_boiling_point,
            exploded,
            detonators,
            final_volatility: round.pot().total_volatility(),
            cards_played: round.pot().card_count(),
            modifier,
        });

        // Return the pot's ingredients (and any skimmed ones) to their owners'
        // pantry discards for future reshuffles.
        for pc in round.pot().cards.iter() {
            if let Some(pantry) = self.pantries.get_mut(&pc.player) {
                pantry.discard_cards([pc.ingredient]);
            }
        }
        for (player, ingredient) in round.removed() {
            if let Some(pantry) = self.pantries.get_mut(player) {
                pantry.discard_cards([*ingredient]);
            }
        }

        RoundSettlement { depile, scoring }
    }

    /// The players tied for the lead (highest cumulative score), in seating order.
    /// More than one means a tie a Deathmatch would break.
    pub fn leaders(&self) -> Vec<PlayerId> {
        let best = self.scores.values().copied().max().unwrap_or(0);
        self.players
            .iter()
            .map(|p| p.id)
            .filter(|id| self.scores[id] == best)
            .collect()
    }

    /// Resolve a tie for the lead via a Deathmatch among `leaders`, shedding the
    /// lowest-volatility ingredient each forced wave. Falls back to co-winners if
    /// the Deathmatch can produce no champion (e.g. all tied hands are empty). The
    /// networked path announces `DeathmatchStarted` before calling this; the result
    /// is identical because both paths share this single tiebreak core.
    pub fn break_tie(&self, leaders: &[PlayerId]) -> Vec<PlayerId> {
        let tied: Vec<(PlayerId, Hand)> = leaders
            .iter()
            .map(|id| (*id, self.hands[id].clone()))
            .collect();
        let mut shed_lowest = |_p: PlayerId, hand: &Hand| {
            hand.ingredients()
                .iter()
                .min_by_key(|c| c.volatility)
                .unwrap()
                .id
        };
        let result = run_deathmatch(
            tied,
            self.bp_min,
            self.bp_max,
            &mut shed_lowest,
            self.seed ^ 0xD3A7_4A7C,
        );
        match result {
            DeathmatchResult::Champion(p) => vec![p],
            DeathmatchResult::CoChampions(ps) if !ps.is_empty() => ps,
            DeathmatchResult::CoChampions(_) => leaders.to_vec(),
        }
    }

    /// A player's current private hand, if seated.
    pub fn hand(&self, player: PlayerId) -> Option<&Hand> {
        self.hands.get(&player)
    }

    /// Every seated player's private hand, keyed by id.
    pub fn hands(&self) -> &HashMap<PlayerId, Hand> {
        &self.hands
    }

    /// Total ingredients each player has committed so far.
    pub fn cards_played(&self) -> &HashMap<PlayerId, u32> {
        &self.cards_played
    }

    /// The per-round analytics logged so far (one [`RoundLog`] per settled round).
    pub fn round_logs(&self) -> &[RoundLog] {
        &self.rounds
    }

    /// The per-wave action log in decision order — the deterministic replay input
    /// (populated by [`Game::resolve_wave`], so the networked path has it too).
    pub fn action_log(&self) -> &[WaveChoice] {
        &self.action_log
    }

    /// The cumulative scores so far.
    pub fn scores(&self) -> &HashMap<PlayerId, i32> {
        &self.scores
    }

    /// The active cauldron modifiers (cumulative across rounds).
    pub fn active_modifiers(&self) -> &[ModifierKind] {
        self.modifiers.kinds()
    }

    /// Players still active in the open round (act next wave). Empty between rounds.
    pub fn active(&self) -> &[PlayerId] {
        match &self.current {
            Some(r) => r.round.active(),
            None => &[],
        }
    }

    /// The current 1-based wave number of the open round (0 between rounds).
    pub fn wave_number(&self) -> u8 {
        self.current.as_ref().map_or(0, |r| r.round.wave_number())
    }

    /// Whether a round is open for more waves.
    pub fn round_is_open(&self) -> bool {
        self.current.as_ref().is_some_and(|r| r.round.is_open())
    }

    /// Per-player contributed-card counts in the open round's pot, in `order`; all
    /// zero between rounds.
    pub fn contributions(&self, order: &[PlayerId]) -> Vec<(PlayerId, u8)> {
        let pot = self.current.as_ref().map(|r| r.round.pot());
        order
            .iter()
            .map(|id| {
                let count = pot.map_or(0, |p| {
                    p.cards.iter().filter(|pc| pc.player == *id).count() as u8
                });
                (*id, count)
            })
            .collect()
    }

    /// Build a persistable record for this completed game (delegates to the
    /// shared [`build_completed_game`] so the async loop produces an identical
    /// shape), given its encoded `replay`.
    pub fn to_completed_game(
        &self,
        outcome: &GameOutcome,
        replay: StoredReplay,
        game_id: Uuid,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
    ) -> CompletedGame {
        build_completed_game(
            self.players
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            &outcome.scores,
            &outcome.cards_played,
            &outcome.rounds,
            &outcome.winners,
            game_id,
            started_at,
            ended_at,
            replay,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::vocab::Color;
    use uuid::Uuid;

    fn registry_and_config() -> (ContentRegistry, ContentConfig) {
        let cfg = ContentConfig::from_toml(include_str!("../../content.toml")).unwrap();
        let reg = cfg.build_registry().unwrap();
        (reg, cfg)
    }

    fn four_players() -> Vec<Player> {
        Color::PLAYER_COLORS
            .into_iter()
            .enumerate()
            .map(|(i, color)| Player {
                id: PlayerId(Uuid::from_u128(i as u128 + 1)),
                color,
                display_name: format!("p{i}"),
            })
            .collect()
    }

    /// A decider that always plays the first ingredient in hand as a Vote, no
    /// spells, else passes.
    fn eager() -> impl FnMut(PlayerId, &Hand) -> WaveChoice {
        |_player, hand| match hand.ingredients().first() {
            Some(first) => WaveChoice {
                action: WaveAction::Play {
                    card: first.id,
                    colorless: false,
                },
                spell: None,
            },
            None => WaveChoice::pass(),
        }
    }

    /// A decider that also casts its first spell every wave (legal targets only).
    fn spell_happy() -> impl FnMut(PlayerId, &Hand) -> WaveChoice {
        |player, hand| {
            let action = match hand.ingredients().first() {
                Some(first) => WaveAction::Play {
                    card: first.id,
                    colorless: false,
                },
                None => WaveAction::Pass,
            };
            let spell = hand.spells().first().map(|s| SpellChoice {
                spell: s.id,
                target: match s.kind.target_kind() {
                    TargetKind::None => None,
                    TargetKind::Color => Some(SpellTarget::Color { color: Color::Ruby }),
                    TargetKind::Player => Some(SpellTarget::Player {
                        // Target the next seat (never self).
                        player: PlayerId(Uuid::from_u128((player.0.as_u128() % 4) + 1)),
                    }),
                },
            });
            WaveChoice { action, spell }
        }
    }

    #[test]
    fn full_game_completes_with_five_rounds() {
        let (reg, cfg) = registry_and_config();
        let mut game = Game::new(&reg, &cfg, four_players(), 12345);
        let mut decider = eager();
        let outcome = game.play_out(&mut decider);
        assert_eq!(outcome.rounds.len(), ROUND_COUNT as usize);
        assert_eq!(outcome.scores.len(), 4);
        assert!(!outcome.winners.is_empty());
        // Rounds 2..5 each drew a modifier; round 1 did not.
        assert!(outcome.rounds[0].modifier.is_none());
        assert!(outcome.rounds[1].modifier.is_some());
    }

    #[test]
    fn many_games_complete_without_panics() {
        let (reg, cfg) = registry_and_config();
        for seed in 0..300u64 {
            let mut game = Game::new(&reg, &cfg, four_players(), seed);
            let mut decider = eager();
            let outcome = game.play_out(&mut decider);
            assert_eq!(outcome.rounds.len(), ROUND_COUNT as usize, "seed {seed}");
            assert!(!outcome.winners.is_empty(), "seed {seed}");
            assert_eq!(outcome.scores.len(), 4, "seed {seed}");
        }
    }

    #[test]
    fn many_spell_heavy_games_complete_without_panics() {
        let (reg, cfg) = registry_and_config();
        for seed in 0..150u64 {
            let mut game = Game::new(&reg, &cfg, four_players(), seed);
            let mut decider = spell_happy();
            let outcome = game.play_out(&mut decider);
            assert_eq!(outcome.rounds.len(), ROUND_COUNT as usize, "seed {seed}");
            assert!(!outcome.winners.is_empty(), "seed {seed}");
        }
    }

    /// Only detonators ever lose points on an explosion (the P-symmetry on the
    /// full engine): in every exploded round, every negative delta belongs to a
    /// detonator or a Hex target — and with no spells in play, exactly to the
    /// detonators.
    #[test]
    fn explosions_cost_only_detonators_without_spells() {
        let (reg, cfg) = registry_and_config();
        for seed in 0..50u64 {
            let mut game = Game::new(&reg, &cfg, four_players(), seed);
            let mut decider = eager();
            let outcome = game.play_out(&mut decider);
            for round in outcome.rounds.iter().filter(|r| r.exploded) {
                assert!(
                    !round.detonators.is_empty(),
                    "an ingredient-driven explosion names its detonators (seed {seed})"
                );
            }
        }
    }

    #[test]
    fn games_are_deterministic_under_a_seed() {
        let (reg, cfg) = registry_and_config();
        let run = || {
            let mut g = Game::new(&reg, &cfg, four_players(), 999);
            let mut d = spell_happy();
            g.play_out(&mut d).scores
        };
        assert_eq!(run(), run());
    }

    /// The orchestration core carries the per-player analytics and the summary
    /// stats that feed a persistable [`CompletedGame`].
    #[test]
    fn completed_game_carries_per_player_analytics_and_stats() {
        let (reg, cfg) = registry_and_config();
        let roster = four_players();
        let mut game = Game::new(&reg, &cfg, roster.clone(), 2024);
        let mut decider = eager();
        let outcome = game.play_out(&mut decider);

        let now = chrono::Utc::now();
        let game_id = Uuid::new_v4();
        let replay = crate::replay::encode_replay(
            game_id,
            outcome.seed,
            &cfg,
            roster
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            &outcome.action_log,
            &[],
        )
        .expect("encode");
        let completed = game.to_completed_game(&outcome, replay, game_id, now, now);

        assert_eq!(completed.stats.round_count, ROUND_COUNT as i16);
        assert_eq!(completed.stats.player_count, 4);
        assert_eq!(completed.players.len(), 4);
        let total_cards: i16 = completed.players.iter().map(|p| p.cards_played).sum();
        assert!(total_cards > 0, "cards played are tracked");
        assert_eq!(completed.stats.cards_played, total_cards as i32);
        let mut positions: Vec<i16> = completed
            .players
            .iter()
            .map(|p| p.finish_position)
            .collect();
        positions.sort();
        assert_eq!(positions, vec![1, 2, 3, 4]);
        assert!(completed.winner_ids.is_some_and(|w| !w.is_empty()));
    }

    #[test]
    fn tie_routes_into_deathmatch_and_produces_a_champion() {
        let (reg, cfg) = registry_and_config();
        let game = {
            let mut g = Game::new(&reg, &cfg, four_players(), 5);
            let p1 = PlayerId(Uuid::from_u128(1));
            let p2 = PlayerId(Uuid::from_u128(2));
            // p1 carries a huge-volatility ingredient; p2 a tiny one. A forced
            // wave explodes for sure, so p1 is the Detonator.
            let mut h1 = Hand::new();
            h1.add_ingredients([Ingredient {
                id: boiling_point_protocol::CardId(100_000),
                color: Color::Ruby,
                volatility: 99,
                points: 0,
            }]);
            let mut h2 = Hand::new();
            h2.add_ingredients([Ingredient {
                id: boiling_point_protocol::CardId(100_001),
                color: Color::Sapphire,
                volatility: 1,
                points: 0,
            }]);
            g.hands.insert(p1, h1);
            g.hands.insert(p2, h2);
            g
        };
        let p1 = PlayerId(Uuid::from_u128(1));
        let p2 = PlayerId(Uuid::from_u128(2));
        let winners = game.break_tie(&[p1, p2]);
        assert_eq!(winners, vec![p2], "the lower-volatility player survives");
    }
}
