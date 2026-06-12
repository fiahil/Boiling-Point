//! Balance-statistics aggregation across a batch.
//!
//! Reduces the per-game records the runner collects into the numbers that drive
//! every `[needs playtesting]` value: explosion rate (vs the boom2 ~45% target),
//! detonator distribution by strategy, Peek-fire rate, freeze (all-pass) rates,
//! win distribution by colour and by strategy, average pot value, and average
//! cards and waves per round. Ordered maps and fixed-order folds keep the
//! aggregate itself reproducible, so two runs of the same seed produce
//! byte-identical stats.

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
    /// Fraction of rounds that exploded (compare against the ~45% boom2 target).
    pub explosion_rate: f64,
    /// Rounds that ended on an all-pass wave (the freeze / Vulture signal).
    pub all_pass_rounds: usize,
    /// Fraction of rounds ending all-pass.
    pub all_pass_rate: f64,
    /// Hard freezes: rounds that settled with an empty pot (nobody ever played).
    pub empty_pot_rounds: usize,
    /// Total detonation hits (a player named a detonator in an exploded round).
    pub detonations: usize,
    /// Mean detonators per exploded round (1 ⇒ clean single-culprit booms).
    pub avg_detonators_per_explosion: f64,
    /// Detonation hits by the liable seat's strategy (who pays the booms).
    pub detonations_by_strategy: BTreeMap<String, usize>,
    /// Total visible Peek casts.
    pub peek_casts: usize,
    /// Mean Peek casts per game (the Peek-economy dial: 8 Peeks are dealt per
    /// table per game at 2 copies × 4 seats, drawn over 5 rounds).
    pub peek_casts_per_game: f64,
    /// Total visible spell activations (Instants only — silent primes excluded).
    pub spell_casts: usize,
    /// Mean visible spell casts per round.
    pub spell_casts_per_round: f64,
    /// Mean scored pot value per round.
    pub avg_pot_value: f64,
    /// Mean ingredients in the cauldron per round.
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
        let mut all_pass_rounds = 0usize;
        let mut empty_pot_rounds = 0usize;
        let mut detonations = 0usize;
        let mut detonations_by_strategy: BTreeMap<String, usize> = BTreeMap::new();
        let mut peek_casts = 0usize;
        let mut spell_casts = 0usize;
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
                if round.ended_all_pass {
                    all_pass_rounds += 1;
                }
                if round.cards_in_pot == 0 {
                    empty_pot_rounds += 1;
                }
                for detonator in &round.detonators {
                    detonations += 1;
                    if let Some(seat) = game.seats.iter().find(|s| &s.player_id == detonator) {
                        *detonations_by_strategy
                            .entry(seat.strategy.clone())
                            .or_insert(0) += 1;
                    }
                }
                peek_casts += round.peek_casts as usize;
                spell_casts += round.spell_casts as usize;
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
        let games_f = records.len().max(1) as f64;
        BatchStats {
            games: records.len(),
            completed_games,
            rounds,
            explosions,
            explosion_rate: explosions as f64 / rounds_f,
            all_pass_rounds,
            all_pass_rate: all_pass_rounds as f64 / rounds_f,
            empty_pot_rounds,
            detonations,
            avg_detonators_per_explosion: detonations as f64 / explosions.max(1) as f64,
            detonations_by_strategy,
            peek_casts,
            peek_casts_per_game: peek_casts as f64 / games_f,
            spell_casts,
            spell_casts_per_round: spell_casts as f64 / rounds_f,
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

    /// Detonation share (0–1) for a strategy, or 0 if no detonations occurred.
    pub fn strategy_detonation_share(&self, strategy: &str) -> f64 {
        if self.detonations == 0 {
            return 0.0;
        }
        *self.detonations_by_strategy.get(strategy).unwrap_or(&0) as f64 / self.detonations as f64
    }
}
