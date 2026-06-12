//! Harness validation (tasks 2.2 + 6.5): reproducibility under a root seed,
//! divergence across seeds, in-process ↔ WebSocket transport parity, and the
//! unattended 1000-game batch — the Principle IV reinstatement proofs.

use boiling_point_ai_client::harness::{
    OutcomeSummary, Report, RunOptions, SampleRun, SampleSpec, Thresholds, TransportKind,
    run_sample,
};
use boiling_point_server::config::{ContentConfig, DEFAULT_CONTENT_TOML};

fn config() -> ContentConfig {
    ContentConfig::default_content()
}

/// Flatten a run into its per-game transport-comparable outcomes.
fn outcomes(run: &SampleRun) -> Vec<OutcomeSummary> {
    run.cells
        .iter()
        .flat_map(|c| c.games.iter().map(|g| g.outcome.clone()))
        .collect()
}

async fn run_baseline(seed: u64, games: u64, transport: TransportKind) -> SampleRun {
    run_sample(
        &config(),
        &SampleSpec::baseline(seed, games),
        RunOptions {
            transport,
            allow_agents: false,
        },
    )
    .await
    .expect("batch completes")
}

/// Same root seed ⇒ byte-identical reports (game outcomes, stats, and all).
#[tokio::test]
async fn same_seed_reproduces_the_whole_batch() {
    let first = run_baseline(2026, 25, TransportKind::InProcess).await;
    let again = run_baseline(2026, 25, TransportKind::InProcess).await;
    assert_eq!(outcomes(&first), outcomes(&again));
    let report_a = Report::build(&first, DEFAULT_CONTENT_TOML, Thresholds::default());
    let report_b = Report::build(&again, DEFAULT_CONTENT_TOML, Thresholds::default());
    assert_eq!(report_a.to_json(), report_b.to_json());
    assert!(
        report_a.reproducible,
        "all-bot in-process runs are reproducible"
    );
}

/// Different root seeds ⇒ different game outcomes (overwhelmingly).
#[tokio::test]
async fn different_seeds_diverge() {
    let a = run_baseline(1, 25, TransportKind::InProcess).await;
    let b = run_baseline(2, 25, TransportKind::InProcess).await;
    assert_ne!(outcomes(&a), outcomes(&b));
}

/// The same seeded scenario over the in-process seam and the real WebSocket
/// wire produces identical outcomes (task 2.2: transport parity is structural
/// — same codec bytes, same engine seeds, same deterministic brains).
#[tokio::test]
async fn transport_parity_in_process_vs_websocket() {
    let in_process = run_baseline(777, 4, TransportKind::InProcess).await;
    let websocket = run_baseline(777, 4, TransportKind::WebSocket).await;
    assert_eq!(
        outcomes(&in_process),
        outcomes(&websocket),
        "the same seeded scenario must end identically on both transports"
    );
    // The real wire still voids the reproducibility *claim* in reports (it is
    // the validation transport, not the balance-numbers transport).
    let report = Report::build(&websocket, DEFAULT_CONTENT_TOML, Thresholds::default());
    assert!(!report.reproducible);
}

/// A 1000-game bot-brain batch completes unattended in a single invocation
/// and emits its report (the Principle IV at-scale requirement). Every seat
/// answered every frame from the enumerated set: zero fallbacks, zero errors.
#[tokio::test]
async fn thousand_game_batch_runs_unattended() {
    let run = run_baseline(424242, 1000, TransportKind::InProcess).await;
    let total: usize = run.cells.iter().map(|c| c.games.len()).sum();
    assert_eq!(total, 1000);
    for cell in &run.cells {
        for game in &cell.games {
            for seat in &game.seats {
                assert_eq!(
                    seat.fallbacks, 0,
                    "a deterministic bot must never miss its budget"
                );
            }
        }
    }
    let report = Report::build(&run, DEFAULT_CONTENT_TOML, Thresholds::default());
    let stats = &report.cells[0].stats;
    assert_eq!(stats.games, 1000);
    assert!(stats.rounds >= 5000, "five rounds per game");
    assert!(stats.spell_casts_per_round > 0.0, "spells are exercised");
    assert!(
        !report.to_markdown().is_empty() && !report.to_json().is_empty(),
        "both report artifacts render"
    );
}

/// Archetype divergence at batch level (task 4.4's aggregate half): across a
/// seeded batch the postures produce measurably different aggregate stats.
#[tokio::test]
async fn archetypes_diverge_on_aggregate_stats() {
    let run = run_baseline(99, 200, TransportKind::InProcess).await;
    let report = Report::build(&run, DEFAULT_CONTENT_TOML, Thresholds::default());
    let by_label = &report.cells[0].stats.by_label;
    let cautious = &by_label["cautious"];
    let aggressive = &by_label["aggressive"];
    // The aggressive posture detonates more; the cautious posture folds to
    // safety more. Equality across 200 games would mean the archetypes are
    // not expressing their postures at all.
    assert!(
        aggressive.detonations > cautious.detonations,
        "aggressive ({}) should out-detonate cautious ({})",
        aggressive.detonations,
        cautious.detonations
    );
    assert!(
        cautious.folded_safe > aggressive.folded_safe,
        "cautious ({}) should fold to safety more than aggressive ({})",
        cautious.folded_safe,
        aggressive.folded_safe
    );
}

/// Agent seats in a batch are rejected without the explicit opt-in flag —
/// the no-accidental-spend gate (task 6.4).
#[tokio::test]
async fn agent_seats_require_the_explicit_flag() {
    let spec = SampleSpec::from_toml(
        r#"
        root_seed = 5
        [[cells]]
        name = "agent"
        games = 1
        seats = [
            { brain = "agent" },
            { brain = "bot", archetype = "cautious" },
            { brain = "bot", archetype = "political" },
            { brain = "bot", archetype = "random" },
        ]
        "#,
    )
    .expect("parses");
    let err = run_sample(&config(), &spec, RunOptions::default())
        .await
        .expect_err("must be rejected");
    assert!(err.to_string().contains("agent"), "{err}");
}
