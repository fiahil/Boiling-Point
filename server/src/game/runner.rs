//! The multi-round game runner: deals refill-to-5 hands, draws one cumulative
//! modifier per round from round 2, runs each round's waves to completion,
//! accumulates scores, and ends after the final round.
//!
//! This is the synchronous heart that the async group task (a later task) drives
//! over the network; here it is fully testable in-process via a decision
//! callback. A tie for the lead is broken by a Deathmatch among the tied players.

use std::collections::{HashMap, HashSet};

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::vocab::{CardView, Color, ModifierKind};

use crate::config::{ContentConfig, ROUND_COUNT};
use crate::content::ContentRegistry;
use crate::persistence::{GameResult, PlayerResult, RoundResult};

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
    /// The end-of-round depile (reverse play order; the presenter discloses the
    /// boiling point only on an explosion).
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
        }
    }

    /// Play the whole game with the given decider, returning the outcome. The
    /// synchronous driver of the shared orchestration core: open each round, ask the
    /// decider for every active player's wave choice, resolve the wave, settle the
    /// round, then break any tie for the lead with a Deathmatch. The networked path
    /// (`session::run_game`) drives the very same `begin_round` / `resolve_wave` /
    /// `settle_round` / `break_tie` steps over the wire, so the two cannot drift.
    pub fn play_out(&mut self, decider: &mut dyn Decider) -> GameOutcome {
        // The sync runner has no disconnected players.
        let absent = HashSet::new();
        for round in 1..=ROUND_COUNT {
            self.begin_round(round, &absent);
            while self.round_is_open() {
                let acting = self.active().to_vec();
                let mut choices = HashMap::with_capacity(acting.len());
                for player in acting {
                    let choice = decider.decide(player, &self.hands[&player]);
                    choices.insert(player, choice);
                }
                self.resolve_wave(&choices);
            }
            self.settle_round();
        }
        // A tie for the lead is broken by a Deathmatch among the tied players, using
        // their remaining hands (whole-game hand management matters).
        let leaders = self.leaders();
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
                        if self.hands[player].is_empty() {
                            emptied.push(*player);
                        }
                        committed.push((*player, card));
                    } else {
                        passers.push(*player); // invalid / unheld card → treated as a pass
                    }
                }
                _ => passers.push(*player),
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

    /// The orchestration core carries the per-player and per-round analytics that
    /// feed a persistable [`GameResult`] — the data the pre-convergence async path
    /// dropped. Both paths drive this same `Game`, so the converged async loop now
    /// has it too (converge-game-loops 2.3).
    #[test]
    fn game_result_carries_per_player_and_per_round_analytics() {
        let (reg, cfg) = registry_and_config();
        let mut game = Game::new(&reg, &cfg, four_players(), 2024);
        let mut decider = eager();
        let outcome = game.play_out(&mut decider);

        let now = chrono::Utc::now();
        let result = game.to_game_result(&outcome, Uuid::new_v4(), now, now);

        // Per-round analytics for every round.
        assert_eq!(result.rounds.len(), ROUND_COUNT as usize);
        // Every player has a result line, and cards committed are tracked (the eager
        // decider plays whenever it holds a card).
        assert_eq!(result.players.len(), 4);
        let total_cards: i16 = result.players.iter().map(|p| p.cards_played_total).sum();
        assert!(
            total_cards > 0,
            "the converged analytics tracked cards played"
        );
        // Finishing positions are a permutation of 1..=4.
        let mut positions: Vec<i16> = result.players.iter().map(|p| p.finish_position).collect();
        positions.sort();
        assert_eq!(positions, vec![1, 2, 3, 4]);
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
