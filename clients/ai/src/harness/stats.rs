//! Per-cell balance statistics (task 6.2): the numbers that drive every
//! `[needs playtesting]` value, aggregated reproducibly (ordered maps, fixed
//! folds) so the same seed yields byte-identical stats.

use std::collections::BTreeMap;

use serde::Serialize;

use super::runner::CellRun;

/// Per-seat-label aggregates within one cell.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LabelStats {
    /// Win credits (co-winners each count).
    pub wins: usize,
    /// Detonation hits (named a detonator in an exploded round).
    pub detonations: usize,
    /// Times this label folded early and watched the round settle safely.
    pub folded_safe: usize,
    /// Decisions answered.
    pub decisions: u64,
    /// Fallback commits (budget misses + illegal answers).
    pub fallbacks: u64,
}

impl LabelStats {
    /// Fallbacks per decision (the D6 health signal).
    pub fn fallback_rate(&self) -> f64 {
        if self.decisions == 0 {
            0.0
        } else {
            self.fallbacks as f64 / self.decisions as f64
        }
    }
}

/// One matrix cell on a secondary axis (persona × Brewer, persona ×
/// deck-archetype): outcomes of the games in which a seat with this label
/// carried this axis value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct BrewerCell {
    /// Games this (label, axis-value) pairing was seated in.
    pub games: usize,
    /// Win credits (co-winners each count).
    pub wins: usize,
    /// Detonation hits (named a detonator in an exploded round).
    pub detonations: usize,
}

impl BrewerCell {
    /// Wins per seated game (the per-seat baseline is 0.25 at a 4-seat table).
    pub fn win_rate(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            self.wins as f64 / self.games as f64
        }
    }
}

/// Aggregated balance statistics for one cell.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CellStats {
    /// Games played / completed.
    pub games: usize,
    /// Rounds played.
    pub rounds: usize,
    /// Rounds that exploded.
    pub explosions: usize,
    /// Fraction of rounds that exploded (boom2 target band ~40–50%).
    pub explosion_rate: f64,
    /// Mean detonators per exploded round.
    pub avg_detonators_per_explosion: f64,
    /// Rounds that ended on an all-pass wave (the freeze signal).
    pub all_pass_rounds: usize,
    /// Fraction of rounds ending all-pass.
    pub all_pass_rate: f64,
    /// Rounds that settled with an empty pot (hard freezes).
    pub empty_pot_rounds: usize,
    /// Visible Peek casts per game (the Peek economy).
    pub peek_casts_per_game: f64,
    /// Visible spell activations per round.
    pub spell_casts_per_round: f64,
    /// Fold-to-safety events per round (a player folded early and the round
    /// settled safely).
    pub fold_to_safety_per_round: f64,
    /// Mean scored pot value per round.
    pub avg_pot_value: f64,
    /// Mean ingredients in the cauldron per round.
    pub avg_cards_per_round: f64,
    /// Mean waves per round (the game-length signal; rounds per game is fixed).
    pub avg_waves_per_round: f64,
    /// Named combos that fired per round (`boom2-compounding`).
    pub combo_fires_per_round: f64,
    /// Count-thresholds that paid off per round (`boom2-compounding`).
    pub threshold_fires_per_round: f64,
    /// Fraction of combo-half cards left lone (no partner) — the dead-draw
    /// signal. Expected non-zero in owner-unknown decks (a lone half is a plain
    /// card); a runaway value means the combo content rarely connects.
    pub lone_combo_half_rate: f64,
    /// Compounding bonus points per round (combo + threshold).
    pub compounding_points_per_round: f64,
    /// Compounding's share of the scored pot value — the snowball read: how much
    /// of the pot comes from compounding rather than printed Votes.
    pub compounding_pot_share: f64,
    /// Total win credits.
    pub total_wins: usize,
    /// Per-label aggregates (wins, detonations, folds, fallback rates).
    pub by_label: BTreeMap<String, LabelStats>,
    /// Win credits by seat colour (the seat-fairness signal: with labels
    /// fixed per seat, a skewed colour means a seating-order bias).
    pub wins_by_color: BTreeMap<String, usize>,
    /// The persona × Brewer win/break matrix (`boom2-brewers`): outcomes keyed
    /// by seat label then by the Brewer the seat **actually picked**. Ordered
    /// maps keep the same seed byte-identical.
    pub brewer_matrix: BTreeMap<String, BTreeMap<String, BrewerCell>>,
    /// The persona × deck-archetype win/break matrix (`boom2-apothecary`):
    /// outcomes keyed by seat label then by the archetype the seat was
    /// **configured to draft** (the bot legalizes the plan per frame).
    pub archetype_matrix: BTreeMap<String, BTreeMap<String, BrewerCell>>,
    /// How often each modifier was drawn.
    pub modifier_draws: BTreeMap<String, usize>,
}

