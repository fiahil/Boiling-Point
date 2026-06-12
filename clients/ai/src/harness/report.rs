//! Balance reports (tasks 6.2/6.3): one [`Report`] value rendered two ways —
//! human markdown for eyeballing and machine JSON for diffing across content
//! configs — with degenerate-strategy detection built in.
//!
//! Every report is keyed to a fingerprint of the content config under test;
//! the tuning loop is run → read smells → edit config → re-run → diff. Runs
//! containing an agent seat are marked **non-reproducible** (D7): the agent
//! brain sits outside the RNG tree.

use serde::Serialize;

use super::runner::{SampleRun, TransportKind};
use super::stats::CellStats;

/// A 64-bit FNV-1a fingerprint of the content config TOML, as hex.
pub fn fingerprint(config_toml: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in config_toml.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Tunable bounds for smell detection — starting hypotheses, all
/// `[needs playtesting]`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Thresholds {
    /// Lower edge of the acceptable explosion-rate band.
    pub explosion_min: f64,
    /// Upper edge of the acceptable explosion-rate band.
    pub explosion_max: f64,
    /// All-pass round rate above which a cell is flagged as freezing.
    pub freeze_max: f64,
    /// Win share above which a single label dominates its cell.
    pub dominant_win_share: f64,
    /// A non-random label whose win share sits within this of the random
    /// baseline's (same cell, enough games) is indistinguishable from noise.
    pub random_indistinguishable_margin: f64,
    /// Games a cell needs before the random-baseline comparison fires.
    pub random_comparison_min_games: usize,
    /// Average scored pot value above which pots are flagged as runaway.
    pub runaway_pot_value: f64,
    /// Per-seat win-rate band for a Brewer (baseline 0.25 at a 4-seat table):
    /// outside it the Brewer is flagged as breaking or crippled.
    pub brewer_win_rate_min: f64,
    /// Upper edge of the per-Brewer win-rate band.
    pub brewer_win_rate_max: f64,
    /// Seat-games a Brewer needs before its win-rate band fires.
    pub brewer_min_games: usize,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            explosion_min: 0.40,
            explosion_max: 0.50,
            freeze_max: 0.35,
            dominant_win_share: 0.40,
            random_indistinguishable_margin: 0.05,
            random_comparison_min_games: 200,
            // ~2.5× the sketched P≈10 — known to fire on the current content
            // (the standing fat-pots finding, docs/06_boom2/02).
            runaway_pot_value: 25.0,
            // ±10pp around the 25% per-seat baseline. `[needs playtesting]`.
            brewer_win_rate_min: 0.15,
            brewer_win_rate_max: 0.35,
            brewer_min_games: 200,
        }
    }
}

/// One flagged balance smell, with the numbers that triggered it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Smell {
    /// The cell the smell was found in.
    pub cell: String,
    /// A short machine-stable category.
    pub kind: String,
    /// A human-readable explanation including the numbers.
    pub detail: String,
}

/// One cell's slice of the report.
#[derive(Debug, Clone, Serialize)]
pub struct CellReport {
    /// The cell's name from the spec.
    pub name: String,
    /// The seat labels, in seating order.
    pub seats: Vec<String>,
    /// Aggregated statistics.
    pub stats: CellStats,
}

/// A complete balance report.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Fingerprint of the content config the batch ran against.
    pub config_fingerprint: String,
    /// Root seed the batch was driven by.
    pub root_seed: u64,
    /// Transport backend.
    pub transport: String,
    /// False when an agent seat took part — same seed will NOT reproduce.
    pub reproducible: bool,
    /// Brewer/deck-archetype axis status (lands with later boom2 changes).
    pub pending_axes: &'static str,
    /// The thresholds smell detection ran against.
    pub thresholds: Thresholds,
    /// Per-cell statistics, in spec order.
    pub cells: Vec<CellReport>,
    /// Flagged balance smells across all cells.
    pub smells: Vec<Smell>,
}

