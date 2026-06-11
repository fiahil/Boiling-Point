//! `boom-balance-metrics`: the single source of the v2 balance metric
//! definitions — each metric defined **exactly once** (id, formula, unit,
//! `[needs playtesting]` target) and consumed by every population that measures
//! the game:
//!
//! - the **live pipeline** — the Prometheus emitters
//!   ([`crate::observability::metric`]) bump the [`series`] this module names, and
//!   the admin projection folds completed spans into an [`Accumulator`] and
//!   evaluates [`definitions`] for the balance dashboard;
//! - the **benchmarking suite** (`boom2-benchmarking`) — its balance-study runner
//!   links this crate and evaluates the same definitions over bot games, so "does
//!   live play match the harness?" is a direct comparison, never a reconciliation.
//!
//! Definitions are data plus pure functions over [`BalanceEvent`]s — no I/O.
//! Neither consumer re-derives a formula; both feed events and read values.
//!
//! Targets are **playtest hypotheses** (Constitution IV): every target is tagged
//! [`NEEDS_PLAYTESTING`], seeded from the decision log's starting numbers
//! (`docs/06_boom2/02_toward-a-v2-core.md`) where one exists and absent
//! otherwise. Targets are updated here from balance-study results, never
//! hand-tuned in a dashboard.

use std::collections::BTreeMap;

use serde::Serialize;

use super::span_schema::{attr, span};

/// The standing status of every balance target until a study validates it.
pub const NEEDS_PLAYTESTING: &str = "needs playtesting";

/// What a metric's value is denominated in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// A fraction in `[0, 1]`.
    Ratio,
    /// A per-round average.
    PerRound,
    /// A per-game average.
    PerGame,
    /// A per-boom average.
    PerBoom,
    /// Seconds.
    Seconds,
}

/// A `[needs playtesting]` working target: a single working value or a band.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// A single working value to compare against.
    Point {
        /// The working value.
        value: f64,
    },
    /// An inclusive healthy band.
    Band {
        /// Lower bound.
        lo: f64,
        /// Upper bound.
        hi: f64,
    },
}

/// One observed v2 engine outcome, the shared event currency every consumer
/// feeds into an [`Accumulator`]. The live pipeline derives these from completed
/// spans ([`event_from_span`]); balance studies derive them from engine outcomes.
#[derive(Debug, Clone, PartialEq)]
pub enum BalanceEvent {
    /// A game completed.
    GameCompleted {
        /// Game duration in milliseconds.
        duration_ms: u64,
    },
    /// A round settled (boom or safe).
    RoundSettled {
        /// Whether the round boomed (detonator-only explosion).
        boomed: bool,
        /// Whether the round froze (settled with an empty pot — everyone passed).
        frozen: bool,
        /// Round duration in milliseconds.
        duration_ms: u64,
    },
    /// A boom's detonator split (who ate −P).
    Boom {
        /// Number of detonators that split the pot.
        detonators: u64,
    },
    /// A wave resolved.
    WaveResolved {
        /// Whether the commit window closed on its timer.
        timed_out: bool,
        /// Ingredients committed this wave.
        commits: u64,
        /// Players who passed/folded this wave.
        folds: u64,
        /// Wave duration in milliseconds.
        duration_ms: u64,
    },
    /// A spell was cast (visible activation).
    SpellCast {
        /// The spell kind's name.
        kind: String,
    },
    /// A player reconnected mid-game.
    PlayerReconnected,
}

