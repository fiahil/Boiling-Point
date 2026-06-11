//! The balance report and smell detection (D5).
//!
//! Aggregated statistics are emitted two ways from one [`Report`] value: a
//! human-readable markdown summary for eyeballing, and a machine-readable JSON
//! file for diffing across config versions. Every report is keyed to a
//! fingerprint of the content config under test, so a tuning loop is: run → read
//! flags → edit `game-content-config` → re-run → diff.
//!
//! [`Smell`]s flag the balance problems worth a human's attention — a strategy or
//! colour winning disproportionately, an explosion rate outside the target band,
//! or runaway pots — each with the numbers that triggered it. The thresholds are
//! themselves `[needs playtesting]`: [`Thresholds::default`] ships sensible
//! starting values to be refined from real batch output.

use serde::Serialize;

use crate::stats::BatchStats;

/// A 64-bit FNV-1a fingerprint of the content config, as a hex string. Lets a
/// report state exactly which config produced it without a semantic version field.
pub fn fingerprint(config_toml: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in config_toml.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Tunable bounds for balance-smell detection. All are starting hypotheses.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Thresholds {
    /// Lower edge of the acceptable explosion-rate band.
    pub explosion_min: f64,
    /// Upper edge of the acceptable explosion-rate band.
    pub explosion_max: f64,
    /// Win share above which a single strategy/colour is flagged as dominant.
    pub dominant_win_share: f64,
    /// Average pot value above which pots are flagged as runaway.
    pub runaway_pot_value: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        // The proposal targets a ~30–40% explosion rate; with four distinct
        // strategies/colours an even share is 25%, so 40% is a clear dominance
        // signal. The runaway-pot cap is a coarse starting guess.
        Thresholds {
            explosion_min: 0.30,
            explosion_max: 0.40,
            dominant_win_share: 0.40,
            runaway_pot_value: 25.0,
        }
    }
}

/// One flagged balance smell, with the supporting numbers that triggered it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Smell {
    /// A short machine-stable category (e.g. `explosion_rate`, `dominant_strategy`).
    pub kind: String,
    /// A human-readable explanation including the numbers.
    pub detail: String,
}

impl Smell {
    /// Detect every balance smell in `stats` against `thresholds`, in a stable order.
    pub fn detect(stats: &BatchStats, thresholds: &Thresholds) -> Vec<Smell> {
        let mut smells = Vec::new();

        // Explosion rate outside the target band.
        if stats.rounds > 0
            && (stats.explosion_rate < thresholds.explosion_min
                || stats.explosion_rate > thresholds.explosion_max)
        {
            let missed = if stats.explosion_rate < thresholds.explosion_min {
                "below"
            } else {
                "above"
            };
            smells.push(Smell {
                kind: "explosion_rate".into(),
                detail: format!(
                    "explosion rate {:.1}% is {missed} the target band [{:.0}%, {:.0}%]",
                    stats.explosion_rate * 100.0,
                    thresholds.explosion_min * 100.0,
                    thresholds.explosion_max * 100.0,
                ),
            });
        }

        // A strategy winning disproportionately.
        for (strategy, &wins) in &stats.wins_by_strategy {
            let share = stats.strategy_win_share(strategy);
            if share > thresholds.dominant_win_share {
                smells.push(Smell {
                    kind: "dominant_strategy".into(),
                    detail: format!(
                        "strategy '{strategy}' won {:.1}% of games ({wins}/{}), above the {:.0}% dominance threshold",
                        share * 100.0,
                        stats.total_wins,
                        thresholds.dominant_win_share * 100.0,
                    ),
                });
            }
        }

        // A colour winning disproportionately.
        for (color, &wins) in &stats.wins_by_color {
            let share = stats.color_win_share(color);
            if share > thresholds.dominant_win_share {
                smells.push(Smell {
                    kind: "dominant_color".into(),
                    detail: format!(
                        "colour '{color}' won {:.1}% of games ({wins}/{}), above the {:.0}% dominance threshold",
                        share * 100.0,
                        stats.total_wins,
                        thresholds.dominant_win_share * 100.0,
                    ),
                });
            }
        }

        // Runaway pots.
        if stats.avg_pot_value > thresholds.runaway_pot_value {
            smells.push(Smell {
                kind: "runaway_pots".into(),
                detail: format!(
                    "average pot value {:.1} exceeds the runaway threshold {:.1}",
                    stats.avg_pot_value, thresholds.runaway_pot_value
                ),
            });
        }

        smells
    }
}

/// A complete balance report: the run's parameters, the aggregated statistics, and
/// any flagged smells — keyed to the config fingerprint under test.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Fingerprint of the content config the batch ran against.
    pub config_fingerprint: String,
    /// Root seed the batch was driven by.
    pub seed: u64,
    /// Transport backend used ("in-process" or "websocket").
    pub transport: String,
    /// Strategy assigned to each seat, in seat order.
    pub strategy_assignment: Vec<String>,
    /// The thresholds smell detection ran against.
    pub thresholds: Thresholds,
    /// Aggregated balance statistics.
    pub stats: BatchStats,
    /// Flagged balance smells (empty ⇒ nothing tripped a threshold).
    pub smells: Vec<Smell>,
}

