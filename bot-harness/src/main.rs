//! `bot-harness` — the balance batch runner CLI.
//!
//! Plays N complete games with a chosen strategy assignment against a content
//! config, aggregates the balance statistics, prints a human-readable summary, and
//! optionally writes the machine-readable JSON report for diffing across config
//! versions.
//!
//! ```text
//! bot-harness [--games N] [--seed N] [--transport in-process|websocket]
//!             [--strategies a,b,c,d] [--config PATH] [--report PATH]
//! ```

use std::process::ExitCode;

use boiling_point_bot_harness::report::{Report, Thresholds, fingerprint};
use boiling_point_bot_harness::runner::{BatchParams, TransportKind, run_batch};
use boiling_point_bot_harness::stats::BatchStats;
use boiling_point_bot_harness::strategy::BASELINE_NAMES;
use boiling_point_server::config::ContentConfig;
use clap::Parser;

/// The default content config, embedded so the harness runs with no arguments.
const DEFAULT_CONFIG: &str = include_str!("../../server/content.toml");

/// Command-line arguments for the balance batch runner.
#[derive(Debug, Parser)]
#[command(
    name = "bot-harness",
    version,
    about = "Boiling Point Layer-1 balance batch runner — plays N seeded games and reports balance statistics."
)]
struct Cli {
    /// Number of complete games to play.
    #[arg(long, default_value_t = 1000)]
    games: u64,
    /// Root seed for the deterministic RNG tree.
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// Transport: `in-process` (reproducible) or `websocket` (smaller, real wire).
    #[arg(long, default_value = "in-process")]
    transport: String,
    /// Four comma-separated seat strategies, one per seat (default: the four
    /// baselines). Available: cautious, aggressor, diplomat, random.
    #[arg(long, value_name = "A,B,C,D")]
    strategies: Option<String>,
    /// Content config TOML to load (default: the embedded server config).
    #[arg(long, value_name = "PATH")]
    config: Option<String>,
    /// Write the machine-readable JSON report to this path.
    #[arg(long, value_name = "PATH")]
    report: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let cli = Cli::parse();

    let transport = match cli.transport.as_str() {
        "in-process" => TransportKind::InProcess,
        "websocket" => TransportKind::WebSocket,
        other => return Err(format!("unknown transport: {other}")),
    };
    let strategies: Vec<String> = match &cli.strategies {
        Some(s) => s.split(',').map(|s| s.trim().to_string()).collect(),
        None => BASELINE_NAMES.iter().map(|s| s.to_string()).collect(),
    };
    if strategies.len() != 4 {
        return Err(format!(
            "--strategies needs exactly 4 comma-separated names (one per seat), got {}",
            strategies.len()
        ));
    }

    // Load the config text (for both parsing and fingerprinting).
    let config_text = match &cli.config {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("could not read config {path}: {e}"))?,
        None => DEFAULT_CONFIG.to_string(),
    };
    let config =
        ContentConfig::from_toml(&config_text).map_err(|e| format!("invalid config: {e}"))?;
    config
        .validate()
        .map_err(|e| format!("config failed validation: {e}"))?;

    let params = BatchParams {
        games: cli.games,
        seed: cli.seed,
        transport,
        strategy_names: strategies,
    };

    eprintln!(
        "Running {} games ({}), seed {}, seats [{}] …",
        params.games,
        params.transport.label(),
        params.seed,
        params.strategy_names.join(", "),
    );

    let records = run_batch(&config, &params)
        .await
        .map_err(|e| format!("batch failed: {e}"))?;

    let stats = BatchStats::aggregate(&records);
    let report = Report::build(
        fingerprint(&config_text),
        params.seed,
        params.transport.label(),
        params.strategy_names,
        stats,
        Thresholds::default(),
    );

    println!("{}", report.to_markdown());

    if let Some(path) = &cli.report {
        std::fs::write(path, report.to_json())
            .map_err(|e| format!("could not write report {path}: {e}"))?;
        eprintln!("Wrote JSON report to {path}");
    }

    Ok(())
}