/// Map one **completed** v2 span onto its balance event, if it carries one. This
/// is the single span→metrics seam: the projection calls it on span close, so the
/// aggregates and the definitions can never disagree about what a span means.
pub fn event_from_span(
    name: &str,
    attrs: &BTreeMap<String, String>,
    duration_ms: u64,
) -> Option<BalanceEvent> {
    let is_true = |key: &str| attrs.get(key).map(String::as_str) == Some("true");
    let count = |key: &str| {
        attrs
            .get(key)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    match name {
        span::GAME => Some(BalanceEvent::GameCompleted { duration_ms }),
        span::ROUND => Some(BalanceEvent::RoundSettled {
            boomed: is_true(attr::ROUND_BOOMED),
            frozen: is_true(attr::ROUND_FROZEN),
            duration_ms,
        }),
        span::SCORE if is_true(attr::ROUND_BOOMED) => Some(BalanceEvent::Boom {
            detonators: attrs
                .get(attr::DETONATORS)
                .map(|csv| csv.split(',').filter(|s| !s.is_empty()).count() as u64)
                .unwrap_or(0),
        }),
        span::WAVE => Some(BalanceEvent::WaveResolved {
            timed_out: is_true(attr::WAVE_TIMED_OUT),
            commits: count(attr::WAVE_COMMITS),
            folds: count(attr::WAVE_PASSES),
            duration_ms,
        }),
        span::SPELL_CAST => Some(BalanceEvent::SpellCast {
            kind: attrs.get(attr::SPELL_KIND).cloned().unwrap_or_default(),
        }),
        span::RECONNECT => Some(BalanceEvent::PlayerReconnected),
        _ => None,
    }
}

/// Raw counters folded from [`BalanceEvent`]s — the state every definition
/// evaluates over. Pure accumulation; no I/O.
#[derive(Debug, Default, Clone)]
pub struct Accumulator {
    /// Completed games.
    pub games: u64,
    /// Total game time, ms.
    pub total_game_ms: u64,
    /// Settled rounds.
    pub rounds: u64,
    /// Rounds that boomed.
    pub booms: u64,
    /// Rounds that froze (empty pot).
    pub frozen: u64,
    /// Total round time, ms.
    pub total_round_ms: u64,
    /// Resolved waves.
    pub waves: u64,
    /// Waves that closed on their timer.
    pub wave_timeouts: u64,
    /// Total wave time, ms.
    pub total_wave_ms: u64,
    /// Ingredients committed.
    pub commits: u64,
    /// Passes/folds.
    pub folds: u64,
    /// Detonators across all booms.
    pub detonators: u64,
    /// Spell casts (visible activations).
    pub spell_casts: u64,
    /// Spell casts by kind.
    pub casts_by_kind: BTreeMap<String, u64>,
    /// Mid-game reconnections.
    pub reconnects: u64,
}

impl Accumulator {
    /// Fold one event in.
    pub fn record(&mut self, ev: &BalanceEvent) {
        match ev {
            BalanceEvent::GameCompleted { duration_ms } => {
                self.games += 1;
                self.total_game_ms += duration_ms;
            }
            BalanceEvent::RoundSettled {
                boomed,
                frozen,
                duration_ms,
            } => {
                self.rounds += 1;
                self.booms += u64::from(*boomed);
                self.frozen += u64::from(*frozen);
                self.total_round_ms += duration_ms;
            }
            BalanceEvent::Boom { detonators } => self.detonators += detonators,
            BalanceEvent::WaveResolved {
                timed_out,
                commits,
                folds,
                duration_ms,
            } => {
                self.waves += 1;
                self.wave_timeouts += u64::from(*timed_out);
                self.commits += commits;
                self.folds += folds;
                self.total_wave_ms += duration_ms;
            }
            BalanceEvent::SpellCast { kind } => {
                self.spell_casts += 1;
                *self.casts_by_kind.entry(kind.clone()).or_insert(0) += 1;
            }
            BalanceEvent::PlayerReconnected => self.reconnects += 1,
        }
    }

    /// Per-spell cast rates (casts of each kind / settled rounds), for the
    /// per-spell panel. Empty until rounds settle.
    pub fn per_spell_cast_rates(&self) -> Vec<(String, f64)> {
        if self.rounds == 0 {
            return Vec::new();
        }
        self.casts_by_kind
            .iter()
            .map(|(kind, n)| (kind.clone(), *n as f64 / self.rounds as f64))
            .collect()
    }
}

/// One named balance metric: its id, display label, unit, optional
/// `[needs playtesting]` target, and its formula over the accumulated events.
pub struct Definition {
    /// Stable metric id (versioned by the v2 set — never collides with a v1 id).
    pub id: &'static str,
    /// Human label for panels.
    pub label: &'static str,
    /// The value's unit.
    pub unit: Unit,
    /// The working target, when the decision log seeds one (always
    /// [`NEEDS_PLAYTESTING`] until a balance study validates it).
    pub target: Option<Target>,
    /// The formula. `None` when the denominator population is empty.
    eval: fn(&Accumulator) -> Option<f64>,
}

impl Definition {
    /// Evaluate this definition over `acc`. `None` until its population exists.
    pub fn evaluate(&self, acc: &Accumulator) -> Option<f64> {
        (self.eval)(acc)
    }
}

/// `n / d`, or `None` for an empty population.
fn ratio(n: u64, d: u64) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}