/// Detect a cell's smells (explosion band, dominance, freezes, random-floor).
fn detect(cell: &CellReport, thresholds: &Thresholds) -> Vec<Smell> {
    let mut smells = Vec::new();
    let stats = &cell.stats;
    if stats.rounds > 0
        && (stats.explosion_rate < thresholds.explosion_min
            || stats.explosion_rate > thresholds.explosion_max)
    {
        let side = if stats.explosion_rate < thresholds.explosion_min {
            "below"
        } else {
            "above"
        };
        smells.push(Smell {
            cell: cell.name.clone(),
            kind: "explosion_rate".into(),
            detail: format!(
                "explosion rate {:.1}% is {side} the target band [{:.0}%, {:.0}%]",
                stats.explosion_rate * 100.0,
                thresholds.explosion_min * 100.0,
                thresholds.explosion_max * 100.0,
            ),
        });
    }
    for (label, l) in &stats.by_label {
        let share = stats.win_share(label);
        if share > thresholds.dominant_win_share {
            smells.push(Smell {
                cell: cell.name.clone(),
                kind: "dominant_label".into(),
                detail: format!(
                    "'{label}' won {:.1}% of credits ({} of {}), above the {:.0}% dominance threshold",
                    share * 100.0,
                    l.wins,
                    stats.total_wins,
                    thresholds.dominant_win_share * 100.0,
                ),
            });
        }
    }
    // Seat-colour fairness: a colour winning disproportionately (labels are
    // fixed per seat, so this is the seating-order-bias signal).
    for (color, wins) in &stats.wins_by_color {
        let share = stats.color_win_share(color);
        if share > thresholds.dominant_win_share {
            smells.push(Smell {
                cell: cell.name.clone(),
                kind: "dominant_color".into(),
                detail: format!(
                    "colour '{color}' won {:.1}% of credits ({wins} of {}), above the {:.0}% dominance threshold",
                    share * 100.0,
                    stats.total_wins,
                    thresholds.dominant_win_share * 100.0,
                ),
            });
        }
    }
    // Runaway pots (the standing fat-pots finding — expected to fire on the
    // current content until the points-curve work lands).
    if stats.rounds > 0 && stats.avg_pot_value > thresholds.runaway_pot_value {
        smells.push(Smell {
            cell: cell.name.clone(),
            kind: "runaway_pots".into(),
            detail: format!(
                "average scored pot value {:.1} exceeds the runaway threshold {:.1}",
                stats.avg_pot_value, thresholds.runaway_pot_value
            ),
        });
    }
    // The random baseline as a floor: a heuristic statistically level with
    // uniform-random play is itself a finding (design risk: bot competence).
    if stats.games >= thresholds.random_comparison_min_games
        && let Some(random_share) = stats
            .by_label
            .keys()
            .find(|l| l.starts_with("random"))
            .map(|l| stats.win_share(l))
    {
        for label in stats.by_label.keys().filter(|l| !l.starts_with("random")) {
            let share = stats.win_share(label);
            if (share - random_share).abs() <= thresholds.random_indistinguishable_margin {
                smells.push(Smell {
                    cell: cell.name.clone(),
                    kind: "indistinguishable_from_random".into(),
                    detail: format!(
                        "'{label}' wins {:.1}% vs the random baseline's {:.1}% — within ±{:.0}pp over {} games",
                        share * 100.0,
                        random_share * 100.0,
                        thresholds.random_indistinguishable_margin * 100.0,
                        stats.games,
                    ),
                });
            }
        }
    }
    // The mutual-balance gate (`boom2-brewers`): a Brewer whose cross-label
    // per-seat win rate sits outside the band, with enough games behind it, is
    // breaking (or crippled) — the matrix below names which persona it breaks.
    for (brewer, totals) in stats.brewer_totals() {
        if totals.games < thresholds.brewer_min_games {
            continue;
        }
        let rate = totals.win_rate();
        if rate > thresholds.brewer_win_rate_max || rate < thresholds.brewer_win_rate_min {
            let side = if rate > thresholds.brewer_win_rate_max {
                "above"
            } else {
                "below"
            };
            smells.push(Smell {
                cell: cell.name.clone(),
                kind: "brewer_outlier".into(),
                detail: format!(
                    "Brewer '{brewer}' wins {:.1}% of its {} seated games — {side} the [{:.0}%, {:.0}%] band around the 25% seat baseline",
                    rate * 100.0,
                    totals.games,
                    thresholds.brewer_win_rate_min * 100.0,
                    thresholds.brewer_win_rate_max * 100.0,
                ),
            });
        }
    }
    if stats.rounds > 0 && stats.all_pass_rate > thresholds.freeze_max {
        smells.push(Smell {
            cell: cell.name.clone(),
            kind: "frozen_rounds".into(),
            detail: format!(
                "{:.1}% of rounds ended on an all-pass wave ({} of {}), above the {:.0}% freeze threshold",
                stats.all_pass_rate * 100.0,
                stats.all_pass_rounds,
                stats.rounds,
                thresholds.freeze_max * 100.0,
            ),
        });
    }
    if stats.empty_pot_rounds > 0 {
        smells.push(Smell {
            cell: cell.name.clone(),
            kind: "empty_pot_rounds".into(),
            detail: format!(
                "{} round(s) settled with an empty pot — nobody played a single ingredient",
                stats.empty_pot_rounds
            ),
        });
    }
    smells
}

