//! The multi-round game runner: deals refill-to-5 hands, draws one cumulative
//! modifier per round from round 2, runs each round's waves to completion,
//! accumulates scores, and ends after the final round.
//!
//! This is the synchronous heart that the async group task (a later task) drives
//! over the network; here it is fully testable in-process via a decision
//! callback. A tie for the lead is broken by a Deathmatch among the tied players.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use uuid::Uuid;

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::server::{Contribution, DepileEntry, PlayerScore, ScoringOutcome};
use boiling_point_protocol::vocab::{CardView, Color, HandCard, ModifierKind};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::persistence::{CompletedGame, GameStats, PlayerOutcome, StoredReplay};

use super::card::Card;
use super::deathmatch::{DeathmatchResult, run_deathmatch};
use super::deck::Deck;
use super::modifiers::ActiveModifiers;
use super::round::{DepileData, Round, RoundEnd, WaveChoice, WaveInput};
use super::scoring::{ExplosionResult, SafeScore, ScoringContext, explosion, score_safe};
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
    /// Final pot volatility.
    pub final_volatility: i32,
    /// Cards played in the round.
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
    /// Total cards each player committed across the game.
    pub cards_played: HashMap<PlayerId, u32>,
    /// The root seed this game was played from — drives all deterministic RNG
    /// (deck, modifier shuffle, boiling points, Deathmatch), so re-running from
    /// it plus [`action_log`](Self::action_log) reproduces the game exactly.
    pub seed: u64,
    /// The ordered per-wave player actions in engine decision order — the
    /// deterministic input a timeless replay re-runs. See [`crate::replay`].
    pub action_log: Vec<WaveChoice>,
}

/// One public-facing event in a reconstructed game's stream (deals, wave
/// reveals, depile, scoring, final scores). A replay re-runs the engine and
/// emits this stream; post-game it MAY reveal everything the depile revealed.
/// Mirrors the broadcast [`boiling_point_protocol::ServerMessage`] payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplayEvent {
    /// The game began with this roster.
    GameStarted {
        /// Seated players: id, colour, display name.
        players: Vec<(PlayerId, Color, String)>,
        /// Rounds to be played.
        round_count: u8,
    },
    /// A round's hands were dealt (revealed post-game).
    RoundDealt {
        /// 1-based round number.
        round_number: u8,
        /// Each player's hand after the refill.
        hands: Vec<(PlayerId, Vec<HandCard>)>,
    },
    /// A cumulative modifier became active for this round.
    ModifierRevealed {
        /// The drawn modifier.
        modifier: ModifierKind,
        /// The round it was drawn for.
        round_number: u8,
    },
    /// A wave resolved: who acted and the new pot count (never card identities).
    WaveResolved {
        /// 1-based round number.
        round_number: u8,
        /// 1-based wave number within the round.
        wave_number: u8,
        /// Players who committed a card this wave.
        played: Vec<PlayerId>,
        /// Players who passed (now locked out).
        passed: Vec<PlayerId>,
        /// Total cards now in the cauldron.
        cauldron_card_count: u8,
        /// Per-player contributed-card counts after this wave.
        contributions: Vec<Contribution>,
    },
    /// End-of-round play-order reveal of the pot.
    Depile {
        /// 1-based round number.
        round_number: u8,
        /// Revealed cards, first-added first (play order).
        reveals: Vec<DepileEntry>,
        /// Whether the round exploded.
        exploded: bool,
        /// The boiling point — revealed only on explosion.
        boiling_point: Option<u8>,
        /// Index into `reveals` of the card that tipped past the boiling point.
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
    },
    /// An explosion: everyone (bar the shielded) loses the pot value.
    Explosion {
        /// 1-based round number.
        round_number: u8,
        /// The pot value lost.
        pot_value: u32,
        /// Per-player score delta (negative loss; zero for shielded).
        deltas: Vec<PlayerScore>,
        /// Players who were shielded and took no loss.
        shielded: Vec<PlayerId>,
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

/// Build a persistable [`CompletedGame`] from the parts both game loops hold (the
/// in-process [`Game`] and the async `session::run_game`) plus the encoded
/// `replay`. Finishing positions rank by descending final score; the `stats_*`
/// summary and the deathmatch flag (a tie for the lead) are derived here. Shared
/// so both paths persist identically (review-remediation F2: the two loops
/// converge on one result shape).
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
    /// How many hand refills reshuffled the discard back in (a public table event).
    pub reshuffles: usize,
    /// The round's effective (post-modifier) boiling point — secret; for the reveal
    /// span and the private Peek disclosure only, never broadcast.
    pub effective_boiling_point: i32,
}