impl CellStats {
    /// Aggregate one cell's games.
    pub fn aggregate(cell: &CellRun) -> Self {
        let mut rounds = 0usize;
        let mut explosions = 0usize;
        let mut detonations = 0usize;
        let mut all_pass_rounds = 0usize;
        let mut empty_pot_rounds = 0usize;
        let mut peek_casts = 0usize;
        let mut spell_casts = 0usize;
        let mut fold_to_safety = 0usize;
        let mut pot_value_sum = 0u64;
        let mut cards_sum = 0u64;
        let mut waves_sum = 0u64;
        let mut combo_fires_sum = 0u64;
        let mut threshold_fires_sum = 0u64;
        let mut combo_halves_sum = 0u64;
        let mut lone_combo_halves_sum = 0u64;
        let mut compounding_points_sum = 0u64;
        let mut total_wins = 0usize;
        let mut by_label: BTreeMap<String, LabelStats> = BTreeMap::new();
        let mut wins_by_color: BTreeMap<String, usize> = BTreeMap::new();
        let mut brewer_matrix: BTreeMap<String, BTreeMap<String, BrewerCell>> = BTreeMap::new();
        let mut archetype_matrix: BTreeMap<String, BTreeMap<String, BrewerCell>> = BTreeMap::new();
        let mut modifier_draws: BTreeMap<String, usize> = BTreeMap::new();

        for game in &cell.games {
            let label_of = |player| {
                game.seats
                    .iter()
                    .find(|s| s.player == player)
                    .map(|s| s.label.clone())
            };
            let color_of = |player| {
                game.seats
                    .iter()
                    .find(|s| s.player == player)
                    .map(|s| format!("{:?}", s.color))
            };
            let brewer_of = |player| {
                game.seats
                    .iter()
                    .find(|s| s.player == player)
                    .and_then(|s| s.brewer.clone())
            };
            let archetype_of = |player| {
                game.seats
                    .iter()
                    .find(|s| s.player == player)
                    .and_then(|s| s.deck_archetype.clone())
            };
            for seat in &game.seats {
                let entry = by_label.entry(seat.label.clone()).or_default();
                entry.decisions += seat.decisions as u64;
                entry.fallbacks += seat.fallbacks as u64;
                if let Some(brewer) = &seat.brewer {
                    brewer_matrix
                        .entry(seat.label.clone())
                        .or_default()
                        .entry(brewer.clone())
                        .or_default()
                        .games += 1;
                }
                if let Some(archetype) = &seat.deck_archetype {
                    archetype_matrix
                        .entry(seat.label.clone())
                        .or_default()
                        .entry(archetype.clone())
                        .or_default()
                        .games += 1;
                }
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
                    if let Some(label) = label_of(*detonator) {
                        if let (Some(brewer), Some(cell)) =
                            (brewer_of(*detonator), brewer_matrix.get_mut(&label))
                            && let Some(entry) = cell.get_mut(&brewer)
                        {
                            entry.detonations += 1;
                        }
                        if let (Some(archetype), Some(cell)) =
                            (archetype_of(*detonator), archetype_matrix.get_mut(&label))
                            && let Some(entry) = cell.get_mut(&archetype)
                        {
                            entry.detonations += 1;
                        }
                        by_label.entry(label).or_default().detonations += 1;
                    }
                }
                for folder in &round.folded_safe {
                    fold_to_safety += 1;
                    if let Some(label) = label_of(*folder) {
                        by_label.entry(label).or_default().folded_safe += 1;
                    }
                }
                peek_casts += round.peek_casts as usize;
                spell_casts += round.spell_casts as usize;
                pot_value_sum += round.pot_value as u64;
                cards_sum += round.cards_in_pot as u64;
                waves_sum += round.waves as u64;
                combo_fires_sum += round.combo_fires as u64;
                threshold_fires_sum += round.threshold_fires as u64;
                combo_halves_sum += round.combo_halves as u64;
                lone_combo_halves_sum += round.lone_combo_halves as u64;
                compounding_points_sum += round.compounding_points as u64;
                if let Some(modifier) = round.modifier {
                    *modifier_draws.entry(format!("{modifier:?}")).or_insert(0) += 1;
                }
            }
            for winner in &game.winners {
                if let Some(label) = label_of(*winner) {
                    total_wins += 1;
                    if let (Some(brewer), Some(cell)) =
                        (brewer_of(*winner), brewer_matrix.get_mut(&label))
                        && let Some(entry) = cell.get_mut(&brewer)
                    {
                        entry.wins += 1;
                    }
                    if let (Some(archetype), Some(cell)) =
                        (archetype_of(*winner), archetype_matrix.get_mut(&label))
                        && let Some(entry) = cell.get_mut(&archetype)
                    {
                        entry.wins += 1;
                    }
                    by_label.entry(label).or_default().wins += 1;
                }
                if let Some(color) = color_of(*winner) {
                    *wins_by_color.entry(color).or_insert(0) += 1;
                }
            }
        }