/// Mean seconds from a millisecond total, or `None` for an empty population.
fn mean_secs(total_ms: u64, d: u64) -> Option<f64> {
    (d > 0).then(|| total_ms as f64 / d as f64 / 1000.0)
}

/// The core v2 balance metric definitions, in display order. Per-feature metrics
/// (per-Brewer pick/win rates, bucket pick rates, compounding trigger rates)
/// extend this list **additively** as `boom2-brewers` / `boom2-apothecary` /
/// `boom2-compounding` land — existing ids are never renamed or redefined.
///
/// The carried-over fleet figures (active games/groups, connected players,
/// groups/games counters) are live gauges sourced from the open-span registry and
/// the [`series`] gauges, not foldable definitions; see [`series`].
pub const DEFINITIONS: &[Definition] = &[
    Definition {
        id: "boom_rate",
        label: "Boom rate",
        unit: Unit::Ratio,
        // Decision log: "~45% of rounds"; the first harness derivation measured
        // 44.8% over 2000 seeded games at the adopted BP 31–43 window.
        target: Some(Target::Point { value: 0.45 }),
        eval: |a| ratio(a.booms, a.rounds),
    },
    Definition {
        id: "freeze_rate",
        label: "Freeze (all-pass) rate",
        unit: Unit::Ratio,
        // Decision log: rounds "must not freeze"; the harness measured 0.0%.
        target: Some(Target::Point { value: 0.0 }),
        eval: |a| ratio(a.frozen, a.rounds),
    },
    Definition {
        id: "detonators_per_boom",
        label: "Detonators per boom",
        unit: Unit::PerBoom,
        target: None,
        eval: |a| ratio(a.detonators, a.booms),
    },
    Definition {
        id: "fold_rate",
        label: "Fold rate",
        unit: Unit::Ratio,
        target: None,
        eval: |a| ratio(a.folds, a.commits + a.folds),
    },
    Definition {
        id: "wave_depth",
        label: "Waves per round",
        unit: Unit::PerRound,
        target: None,
        eval: |a| ratio(a.waves, a.rounds),
    },
    Definition {
        id: "wave_duration_seconds",
        label: "Avg wave duration",
        unit: Unit::Seconds,
        target: None,
        eval: |a| mean_secs(a.total_wave_ms, a.waves),
    },
    Definition {
        id: "round_duration_seconds",
        label: "Avg round duration",
        unit: Unit::Seconds,
        // Decision log: "5 × ~2.5 min" rounds.
        target: Some(Target::Point { value: 150.0 }),
        eval: |a| mean_secs(a.total_round_ms, a.rounds),
    },
    Definition {
        id: "game_duration_seconds",
        label: "Avg game duration",
        unit: Unit::Seconds,
        // Decision log: "≈ 15–18 min" per game (C2).
        target: Some(Target::Band {
            lo: 900.0,
            hi: 1080.0,
        }),
        eval: |a| mean_secs(a.total_game_ms, a.games),
    },
    Definition {
        id: "spell_cast_rate",
        label: "Spell casts per round",
        unit: Unit::PerRound,
        target: None,
        eval: |a| ratio(a.spell_casts, a.rounds),
    },
    Definition {
        id: "wave_timeout_rate",
        label: "Wave timeout rate",
        unit: Unit::Ratio,
        target: None,
        eval: |a| ratio(a.wave_timeouts, a.waves),
    },
    Definition {
        id: "reconnection_rate",
        label: "Reconnections per game",
        unit: Unit::PerGame,
        target: None,
        eval: |a| ratio(a.reconnects, a.games),
    },
];

/// All definitions, in display order.
pub fn definitions() -> &'static [Definition] {
    DEFINITIONS
}

/// Look up one definition by id.
pub fn definition(id: &str) -> Option<&'static Definition> {
    DEFINITIONS.iter().find(|d| d.id == id)
}

/// One evaluated metric, as both the balance dashboard and study reports render
/// it: id, value (absent until its population exists), and the target band
/// (absent ⇒ render the observed value with no band).
#[derive(Debug, Clone, Serialize)]
pub struct MetricValue {
    /// The definition's id.
    pub id: &'static str,
    /// The definition's display label.
    pub label: &'static str,
    /// The value's unit.
    pub unit: Unit,
    /// The evaluated value, when its population exists.
    pub value: Option<f64>,
    /// The working target, when seeded.
    pub target: Option<Target>,
    /// [`NEEDS_PLAYTESTING`] whenever a target is present.
    pub target_status: Option<&'static str>,
}