/// What resolving one wave produced, for a networked presenter.
pub struct WaveResolution {
    /// The cards committed this wave, with identities — server-side only, never wired.
    pub committed: Vec<(PlayerId, Card)>,
    /// Players who passed (or whose choice was an invalid / unheld card).
    pub passers: Vec<PlayerId>,
    /// Players who privately played Peek and should receive the boiling point.
    pub peeked: Vec<PlayerId>,
    /// Cards revealed to the whole table (by Expose).
    pub exposed: Vec<CardView>,
    /// Owners whose hand grew from a recall this wave — re-send each a private hand (D3).
    pub recalled_to: Vec<PlayerId>,
    /// Number of cards in the pot after this wave.
    pub pot_card_count: u8,
    /// Running pot volatility after this wave (secret — for the reveal span only).
    pub pot_volatility: i32,
    /// `Some` if this wave ended the round.
    pub ended: Option<RoundEnd>,
}

/// A round's scoring outcome.
pub enum RoundScoring {
    /// The pot exploded (shared loss).
    Exploded(ExplosionResult),
    /// The pot resolved safely (dominance payout).
    Safe(SafeScore),
}

/// What settling a round produced, for a networked presenter.
pub struct RoundSettlement {
    /// The end-of-round depile (play order, first-added first; the presenter
    /// discloses the boiling point only on an explosion).
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
    /// Choose to play a card or pass for `player`, who holds `hand`.
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
    scores: HashMap<PlayerId, i32>,
    color_owner: HashMap<Color, PlayerId>,
    deck: Deck,
    modifiers: ActiveModifiers,
    modifier_pile: Vec<ModifierKind>,
    rng: StdRng,
    bp_min: u8,
    bp_max: u8,
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

        let color_owner = players.iter().map(|p| (p.color, p.id)).collect();
        let scores = players.iter().map(|p| (p.id, 0)).collect();
        let cards_played = players.iter().map(|p| (p.id, 0)).collect();
        let hands = players.iter().map(|p| (p.id, Hand::new())).collect();
        let deck = Deck::build(registry, seed);