impl Report {
    /// Build a report from a batch's statistics and run parameters.
    pub fn build(
        config_fingerprint: String,
        seed: u64,
        transport: &str,
        strategy_assignment: Vec<String>,
        stats: BatchStats,
        thresholds: Thresholds,
    ) -> Self {
        let smells = Smell::detect(&stats, &thresholds);
        Report {
            config_fingerprint,
            seed,
            transport: transport.to_string(),
            strategy_assignment,
            thresholds,
            stats,
            smells,
        }
    }

    /// Serialise to pretty JSON (the machine-readable artifact).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }

    /// Render the human-readable markdown summary.
    pub fn to_markdown(&self) -> String {
        let s = &self.stats;
        let mut out = String::new();
        out.push_str("# Boiling Point — Balance Report\n\n");
        out.push_str(&format!(
            "- Config fingerprint: `{}`\n",
            self.config_fingerprint
        ));
        out.push_str(&format!("- Seed: `{}`\n", self.seed));
        out.push_str(&format!("- Transport: {}\n", self.transport));
        out.push_str(&format!(
            "- Strategy assignment (by seat): {}\n",
            self.strategy_assignment.join(", ")
        ));
        out.push_str(&format!(
            "- Games: {} ({} completed)\n\n",
            s.games, s.completed_games
        ));

        out.push_str("## Core balance numbers\n\n");
        out.push_str(&format!(
            "- Explosion rate: **{:.1}%** ({} of {} rounds) — target {:.0}–{:.0}%\n",
            s.explosion_rate * 100.0,
            s.explosions,
            s.rounds,
            self.thresholds.explosion_min * 100.0,
            self.thresholds.explosion_max * 100.0,
        ));
        out.push_str(&format!(
            "- Avg pot value / round: {:.2}\n",
            s.avg_pot_value
        ));
        out.push_str(&format!(
            "- Avg cards / round: {:.2}\n",
            s.avg_cards_per_round
        ));
        out.push_str(&format!(
            "- Avg waves / round: {:.2}\n\n",
            s.avg_waves_per_round
        ));

        out.push_str("## Win distribution\n\n");
        out.push_str("By strategy:\n");
        for (strategy, wins) in &s.wins_by_strategy {
            out.push_str(&format!(
                "- {strategy}: {wins} ({:.1}%)\n",
                s.strategy_win_share(strategy) * 100.0
            ));
        }
        out.push_str("\nBy colour:\n");
        for (color, wins) in &s.wins_by_color {
            out.push_str(&format!(
                "- {color}: {wins} ({:.1}%)\n",
                s.color_win_share(color) * 100.0
            ));
        }

        out.push_str("\n## Modifier draws\n\n");
        if s.modifier_draws.is_empty() {
            out.push_str("- (none)\n");
        } else {
            for (modifier, count) in &s.modifier_draws {
                out.push_str(&format!("- {modifier}: {count}\n"));
            }
        }

        out.push_str("\n## Balance smells\n\n");
        if self.smells.is_empty() {
            out.push_str("- None — every metric is within its configured threshold.\n");
        } else {
            for smell in &self.smells {
                out.push_str(&format!("- ⚠️ **{}** — {}\n", smell.kind, smell.detail));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn stats_with(
        explosion_rate: f64,
        rounds: usize,
        wins_by_strategy: &[(&str, usize)],
        avg_pot_value: f64,
    ) -> BatchStats {
        let wins: BTreeMap<String, usize> = wins_by_strategy
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();
        let total: usize = wins.values().sum();
        BatchStats {
            games: total,
            completed_games: total,
            rounds,
            explosions: (explosion_rate * rounds as f64) as usize,
            explosion_rate,
            avg_pot_value,
            avg_cards_per_round: 6.0,
            avg_waves_per_round: 2.0,
            total_wins: total,
            wins_by_strategy: wins,
            wins_by_color: BTreeMap::new(),
            modifier_draws: BTreeMap::new(),
        }
    }

    /// A balanced batch trips no smell.
    #[test]
    fn clean_batch_has_no_smells() {
        let stats = stats_with(
            0.35,
            1000,
            &[
                ("cautious", 25),
                ("aggressor", 25),
                ("diplomat", 25),
                ("random", 25),
            ],
            10.0,
        );
        assert!(Smell::detect(&stats, &Thresholds::default()).is_empty());
    }

    /// A skewed (dominant) strategy is flagged with its numbers (task 7.2 path).
    #[test]
    fn dominant_strategy_is_flagged() {
        let stats = stats_with(
            0.35,
            1000,
            &[
                ("cautious", 5),
                ("aggressor", 90),
                ("diplomat", 3),
                ("random", 2),
            ],
            10.0,
        );
        let smells = Smell::detect(&stats, &Thresholds::default());
        assert!(
            smells
                .iter()
                .any(|s| s.kind == "dominant_strategy" && s.detail.contains("aggressor"))
        );
    }

    /// An off-target explosion rate is flagged and names the band it missed.
    #[test]
    fn off_target_explosion_rate_is_flagged() {
        let low = stats_with(0.05, 1000, &[("a", 1)], 10.0);
        let smells = Smell::detect(&low, &Thresholds::default());
        assert!(
            smells
                .iter()
                .any(|s| s.kind == "explosion_rate" && s.detail.contains("below"))
        );
    }

    /// The fingerprint is stable and content-sensitive.
    #[test]
    fn fingerprint_is_stable_and_sensitive() {
        assert_eq!(fingerprint("a = 1"), fingerprint("a = 1"));
        assert_ne!(fingerprint("a = 1"), fingerprint("a = 2"));
    }
}
