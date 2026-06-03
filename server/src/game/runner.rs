//! The multi-round game runner: deals refill-to-5 hands, draws one cumulative
//! modifier per round from round 2, runs each round's waves to completion,
//! accumulates scores, and ends after the final round.
//!
//! This is the synchronous heart that the async room task (a later task) drives
//! over the network; here it is fully testable in-process via a decision
//! callback. A tie for the lead is broken by a Deathmatch among the tied players.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use uuid::Uuid;

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::server::{Contribution, DepileEntry, PlayerScore, ScoringOutcome};
use boiling_point_protocol::vocab::{Color, HandCard, ModifierKind};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::persistence::{GameResult, PlayerResult, RoundResult};

use super::deathmatch::{DeathmatchResult, run_deathmatch};
use super::deck::Deck;
use super::modifiers::ActiveModifiers;
use super::round::{Round, RoundEnd, WaveChoice, WaveInput};
use super::scoring::{ScoringContext, explosion, score_safe};
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
    /// End-of-round reverse-order reveal of the pot.
    Depile {
        /// 1-based round number.
        round_number: u8,
        /// Revealed cards, last-added first.
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

/// Build a persistable [`GameResult`] from the parts both game loops hold (the
/// in-process [`Game`] and the async `session::run_game`). Finishing positions
/// rank by descending final score. Shared so both paths persist identically
/// (review-remediation F2: the two loops converge on one result shape).
#[allow(clippy::too_many_arguments)]
pub fn build_game_result(
    players: impl IntoIterator<Item = (PlayerId, String)>,
    scores: &HashMap<PlayerId, i32>,
    cards_played: &HashMap<PlayerId, u32>,
    rounds: &[RoundLog],
    game_id: Uuid,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
) -> GameResult {
    let players: Vec<(PlayerId, String)> = players.into_iter().collect();

    // Finishing positions by descending score.
    let mut ranked: Vec<(PlayerId, i32)> = scores.iter().map(|(p, s)| (*p, *s)).collect();
    ranked.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
    let position: HashMap<PlayerId, i16> = ranked
        .iter()
        .enumerate()
        .map(|(i, (p, _))| (*p, (i + 1) as i16))
        .collect();

    let player_results = players
        .iter()
        .map(|(id, display_name)| PlayerResult {
            player_id: id.0,
            display_name: display_name.clone(),
            final_score: scores[id],
            finish_position: position[id],
            cards_played_total: *cards_played.get(id).unwrap_or(&0) as i16,
        })
        .collect();

    let round_results = rounds
        .iter()
        .map(|r| RoundResult {
            round_number: r.round_number as i16,
            threshold: r.effective_boiling_point as i16,
            exploded: r.exploded,
            volatility_total: r.final_volatility as i16,
            cards_played: r.cards_played as i16,
        })
        .collect();

    GameResult {
        game_id,
        started_at,
        ended_at,
        round_count: rounds.len() as i16,
        players: player_results,
        rounds: round_results,
    }
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
    /// Per-wave player choices in decision order — the replay action log.
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
            action_log: Vec::new(),
        }
    }

    /// Play the whole game with the given decider, returning the outcome.
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

    /// The shared play loop, parameterised by where its public events go.
    fn play_out_inner(&mut self, decider: &mut dyn Decider, sink: &mut EventSink) -> GameOutcome {
        sink.emit(|| ReplayEvent::GameStarted {
            players: self
                .players
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone()))
                .collect(),
            round_count: ROUND_COUNT,
        });
        for round in 1..=ROUND_COUNT {
            self.play_round(round, decider, sink);
        }
        let best = self.scores.values().copied().max().unwrap_or(0);
        let leaders: Vec<PlayerId> = self
            .players
            .iter()
            .map(|p| p.id)
            .filter(|id| self.scores[id] == best)
            .collect();
        // A tie for the lead is broken by a Deathmatch among the tied players,
        // using their remaining hands (whole-game hand management matters).
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

    /// Resolve a tie for the lead via a Deathmatch among `leaders`, shedding the
    /// lowest-volatility card each forced wave. Falls back to co-winners if the
    /// Deathmatch can produce no champion (e.g. all tied hands are empty).
    fn break_tie(&self, leaders: &[PlayerId]) -> Vec<PlayerId> {
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

    fn play_round(&mut self, round_number: u8, decider: &mut dyn Decider, sink: &mut EventSink) {
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
        if let Some(kind) = modifier {
            sink.emit(|| ReplayEvent::ModifierRevealed {
                modifier: kind,
                round_number,
            });
        }

        // Refill every hand to the 5-card floor (carryover kept).
        let ids: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        for id in &ids {
            let len = self.hands[id].len();
            let (drawn, _reshuffled) = self.deck.refill(len);
            self.hands.get_mut(id).unwrap().add(drawn);
        }
        sink.emit(|| ReplayEvent::RoundDealt {
            round_number,
            hands: ids.iter().map(|id| (*id, self.hands[id].views())).collect(),
        });

        // Hidden boiling point for the round (+ active modifier offsets).
        let base = self.rng.gen_range(self.bp_min..=self.bp_max);
        let effective_bp = self.modifiers.effective_boiling_point(base, self.registry);
        let start_vol = self.modifiers.start_volatility(self.registry);

        let active: Vec<PlayerId> = ids
            .iter()
            .copied()
            .filter(|id| !self.hands[id].is_empty())
            .collect();
        let mut round = Round::start(active, effective_bp, start_vol);
        let mut wave_number: u8 = 1;

        // Run waves until the round ends.
        while round.is_open() {
            let acting: Vec<PlayerId> = round.active().to_vec();
            let mut committed = Vec::new();
            let mut passers = Vec::new();
            let mut emptied = Vec::new();
            for player in acting {
                let choice = decider.decide(player, &self.hands[&player]);
                // Record the raw decision in engine call order: this is the
                // deterministic replay input (the same coercion below re-runs on
                // reconstruction against the identically-dealt hand).
                self.action_log.push(choice);
                match choice {
                    WaveChoice::Play(card_id) => {
                        if let Some(card) = self.hands.get_mut(&player).unwrap().take(card_id) {
                            *self.cards_played.get_mut(&player).unwrap() += 1;
                            if self.hands[&player].is_empty() {
                                emptied.push(player);
                            }
                            committed.push((player, card));
                        } else {
                            passers.push(player); // invalid card → treated as a pass
                        }
                    }
                    WaveChoice::Pass => passers.push(player),
                }
            }
            let played: Vec<PlayerId> = committed.iter().map(|(p, _)| *p).collect();
            let passed = passers.clone();
            let report = round.apply_wave(
                self.registry,
                WaveInput {
                    committed,
                    passers,
                    emptied,
                    recalls: HashMap::new(),
                },
            );
            // Return any recalled cards to their owners' hands.
            for (player, card) in report.outcome.recalled {
                if let Some(hand) = self.hands.get_mut(&player) {
                    hand.add([card]);
                }
            }
            sink.emit(|| ReplayEvent::WaveResolved {
                round_number,
                wave_number,
                played,
                passed,
                cauldron_card_count: round.pot().card_count() as u8,
                contributions: pot_contributions(&round, &self.players),
            });
            wave_number += 1;
        }

        self.settle_round(round_number, &round, modifier, effective_bp, sink);
    }

    fn settle_round(
        &mut self,
        round_number: u8,
        round: &Round,
        modifier: Option<ModifierKind>,
        effective_boiling_point: i32,
        sink: &mut EventSink,
    ) {
        let all: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        let ctx = ScoringContext {
            modifiers: &self.modifiers,
            registry: self.registry,
            color_owner: &self.color_owner,
            shielded: round.shielded(),
            all_players: &all,
        };
        let exploded = round.ended() == Some(RoundEnd::Exploded);

        // The depile reveal (boiling point disclosed only on an explosion).
        sink.emit(|| {
            let depile = round.depile();
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

        if exploded {
            let result = explosion(round.pot(), &ctx);
            for (player, delta) in &result.deltas {
                *self.scores.get_mut(player).unwrap() += delta;
            }
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
        } else {
            let result = score_safe(round.pot(), &ctx);
            for (player, delta) in &result.awards {
                *self.scores.get_mut(player).unwrap() += delta;
            }
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

        self.rounds.push(RoundLog {
            round_number,
            effective_boiling_point,
            exploded,
            final_volatility: round.pot().volatility,
            cards_played: round.pot().card_count(),
            modifier,
        });
        sink.emit(|| ReplayEvent::ScoreUpdate {
            scores: scores_in_order(&self.scores, &self.players),
        });

        // Return the pot's cards to the discard for future reshuffles.
        let spent: Vec<_> = round.pot().cards.iter().map(|pc| pc.card).collect();
        self.deck.discard_cards(spent);
    }

    /// Build a persistable result for this completed game (delegates to the
    /// shared [`build_game_result`] so the async loop produces an identical shape).
    pub fn to_game_result(
        &self,
        outcome: &GameOutcome,
        game_id: Uuid,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
    ) -> GameResult {
        build_game_result(
            self.players.iter().map(|p| (p.id, p.display_name.clone())),
            &outcome.scores,
            &outcome.cards_played,
            &outcome.rounds,
            game_id,
            started_at,
            ended_at,
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
        let result = game.to_game_result(&outcome, game_id, now, now);
        let replay = crate::replay::encode_replay(
            game_id,
            outcome.seed,
            &cfg,
            roster
                .iter()
                .map(|p| (p.id, p.color, p.display_name.clone())),
            &outcome.action_log,
        )
        .expect("encode replay");

        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres@localhost:5432/boiling_point".to_string());
        let pool = crate::persistence::connect(&url).await.expect("connect");
        crate::persistence::run_migrations(&pool)
            .await
            .expect("migrate");
        // One completion write: game + per-player + per-round + replay rows.
        crate::persistence::persist_game(&pool, &result, Some(&replay))
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