/// Evaluate every definition over `acc`, in display order.
pub fn evaluate_all(acc: &Accumulator) -> Vec<MetricValue> {
    DEFINITIONS
        .iter()
        .map(|d| MetricValue {
            id: d.id,
            label: d.label,
            unit: d.unit,
            value: d.evaluate(acc),
            target: d.target,
            target_status: d.target.map(|_| NEEDS_PLAYTESTING),
        })
        .collect()
}

/// The Prometheus series the live emitter writes, named here so the emitters and
/// the Grafana dashboard reference one vocabulary. The v2 balance series carry
/// **new ids** (never colliding with retired v1 series); the fleet/ops series
/// carry over from v1 with unchanged identity and meaning.
pub mod series {
    // --- fleet/ops (carried over from v1, identity unchanged) ---
    /// Groups created (counter).
    pub const GROUPS_CREATED: &str = "groups_created_total";
    /// Live groups (gauge).
    pub const GROUPS_ACTIVE: &str = "groups_active";
    /// Games started (counter).
    pub const GAMES_STARTED: &str = "games_started_total";
    /// Games in progress (gauge).
    pub const GAMES_ACTIVE: &str = "games_active";
    /// Games completed (counter).
    pub const GAMES_COMPLETED: &str = "games_completed_total";
    /// Connected players (gauge).
    pub const PLAYERS_CONNECTED: &str = "players_connected";
    /// Mid-game reconnections (counter).
    pub const PLAYER_RECONNECTS: &str = "player_reconnects_total";
    /// Waves resolved (counter).
    pub const WAVES: &str = "waves_total";
    /// Waves that closed on their timer (counter).
    pub const WAVE_TIMEOUTS: &str = "wave_timeouts_total";
    /// Rounds settled (counter).
    pub const ROUNDS: &str = "rounds_total";
    /// Round duration (histogram, seconds).
    pub const ROUND_DURATION: &str = "round_duration_seconds";
    /// Game duration (histogram, seconds).
    pub const GAME_DURATION: &str = "game_duration_seconds";

    // --- v2 balance series (new ids) ---
    /// Rounds that boomed (counter) — `boom_rate`'s numerator.
    pub const ROUNDS_BOOMED: &str = "rounds_boomed_total";
    /// Rounds that froze (counter) — `freeze_rate`'s numerator.
    pub const ROUNDS_FROZEN: &str = "rounds_frozen_total";
    /// Detonators across booms (counter) — `detonators_per_boom`'s numerator.
    pub const BOOM_DETONATORS: &str = "boom_detonators_total";
    /// Ingredients committed (counter) — `fold_rate`'s co-denominator.
    pub const WAVE_COMMITS: &str = "wave_commits_total";
    /// Passes/folds (counter) — `fold_rate`'s numerator.
    pub const WAVE_FOLDS: &str = "wave_folds_total";
    /// Spell casts (counter, labelled by `spell`) — `spell_cast_rate`.
    pub const SPELL_CASTS: &str = "spell_casts_total";
    /// The [`SPELL_CASTS`] label key.
    pub const SPELL_LABEL: &str = "spell";
    /// Wave duration (histogram, seconds).
    pub const WAVE_DURATION: &str = "wave_duration_seconds";
}