        Game {
            registry,
            players,
            hands,
            scores,
            color_owner,
            deck,
            modifiers: ActiveModifiers::new(),
            modifier_pile,
            rng,
            bp_min: config.boiling_point.min,
            bp_max: config.boiling_point.max,
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
    /// (`session::run_game`) drives the very same `begin_round` / `resolve_wave` /
    /// `settle_round` / `break_tie` steps over the wire, so the two cannot drift.
    pub fn play_out(&mut self, decider: &mut dyn Decider) -> GameOutcome {
        self.play_out_inner(decider, &mut EventSink::Discard)
    }

    /// Play the whole game and also collect the reconstructable public event
    /// stream (deals, wave reveals, depile, scores). Used to reconstruct a
    /// timeless replay — see [`crate::replay::reconstruct`].
    pub fn play_out_with_events(
        &mut self,
        decider: &mut dyn Decider,
    ) -> (GameOutcome, Vec<ReplayEvent>) {
        let mut events = Vec::new();
        let outcome = self.play_out_inner(decider, &mut EventSink::Collect(&mut events));
        (outcome, events)
    }

    /// The shared play loop driving the orchestration core, parameterised by where
    /// its reconstructable public events go (`Discard` for normal play, `Collect`
    /// for a replay). It steps the same `begin_round` / `resolve_wave` /
    /// `settle_round` / `break_tie` core the networked `session::run_game` drives,
    /// emitting the public event stream from those steps' returns.
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
                    .map(|p| (p.id, self.hands[&p.id].views()))
                    .collect(),
            });
            while self.round_is_open() {
                let wave_number = self.wave_number();
                let acting = self.active().to_vec();
                let mut choices = HashMap::with_capacity(acting.len());
                for player in &acting {
                    let choice = decider.decide(*player, &self.hands[player]);
                    choices.insert(*player, choice);
                }
                let resolution = self.resolve_wave(&choices);
                sink.emit(|| ReplayEvent::WaveResolved {
                    round_number,
                    wave_number,
                    played: resolution.committed.iter().map(|(p, _)| *p).collect(),
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
                    reveals: depile
                        .reveals
                        .iter()
                        .map(|item| DepileEntry {
                            player: item.player,
                            card: item.card.view(),
                            running_volatility: item.running_volatility.max(0) as u8,
                        })
                        .collect(),
                    exploded,
                    boiling_point: exploded.then_some(depile.boiling_point),
                    crossing_index: if exploded {
                        depile.crossing_index
                    } else {
                        None
                    },
                }
            });
            match &settlement.scoring {
                RoundScoring::Exploded(result) => {
                    sink.emit(|| ReplayEvent::Explosion {
                        round_number,
                        pot_value: result.pot_value,
                        deltas: deltas_in_order(&result.deltas, &self.players),
                        // Ordered by seat for a stable stream.
                        shielded: self
                            .players
                            .iter()
                            .map(|p| p.id)
                            .filter(|id| result.shielded.contains(id))
                            .collect(),
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
                        }
                    });
                }
            }
            sink.emit(|| ReplayEvent::ScoreUpdate {
                scores: scores_in_order(&self.scores, &self.players),
            });
        }
        // A tie for the lead is broken by a Deathmatch among the tied players, using
        // their remaining hands (whole-game hand management matters).
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
    /// refill every hand to the [`crate::config::HAND_SIZE`] floor, roll the hidden
    /// boiling point, and start the round with the players who still hold cards and
    /// are not `absent` (the networked path passes its disconnected set; the sync
    /// path passes an empty set). Returns what a presenter must announce.
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

        // Refill every hand to the 5-card floor (carryover kept), counting how many
        // refills reshuffled the discard back in (a public table event).
        let ids: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        let mut reshuffles = 0usize;
        for id in &ids {
            let len = self.hands[id].len();
            let (drawn, reshuffled) = self.deck.refill(len);
            self.hands.get_mut(id).unwrap().add(drawn);
            if reshuffled {
                reshuffles += 1;
            }
        }

        // Hidden boiling point for the round (+ active modifier offsets).
        let base = self.rng.gen_range(self.bp_min..=self.bp_max);
        let effective_boiling_point = self.modifiers.effective_boiling_point(base, self.registry);
        let start_vol = self.modifiers.start_volatility(self.registry);

        let active: Vec<PlayerId> = ids
            .iter()
            .copied()
            .filter(|id| !self.hands[id].is_empty() && !absent.contains(id))
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
            reshuffles,
            effective_boiling_point,
        }
    }

    /// Resolve one wave from each acting player's `choices`. Validates every choice
    /// against the player's hand (an unheld card is treated as a pass), removes the
    /// committed cards, applies the wave to the round, and returns any recalled cards
    /// to their owners. Reports what a presenter must surface. Panics if no round is
    /// open (drive after [`Game::begin_round`]).
    pub fn resolve_wave(&mut self, choices: &HashMap<PlayerId, WaveChoice>) -> WaveResolution {
        let acting: Vec<PlayerId> = self
            .current
            .as_ref()
            .expect("a round is open")
            .round
            .active()
            .to_vec();

        let mut committed = Vec::new();
        let mut passers = Vec::new();
        let mut emptied = Vec::new();
        for player in &acting {
            match choices.get(player) {
                Some(WaveChoice::Play(card_id)) => {
                    if let Some(card) = self.hands.get_mut(player).and_then(|h| h.take(*card_id)) {
                        *self.cards_played.get_mut(player).unwrap() += 1;
                        // Record the effective decision in acting order — the
                        // deterministic replay input. A committed card replays as
                        // itself; an unheld card (below) replays as a safe pass.
                        self.action_log.push(WaveChoice::Play(*card_id));
                        if self.hands[player].is_empty() {
                            emptied.push(*player);
                        }
                        committed.push((*player, card));
                    } else {
                        self.action_log.push(WaveChoice::Pass);
                        passers.push(*player); // invalid / unheld card → treated as a pass
                    }
                }
                _ => {
                    self.action_log.push(WaveChoice::Pass);
                    passers.push(*player);
                }
            }
        }

        // A snapshot of the committed (player, card) pairs for the presenter's spans;
        // `Card` is `Copy`, so this is cheap and taken before the cards move into the pot.
        let committed_report = committed.clone();

        // Copy the registry reference out before borrowing `self.current` mutably.
        let registry = self.registry;
        let round = &mut self.current.as_mut().expect("a round is open").round;
        let report = round.apply_wave(
            registry,
            WaveInput {
                committed,
                passers: passers.clone(),
                emptied,
                recalls: HashMap::new(),
            },
        );
        let ended = report.ended;
        let peeked = report.outcome.peeked;
        let exposed = report.outcome.exposed;

        // Return recalled cards to their owners' hands; their hand grew, so the owner
        // must be told (D3 — the presenter re-sends them a private hand view).
        let mut recalled_to = Vec::new();
        for (player, card) in report.outcome.recalled {
            if let Some(hand) = self.hands.get_mut(&player) {
                hand.add([card]);
                if !recalled_to.contains(&player) {
                    recalled_to.push(player);
                }
            }
        }

        let pot = self.current.as_ref().unwrap().round.pot();
        WaveResolution {
            committed: committed_report,
            passers,
            peeked,
            exposed,
            recalled_to,
            pot_card_count: pot.card_count() as u8,
            pot_volatility: pot.volatility,
            ended,
        }
    }

    /// Settle the open round: depile, score it (explosion or safe brew), fold the
    /// deltas into the cumulative scores, log the round for analytics/persistence, and
    /// return the spent cards to the discard. Hands the depile + scoring back for a
    /// presenter. Panics if no round is open.
    pub fn settle_round(&mut self) -> RoundSettlement {
        let ActiveRound {
            round,
            round_number,
            modifier,
            effective_boiling_point,
        } = self.current.take().expect("a round is open to settle");

        let all: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        let shielded = round.shielded().clone();
        let ctx = ScoringContext {
            modifiers: &self.modifiers,
            registry: self.registry,
            color_owner: &self.color_owner,
            shielded: &shielded,
            all_players: &all,
        };
        let exploded = round.ended() == Some(RoundEnd::Exploded);
        let scoring = if exploded {
            let result = explosion(round.pot(), &ctx);
            for (player, delta) in &result.deltas {
                *self.scores.get_mut(player).unwrap() += *delta;
            }
            RoundScoring::Exploded(result)
        } else {
            let result = score_safe(round.pot(), &ctx);
            for (player, delta) in &result.awards {
                *self.scores.get_mut(player).unwrap() += *delta;
            }
            RoundScoring::Safe(result)
        };

        self.rounds.push(RoundLog {
            round_number,
            effective_boiling_point,
            exploded,
            final_volatility: round.pot().volatility,
            cards_played: round.pot().card_count(),
            modifier,
        });

        // Return the pot's cards to the discard for future reshuffles.
        let spent: Vec<_> = round.pot().cards.iter().map(|pc| pc.card).collect();
        self.deck.discard_cards(spent);

        RoundSettlement {
            depile: round.depile(),
            scoring,
        }
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
    /// lowest-volatility card each forced wave. Falls back to co-winners if the
    /// Deathmatch can produce no champion (e.g. all tied hands are empty). The
    /// networked path announces `DeathmatchStarted` before calling this; the result
    /// is identical because both paths share this single tiebreak core.
    pub fn break_tie(&self, leaders: &[PlayerId]) -> Vec<PlayerId> {
        let tied: Vec<(PlayerId, Hand)> = leaders
            .iter()
            .map(|id| (*id, self.hands[id].clone()))
            .collect();
        let mut shed_lowest = |_p: PlayerId, hand: &Hand| {
            hand.views()
                .iter()
                .min_by_key(|c| c.view.volatility)
                .unwrap()
                .id
        };
        let result = run_deathmatch(
            self.registry,
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

    /// Total cards each player has committed so far.
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
    use crate::game::card::Card;
    use boiling_point_protocol::CardId;
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

    /// A decider that always plays the first card in hand, else passes.
    fn eager() -> impl FnMut(PlayerId, &Hand) -> WaveChoice {
        |_player, hand| {
            if let Some(first) = hand.views().first() {
                WaveChoice::Play(first.id)
            } else {
                WaveChoice::Pass
            }
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
            // No illegal state: every player has a recorded score.
            assert_eq!(outcome.scores.len(), 4, "seed {seed}");
        }
    }

    /// The orchestration core carries the per-player analytics and the summary
    /// stats that feed a persistable [`CompletedGame`] — the data the
    /// pre-convergence async path dropped. Both paths drive this same `Game`, so
    /// the converged async loop now has it too (converge-game-loops 2.3).
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

        // Summary stats: five rounds, four players.
        assert_eq!(completed.stats.round_count, ROUND_COUNT as i16);
        assert_eq!(completed.stats.player_count, 4);
        // Every player has a result line, and cards committed are tracked (the eager
        // decider plays whenever it holds a card).
        assert_eq!(completed.players.len(), 4);
        let total_cards: i16 = completed.players.iter().map(|p| p.cards_played).sum();
        assert!(
            total_cards > 0,
            "the converged analytics tracked cards played"
        );
        assert_eq!(completed.stats.cards_played, total_cards as i32);
        // Finishing positions are a permutation of 1..=4.
        let mut positions: Vec<i16> = completed
            .players
            .iter()
            .map(|p| p.finish_position)
            .collect();
        positions.sort();
        assert_eq!(positions, vec![1, 2, 3, 4]);
        // The winner(s) are recorded.
        assert!(completed.winner_ids.is_some_and(|w| !w.is_empty()));
    }

    #[test]
    fn games_are_deterministic_under_a_seed() {
        let (reg, cfg) = registry_and_config();
        let run = || {
            let mut g = Game::new(&reg, &cfg, four_players(), 999);
            let mut d = eager();
            g.play_out(&mut d).scores
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn tie_routes_into_deathmatch_and_produces_a_champion() {
        let (reg, cfg) = registry_and_config();
        let game = {
            let mut g = Game::new(&reg, &cfg, four_players(), 5);
            let p1 = PlayerId(Uuid::from_u128(1));
            let p2 = PlayerId(Uuid::from_u128(2));
            // p1 carries a huge-volatility card; p2 a tiny one. A forced wave
            // explodes for sure (21 > any bp), so p1 is the Detonator.
            let mut h1 = Hand::new();
            h1.add([Card {
                id: CardId(100),
                color: Color::Ruby,
                volatility: 20,
                points: 0,
                effect: None,
            }]);
            let mut h2 = Hand::new();
            h2.add([Card {
                id: CardId(101),
                color: Color::Sapphire,
                volatility: 1,
                points: 0,
                effect: None,
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

    /// End-to-end (Tasks 5.1/5.2): play a complete game in-process, persist its
    /// result **and** replay payload in one completion write, then load the
    /// replay by game id, verify its integrity, and reconstruct the public event
    /// stream — asserting it matches the originally-played game. Ignored by
    /// default (needs a live DB); run with `--ignored`.
    #[tokio::test]
    #[ignore = "requires a local PostgreSQL (DATABASE_URL)"]
    async fn full_game_persists_results_and_replay_to_db() {
        let (reg, cfg) = registry_and_config();
        let roster = four_players();
        let mut game = Game::new(&reg, &cfg, roster.clone(), 777);
        let mut decider = eager();
        let (outcome, events) = game.play_out_with_events(&mut decider);

        let game_id = Uuid::new_v4();
        let now = chrono::Utc::now();
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
        .expect("encode replay");
        let completed = game.to_completed_game(&outcome, replay, game_id, now, now);

        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres@localhost:5432/boiling_point".to_string());
        let pool = crate::persistence::connect(&url).await.expect("connect");
        crate::persistence::run_migrations(&pool)
            .await
            .expect("migrate");
        // One completion write: the consolidated game_replays row (+ player records).
        crate::persistence::persist_game(&pool, &completed)
            .await
            .expect("persist");

        // Per-player results round-trip (positions 1..=4, unique).
        let fetched = crate::persistence::fetch_player_results(&pool, game_id)
            .await
            .expect("fetch");
        assert_eq!(fetched.len(), 4);
        let mut positions: Vec<i16> = fetched.iter().map(|r| r.2).collect();
        positions.sort();
        assert_eq!(positions, vec![1, 2, 3, 4]);

        // The replay loads by game id, verifies, and reconstructs the same game.
        let loaded = crate::persistence::fetch_replay(&pool, game_id)
            .await
            .expect("fetch replay")
            .expect("a replay row exists");
        let recon = crate::replay::reconstruct(&loaded, &reg, &cfg).expect("reconstruct");
        assert_eq!(recon.events, events, "reconstructed event stream differs");
        assert_eq!(recon.outcome.scores, outcome.scores, "scores differ");
        assert_eq!(recon.outcome.winners, outcome.winners, "winners differ");
    }
}
