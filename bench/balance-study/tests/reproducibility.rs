//! Reproducibility (task 3.4): same provenance ⇒ identical metrics.
//!
//! A transport/integration test — it boots complete in-process games through the
//! harness (no mocking, no DB), exactly as the constitution's e2e layer requires.
//! It proves the spec scenario: "a rerun with the same seeds, config hash, and
//! engine commit emits identical metrics."

use boiling_point_ai_client::harness::{RunOptions, TransportKind};
use boiling_point_balance_study::config::StudyConfig;
use boiling_point_balance_study::runner::run_study;

#[tokio::test]
async fn same_provenance_reproduces_the_metrics() {
    // A small all-bot study (fast, zero Claude calls) run twice, identically.
    let config = StudyConfig::baseline(123_456, 16);
    let options = RunOptions {
        transport: TransportKind::InProcess,
        allow_agents: false,
    };
    let a = run_study(&config, options).await.expect("study a runs");
    let b = run_study(&config, options).await.expect("study b runs");

    assert!(
        a.provenance.reproducible,
        "an all-bot in-process study is reproducible"
    );

    // Same seeds + config + engine ⇒ byte-identical §IV metrics and per-cell
    // statistics. Provenance's commit/timestamp are deliberately NOT compared —
    // they are attribution, not metrics.
    let a_metrics = serde_json::to_value(&a.metrics).unwrap();
    let b_metrics = serde_json::to_value(&b.metrics).unwrap();
    assert_eq!(a_metrics, b_metrics, "the §IV metrics must reproduce");

    let a_harness = serde_json::to_value(&a.harness).unwrap();
    let b_harness = serde_json::to_value(&b.harness).unwrap();
    assert_eq!(
        a_harness, b_harness,
        "the per-cell statistics must reproduce"
    );

    // And the shared fold actually produced data over real games.
    let boom = a
        .metrics
        .iter()
        .find(|m| m.id == "boom_rate")
        .expect("boom_rate is a shared definition");
    assert!(
        boom.value.is_some(),
        "boom_rate is populated from the harness games"
    );
}