/// The v1 balance series, retired atomically with the v1 core (their populations
/// no longer exist). They must never be emitted again; historical data stays in
/// Prometheus storage untouched.
pub const RETIRED_V1_SERIES: &[&str] = &[
    "round_explosions_total",
    "round_dominations_total",
    "round_splits_total",
    "cards_committed_total",
    "deck_reshuffles_total",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic v2 event stream with known answers.
    fn synthetic() -> Accumulator {
        let mut acc = Accumulator::default();
        // 2 games, 10 rounds: 4 boomed (6 detonators total), 1 froze.
        for _ in 0..2 {
            acc.record(&BalanceEvent::GameCompleted {
                duration_ms: 960_000,
            });
        }
        for i in 0..10u64 {
            acc.record(&BalanceEvent::RoundSettled {
                boomed: i < 4,
                frozen: i == 9,
                duration_ms: 150_000,
            });
        }
        for detonators in [1, 2, 2, 1] {
            acc.record(&BalanceEvent::Boom { detonators });
        }
        // 50 waves: 5 timed out, 30 commits, 10 folds.
        for i in 0..50u64 {
            acc.record(&BalanceEvent::WaveResolved {
                timed_out: i < 5,
                commits: u64::from(i < 30),
                folds: u64::from(i >= 40),
                duration_ms: 20_000,
            });
        }
        // 15 casts: 10 Peek, 5 Surge.
        for i in 0..15 {
            acc.record(&BalanceEvent::SpellCast {
                kind: if i < 10 { "Peek" } else { "Surge" }.into(),
            });
        }
        acc.record(&BalanceEvent::PlayerReconnected);
        acc
    }

    fn value(acc: &Accumulator, id: &str) -> f64 {
        definition(id)
            .unwrap_or_else(|| panic!("definition {id} exists"))
            .evaluate(acc)
            .unwrap_or_else(|| panic!("{id} evaluates over the synthetic stream"))
    }

    /// Every core definition evaluates the documented formula over a synthetic
    /// v2 event stream.
    #[test]
    fn definitions_evaluate_the_documented_formulas() {
        let acc = synthetic();
        assert_eq!(value(&acc, "boom_rate"), 0.4);
        assert_eq!(value(&acc, "freeze_rate"), 0.1);
        assert_eq!(value(&acc, "detonators_per_boom"), 1.5);
        assert_eq!(value(&acc, "fold_rate"), 10.0 / 40.0);
        assert_eq!(value(&acc, "wave_depth"), 5.0);
        assert_eq!(value(&acc, "wave_duration_seconds"), 20.0);
        assert_eq!(value(&acc, "round_duration_seconds"), 150.0);
        assert_eq!(value(&acc, "game_duration_seconds"), 960.0);
        assert_eq!(value(&acc, "spell_cast_rate"), 1.5);
        assert_eq!(value(&acc, "wave_timeout_rate"), 0.1);
        assert_eq!(value(&acc, "reconnection_rate"), 0.5);
        // The per-spell breakdown shares the same population.
        let per_spell = acc.per_spell_cast_rates();
        assert_eq!(per_spell, vec![("Peek".into(), 1.0), ("Surge".into(), 0.5)]);
    }

    /// Boom rate counts the v2 detonator-only boom per round, not anything else:
    /// only `RoundSettled { boomed: true }` moves it.
    #[test]
    fn boom_rate_counts_boomed_rounds_only() {
        let mut acc = Accumulator::default();
        acc.record(&BalanceEvent::RoundSettled {
            boomed: true,
            frozen: false,
            duration_ms: 0,
        });
        acc.record(&BalanceEvent::RoundSettled {
            boomed: false,
            frozen: false,
            duration_ms: 0,
        });
        // A Boom event carries the detonator split; it is not a second boom.
        acc.record(&BalanceEvent::Boom { detonators: 2 });
        assert_eq!(value(&acc, "boom_rate"), 0.5);
    }

    /// An empty population evaluates to `None` (rendered as "no data", never 0%
    /// or a division panic), and a missing target stays absent.
    #[test]
    fn empty_population_yields_no_value() {
        let acc = Accumulator::default();
        for m in evaluate_all(&acc) {
            assert!(m.value.is_none(), "{} must be None with no data", m.id);
        }
    }

    /// Targets are seeded from the decision log and tagged as hypotheses; metrics
    /// the log carries no number for ship without a target band.
    #[test]
    fn targets_are_seeded_from_the_decision_log_and_tagged() {
        let acc = synthetic();
        let all = evaluate_all(&acc);
        let by_id = |id: &str| all.iter().find(|m| m.id == id).unwrap();

        assert_eq!(
            by_id("boom_rate").target,
            Some(Target::Point { value: 0.45 })
        );
        assert_eq!(
            by_id("game_duration_seconds").target,
            Some(Target::Band {
                lo: 900.0,
                hi: 1080.0
            })
        );
        // Every present target is a [needs playtesting] hypothesis.
        for m in &all {
            assert_eq!(
                m.target_status,
                m.target.map(|_| NEEDS_PLAYTESTING),
                "{}: a target is always tagged {NEEDS_PLAYTESTING}",
                m.id
            );
        }
        // No starting number in the log ⇒ no band.
        for id in ["detonators_per_boom", "fold_rate", "wave_depth"] {
            assert!(by_id(id).target.is_none(), "{id} has no seeded target");
        }
    }

    /// Span→event mapping: the projection's fold derives exactly these events
    /// from completed v2 spans.
    #[test]
    fn events_map_from_completed_spans() {
        let attrs = |kv: &[(&str, &str)]| -> BTreeMap<String, String> {
            kv.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };

        assert_eq!(
            event_from_span(
                span::ROUND,
                &attrs(&[(attr::ROUND_BOOMED, "true"), (attr::ROUND_FROZEN, "false")]),
                1500,
            ),
            Some(BalanceEvent::RoundSettled {
                boomed: true,
                frozen: false,
                duration_ms: 1500,
            })
        );
        assert_eq!(
            event_from_span(
                span::SCORE,
                &attrs(&[(attr::ROUND_BOOMED, "true"), (attr::DETONATORS, "p1,p2")]),
                10,
            ),
            Some(BalanceEvent::Boom { detonators: 2 })
        );
        // A safe round's score span carries no boom event.
        assert_eq!(
            event_from_span(span::SCORE, &attrs(&[(attr::ROUND_BOOMED, "false")]), 10),
            None
        );
        assert_eq!(
            event_from_span(
                span::WAVE,
                &attrs(&[
                    (attr::WAVE_TIMED_OUT, "true"),
                    (attr::WAVE_COMMITS, "3"),
                    (attr::WAVE_PASSES, "1"),
                ]),
                20_000,
            ),
            Some(BalanceEvent::WaveResolved {
                timed_out: true,
                commits: 3,
                folds: 1,
                duration_ms: 20_000,
            })
        );
        assert_eq!(
            event_from_span(span::SPELL_CAST, &attrs(&[(attr::SPELL_KIND, "Peek")]), 5),
            Some(BalanceEvent::SpellCast {
                kind: "Peek".into()
            })
        );
        // Spans that carry no balance meaning map to nothing.
        assert_eq!(event_from_span(span::COMMIT, &attrs(&[]), 5), None);
        assert_eq!(event_from_span(span::HAND, &attrs(&[]), 5), None);
    }

    /// The fleet/ops series carry over the v1↔v2 cutover with their identity
    /// unchanged — operators keep one continuous set of charts through the swap.
    #[test]
    fn fleet_series_identities_are_unchanged_across_the_cutover() {
        assert_eq!(series::GROUPS_CREATED, "groups_created_total");
        assert_eq!(series::GROUPS_ACTIVE, "groups_active");
        assert_eq!(series::GAMES_STARTED, "games_started_total");
        assert_eq!(series::GAMES_ACTIVE, "games_active");
        assert_eq!(series::GAMES_COMPLETED, "games_completed_total");
        assert_eq!(series::PLAYER_RECONNECTS, "player_reconnects_total");
        assert_eq!(series::WAVES, "waves_total");
        assert_eq!(series::WAVE_TIMEOUTS, "wave_timeouts_total");
        assert_eq!(series::ROUNDS, "rounds_total");
        assert_eq!(series::ROUND_DURATION, "round_duration_seconds");
        assert_eq!(series::GAME_DURATION, "game_duration_seconds");
    }

    /// No v1 metric id survives in the v2 vocabulary: neither the definition ids
    /// nor the Prometheus series reuse a retired id.
    #[test]
    fn no_v1_metric_ids_in_the_v2_set() {
        let active_series = [
            series::GROUPS_CREATED,
            series::GROUPS_ACTIVE,
            series::GAMES_STARTED,
            series::GAMES_ACTIVE,
            series::GAMES_COMPLETED,
            series::PLAYERS_CONNECTED,
            series::PLAYER_RECONNECTS,
            series::WAVES,
            series::WAVE_TIMEOUTS,
            series::ROUNDS,
            series::ROUND_DURATION,
            series::GAME_DURATION,
            series::ROUNDS_BOOMED,
            series::ROUNDS_FROZEN,
            series::BOOM_DETONATORS,
            series::WAVE_COMMITS,
            series::WAVE_FOLDS,
            series::SPELL_CASTS,
            series::WAVE_DURATION,
        ];
        for retired in RETIRED_V1_SERIES {
            assert!(
                !active_series.contains(retired),
                "{retired} is retired and must not be an active series"
            );
            assert!(
                definition(retired).is_none(),
                "{retired} is retired and must not be a definition id"
            );
        }
    }
}
