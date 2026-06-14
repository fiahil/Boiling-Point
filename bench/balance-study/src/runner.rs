//! The study runner (task 3.2): one call from a [`StudyConfig`] to a
//! [`StudyReport`]. It drives the harness's seeded batch runner — it does not
//! reimplement it — then layers provenance and the shared §IV fold on top.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use boiling_point_ai_client::ClientError;
use boiling_point_ai_client::harness::{Report, RunOptions, Thresholds, run_sample};
use boiling_point_server::config::{ContentConfig, DEFAULT_CONTENT_TOML};

use crate::config::StudyConfig;
use crate::report::StudyReport;

/// Run one study to a versioned report. Reads the study's content config (the
/// knob values; embedded default when none), drives the harness over the study's
/// seeded matrix, then assembles provenance + the shared §IV metrics.
pub async fn run_study(
    config: &StudyConfig,
    options: RunOptions,
) -> Result<StudyReport, ClientError> {
    let config_toml = match &config.content_config {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| ClientError::Config(format!("content config {}: {e}", path.display())))?,
        None => DEFAULT_CONTENT_TOML.to_string(),
    };
    let content =
        ContentConfig::from_toml(&config_toml).map_err(|e| ClientError::Config(e.to_string()))?;

    let run = run_sample(&content, &config.sample, options).await?;
    let harness = Report::build(&run, &config_toml, Thresholds::default());

    Ok(StudyReport::build(
        config,
        &run,
        harness,
        engine_commit(),
        generated_unix(),
    ))
}

/// The engine commit the study ran against, for provenance. Tries
/// `git rev-parse --short HEAD`, falls back to `$BENCH_ENGINE_COMMIT`, then
/// `unknown` — provenance only, never part of the reproducibility comparison.
fn engine_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("BENCH_ENGINE_COMMIT").ok())
        .unwrap_or_else(|| "unknown".into())
}

/// Wall-clock now in Unix seconds (display only).
fn generated_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
