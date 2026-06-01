//! Balance-statistics aggregation across a batch.
//!
//! Reduces the per-game records the runner collects into the numbers that drive
//! every `[needs playtesting]` value: explosion rate, win distribution by colour
//! and by strategy, average pot value, average cards and waves per round, and
//! modifier-draw frequency. Ordered maps and fixed-order folds keep the aggregate
//! itself reproducible, so two runs of the same seed produce byte-identical stats.

use std::collections::BTreeMap;

use serde::Serialize;

use boiling_point_protocol::vocab::Color;

use crate::runner::GameRecord;

/// A stable wire/report name for a colour.
pub fn color_name(color: Color) -> &'static str {
    match color {
        Color::Ruby => "Ruby",
        Color::Sapphire => "Sapphire",
        Color::Emerald => "Emerald",
        Color::Amethyst => "Amethyst",
        Color::Wild => "Wild",
    }
}

/// Aggregated balance statistics for one batch. The mix of strategies is recorded
/// alongside so results are always read in context (the report restates it).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatchStats {
    /// Games requested/played.
    pub games: usize,
    /// Games that reached `GameOver`.
    pub completed_games: usize,
    /// Total rounds played across all games.
    pub rounds: usize,
    /// Rounds that exploded.
    pub explosions: usize,
    /// Fraction of rounds that exploded (compare against the ~0.30–0.40 target).
    pub explosion_rate: f64,
    /// Mean scored pot value per round.
    pub avg_pot_value: f64,
    /// Mean cards in the cauldron per round.
    pub avg_cards_per_round: f64,
    /// Mean waves per round.
    pub avg_waves_per_round: f64,
    /// Total win credits awarded (co-winners each count once).
    pub total_wins: usize,
    /// Win credits by strategy name.
    pub wins_by_strategy: BTreeMap<String, usize>,
    /// Win credits by winner colour.
    pub wins_by_color: BTreeMap<String, usize>,
    /// How many times each modifier was drawn.
    pub modifier_draws: BTreeMap<String, usize>,
}

impl BatchStats {
    /// Aggregate a batch of per-game records.
    pub fn aggregate(records: &[GameRecord]) -> Self {
        let mut rounds = 0usize;
        let mut explosions = 0usize;
        let mut pot_value_sum: u64 = 0;
        let mut cards_sum: u64 = 0;
        let mut waves_sum: u64 = 0;
        let mut total_wins = 0usize;
        let mut wins_by_strategy: BTreeMap<String, usize> = BTreeMap::new();
        let mut wins_by_color: BTreeMap<String, usize> = BTreeMap::new();
        let mut modifier_draws: BTreeMap<String, usize> = BTreeMap::new();
        let mut completed_games = 0usize;

        for game in records {
            if game.completed {
                completed_games += 1;
            }
            for round in &game.rounds {
                rounds += 1;
                if round.exploded {
                    explosions += 1;
                }
                pot_value_sum += round.pot_value as u64;
                cards_sum += round.cards_in_pot as u64;
                waves_sum += round.waves as u64;
                if let Some(modifier) = round.modifier {
                    *modifier_draws.entry(format!("{modifier:?}")).or_insert(0) += 1;
                }
            }
            for winner in &game.winners {
                if let Some(seat) = game.seats.iter().find(|s| &s.player_id == winner) {
                    total_wins += 1;
                    *wins_by_strategy.entry(seat.strategy.clone()).or_insert(0) += 1;
                    *wins_by_color
                        .entry(color_name(seat.color).to_string())
                        .or_insert(0) += 1;
                }
            }
        }

        let rounds_f = rounds.max(1) as f64;
        BatchStats {
            games: records.len(),
            completed_games,
            rounds,
            explosions,
            explosion_rate: explosions as f64 / rounds_f,
            avg_pot_value: pot_value_sum as f64 / rounds_f,
            avg_cards_per_round: cards_sum as f64 / rounds_f,
            avg_waves_per_round: waves_sum as f64 / rounds_f,
            total_wins,
            wins_by_strategy,
            wins_by_color,
            modifier_draws,
        }
    }

    /// Win share (0–1) for a strategy, or 0 if no wins were recorded.
    pub fn strategy_win_share(&self, strategy: &str) -> f64 {
        if self.total_wins == 0 {
            return 0.0;
        }
        *self.wins_by_strategy.get(strategy).unwrap_or(&0) as f64 / self.total_wins as f64
    }

    /// Win share (0–1) for a colour, or 0 if no wins were recorded.
    pub fn color_win_share(&self, color: &str) -> f64 {
        if self.total_wins == 0 {
            return 0.0;
        }
        *self.wins_by_color.get(color).unwrap_or(&0) as f64 / self.total_wins as f64
    }
}