        let rounds_f = rounds.max(1) as f64;
        let games_f = cell.games.len().max(1) as f64;
        CellStats {
            games: cell.games.len(),
            rounds,
            explosions,
            explosion_rate: explosions as f64 / rounds_f,
            avg_detonators_per_explosion: detonations as f64 / explosions.max(1) as f64,
            all_pass_rounds,
            all_pass_rate: all_pass_rounds as f64 / rounds_f,
            empty_pot_rounds,
            peek_casts_per_game: peek_casts as f64 / games_f,
            spell_casts_per_round: spell_casts as f64 / rounds_f,
            fold_to_safety_per_round: fold_to_safety as f64 / rounds_f,
            avg_pot_value: pot_value_sum as f64 / rounds_f,
            avg_cards_per_round: cards_sum as f64 / rounds_f,
            avg_waves_per_round: waves_sum as f64 / rounds_f,
            combo_fires_per_round: combo_fires_sum as f64 / rounds_f,
            threshold_fires_per_round: threshold_fires_sum as f64 / rounds_f,
            lone_combo_half_rate: lone_combo_halves_sum as f64 / combo_halves_sum.max(1) as f64,
            compounding_points_per_round: compounding_points_sum as f64 / rounds_f,
            compounding_pot_share: compounding_points_sum as f64 / pot_value_sum.max(1) as f64,
            total_wins,
            by_label,
            wins_by_color,
            brewer_matrix,
            archetype_matrix,
            modifier_draws,
        }
    }

    /// Per-Brewer aggregates across every label: (games, wins, detonations),
    /// the mutual-balance read (no Brewer may break ANY persona, so the
    /// cross-label fold is the headline number; the matrix holds the detail).
    pub fn brewer_totals(&self) -> BTreeMap<String, BrewerCell> {
        Self::axis_totals(&self.brewer_matrix)
    }

    /// Per-deck-archetype aggregates across every label — the
    /// no-degenerate-archetype read (`boom2-apothecary`).
    pub fn archetype_totals(&self) -> BTreeMap<String, BrewerCell> {
        Self::axis_totals(&self.archetype_matrix)
    }

    fn axis_totals(
        matrix: &BTreeMap<String, BTreeMap<String, BrewerCell>>,
    ) -> BTreeMap<String, BrewerCell> {
        let mut totals: BTreeMap<String, BrewerCell> = BTreeMap::new();
        for cells in matrix.values() {
            for (key, cell) in cells {
                let t = totals.entry(key.clone()).or_default();
                t.games += cell.games;
                t.wins += cell.wins;
                t.detonations += cell.detonations;
            }
        }
        totals
    }

    /// A label's win share (0–1) of this cell's win credits.
    pub fn win_share(&self, label: &str) -> f64 {
        if self.total_wins == 0 {
            return 0.0;
        }
        self.by_label.get(label).map_or(0.0, |l| l.wins as f64) / self.total_wins as f64
    }

    /// A colour's win share (0–1) of this cell's win credits.
    pub fn color_win_share(&self, color: &str) -> f64 {
        if self.total_wins == 0 {
            return 0.0;
        }
        self.wins_by_color.get(color).copied().unwrap_or(0) as f64 / self.total_wins as f64
    }
}
