//! The multi-round game runner: deals refill-to-5 hands, draws one cumulative
//! modifier per round from round 2, runs each round's waves to completion,
//! accumulates scores, and ends after the final round.
//!
//! This is the synchronous heart that the async room task (a later task) drives
//! over the network; here it is fully testable in-process via a decision
//! callback. A tie for the lead is broken by a Deathmatch among the tied players.

use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use boiling_point_protocol::vocab::{Color, ModifierKind};
use boiling_point_protocol::PlayerId;

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::persistence::{GameResult, PlayerResult, RoundResult};

use super::deathmatch::{run_deathmatch, DeathmatchResult};
use super::deck::Deck;
use super::modifiers::ActiveModifiers;
use super::round::{Round, RoundEnd, WaveChoice, WaveInput};
use super::scoring::{explosion, score_safe, ScoringContext};
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
        }
    }

    /// Play the whole game with the given decider, returning the outcome.
    pub fn play_out(&mut self, decider: &mut dyn Decider) -> GameOutcome {
        for round in 1..=ROUND_COUNT {
            self.play_round(round, decider);
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
        GameOutcome {
            scores: self.scores.clone(),
            winners,
            rounds: self.rounds.clone(),
            cards_played: self.cards_played.clone(),
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

    fn play_round(&mut self, round_number: u8, decider: &mut dyn Decider) {
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

        // Refill every hand to the 5-card floor (carryover kept).
        let ids: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        for id in &ids {
            let len = self.hands[id].len();
            let (drawn, _reshuffled) = self.deck.refill(len);
            self.hands.get_mut(id).unwrap().add(drawn);
        }

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

        // Run waves until the round ends.
        while round.is_open() {
            let acting: Vec<PlayerId> = round.active().to_vec();
            let mut committed = Vec::new();
            let mut passers = Vec::new();
            let mut emptied = Vec::new();
            for player in acting {
                let choice = decider.decide(player, &self.hands[&player]);
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
        }

        self.settle_round(round_number, &round, modifier, effective_bp);
    }

    fn settle_round(
        &mut self,
        round_number: u8,
        round: &Round,
        modifier: Option<ModifierKind>,
        effective_boiling_point: i32,
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
        if exploded {
            for (player, delta) in explosion(round.pot(), &ctx).deltas {
                *self.scores.get_mut(&player).unwrap() += delta;
            }
        } else {
            for (player, delta) in score_safe(round.pot(), &ctx).awards {
                *self.scores.get_mut(&player).unwrap() += delta;
            }
        }

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
    }

    /// Build a persistable result for this completed game.
    pub fn to_game_result(
        &self,
        outcome: &GameOutcome,
        game_id: uuid::Uuid,
        started_at: chrono::DateTime<chrono::Utc>,
        ended_at: chrono::DateTime<chrono::Utc>,
    ) -> GameResult {
        // Finishing positions by descending score.
        let mut ranked: Vec<(PlayerId, i32)> =
            outcome.scores.iter().map(|(p, s)| (*p, *s)).collect();
        ranked.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
        let position: HashMap<PlayerId, i16> = ranked
            .iter()
            .enumerate()
            .map(|(i, (p, _))| (*p, (i + 1) as i16))
            .collect();

        let players = self
            .players
            .iter()
            .map(|p| PlayerResult {
                player_id: p.id.0,
                display_name: p.display_name.clone(),
                final_score: outcome.scores[&p.id],
                finish_position: position[&p.id],
                cards_played_total: outcome.cards_played[&p.id] as i16,
            })
            .collect();

        let rounds = outcome
            .rounds
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
            round_count: ROUND_COUNT as i16,
            players,
            rounds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::Card;
    use boiling_point_protocol::vocab::Color;
    use boiling_point_protocol::CardId;
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

    /// End-to-end: play a complete game in-process, then persist and read back
    /// its result. Ignored by default (needs a live DB); run with `--ignored`.
    #[tokio::test]
    #[ignore = "requires a local PostgreSQL (DATABASE_URL)"]
    async fn full_game_persists_to_db() {
        let (reg, cfg) = registry_and_config();
        let mut game = Game::new(&reg, &cfg, four_players(), 777);
        let mut decider = eager();
        let outcome = game.play_out(&mut decider);

        let now = chrono::Utc::now();
        let result = game.to_game_result(&outcome, Uuid::new_v4(), now, now);

        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres@localhost:5432/boiling_point".to_string());
        let pool = crate::persistence::connect(&url).await.expect("connect");
        crate::persistence::run_migrations(&pool)
            .await
            .expect("migrate");
        crate::persistence::persist_game(&pool, &result)
            .await
            .expect("persist");

        let fetched = crate::persistence::fetch_player_results(&pool, result.game_id)
            .await
            .expect("fetch");
        assert_eq!(fetched.len(), 4);
        // Positions are 1..=4 and unique.
        let mut positions: Vec<i16> = fetched.iter().map(|r| r.2).collect();
        positions.sort();
        assert_eq!(positions, vec![1, 2, 3, 4]);
    }
}