impl Report {
    /// Build the report for a completed run.
    pub fn build(run: &SampleRun, config_toml: &str, thresholds: Thresholds) -> Self {
        let cells: Vec<CellReport> = run
            .cells
            .iter()
            .map(|cell| CellReport {
                name: cell.spec.name.clone(),
                seats: cell.spec.seats.iter().map(|s| s.brain.label()).collect(),
                stats: CellStats::aggregate(cell),
            })
            .collect();
        let smells = cells.iter().flat_map(|c| detect(c, &thresholds)).collect();
        Report {
            config_fingerprint: fingerprint(config_toml),
            root_seed: run.root_seed,
            transport: run.transport.label().to_string(),
            reproducible: !run.agent_seats_present && run.transport == TransportKind::InProcess,
            pending_axes: "deck-archetype axis pending boom2-apothecary (persona and Brewer axes live)",
            thresholds,
            cells,
            smells,
        }
    }

    /// Serialise to pretty JSON (the diffable artifact).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }

    /// Render the human-readable markdown summary.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Boiling Point — Balance Report (boom2 AI client)\n\n");
        out.push_str(&format!(
            "- Config fingerprint: `{}`\n- Root seed: `{}`\n- Transport: {}\n",
            self.config_fingerprint, self.root_seed, self.transport
        ));
        out.push_str(&format!(
            "- Reproducible: {}\n",
            if self.reproducible {
                "yes (bot brains only, in-process)".to_string()
            } else {
                "**NO** — agent seats and/or the real wire took part".to_string()
            }
        ));
        out.push_str(&format!("- Axes: {}\n", self.pending_axes));

        for cell in &self.cells {
            let s = &cell.stats;
            out.push_str(&format!(
                "\n## Cell `{}` — {} games ({})\n\n",
                cell.name,
                s.games,
                cell.seats.join(" / ")
            ));
            out.push_str(&format!(
                "- Explosion rate: **{:.1}%** ({} of {} rounds) — target {:.0}–{:.0}%\n",
                s.explosion_rate * 100.0,
                s.explosions,
                s.rounds,
                self.thresholds.explosion_min * 100.0,
                self.thresholds.explosion_max * 100.0,
            ));
            out.push_str(&format!(
                "- Detonators per explosion: {:.2}\n- All-pass endings: {:.1}% ({} empty-pot)\n",
                s.avg_detonators_per_explosion,
                s.all_pass_rate * 100.0,
                s.empty_pot_rounds,
            ));
            out.push_str(&format!(
                "- Peek casts/game: {:.2} — spell casts/round: {:.2} — fold-to-safety/round: {:.2}\n",
                s.peek_casts_per_game, s.spell_casts_per_round, s.fold_to_safety_per_round,
            ));
            out.push_str(&format!(
                "- Avg pot value {:.2}, cards {:.2}, waves {:.2} per round\n",
                s.avg_pot_value, s.avg_cards_per_round, s.avg_waves_per_round,
            ));
            out.push_str(
                "\n| seat label | wins | win share | detonations | folded safe | fallback rate |\n",
            );
            out.push_str("|---|---|---|---|---|---|\n");
            for (label, l) in &s.by_label {
                out.push_str(&format!(
                    "| {label} | {} | {:.1}% | {} | {} | {:.2}% |\n",
                    l.wins,
                    s.win_share(label) * 100.0,
                    l.detonations,
                    l.folded_safe,
                    l.fallback_rate() * 100.0,
                ));
            }
            if !s.wins_by_color.is_empty() {
                let colors: Vec<String> = s
                    .wins_by_color
                    .iter()
                    .map(|(color, wins)| {
                        format!("{color} {wins} ({:.1}%)", s.color_win_share(color) * 100.0)
                    })
                    .collect();
                out.push_str(&format!("\nWins by seat colour: {}.\n", colors.join(", ")));
            }
            if !s.brewer_matrix.is_empty() {
                out.push_str(
                    "\n### Persona × Brewer (actual picks; win rate vs the 25% seat baseline)\n\n",
                );
                out.push_str("| persona | brewer | games | wins | win rate | detonations |\n");
                out.push_str("|---|---|---|---|---|---|\n");
                for (label, brewers) in &s.brewer_matrix {
                    for (brewer, cell) in brewers {
                        out.push_str(&format!(
                            "| {label} | {brewer} | {} | {} | {:.1}% | {} |\n",
                            cell.games,
                            cell.wins,
                            cell.win_rate() * 100.0,
                            cell.detonations,
                        ));
                    }
                }
                let totals: Vec<String> = s
                    .brewer_totals()
                    .iter()
                    .map(|(brewer, t)| {
                        format!("{brewer} {:.1}% ({} games)", t.win_rate() * 100.0, t.games)
                    })
                    .collect();
                out.push_str(&format!(
                    "\nPer-Brewer win rates across personas: {}.\n",
                    totals.join(", ")
                ));
            }
        }

        out.push_str("\n## Balance smells\n\n");
        if self.smells.is_empty() {
            out.push_str("- None — every metric is within its configured threshold.\n");
        } else {
            for smell in &self.smells {
                out.push_str(&format!(
                    "- ⚠️ `{}` **{}** — {}\n",
                    smell.cell, smell.kind, smell.detail
                ));
            }
        }
        out
    }
}
