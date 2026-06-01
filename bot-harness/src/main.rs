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

/// The default content config, embedded so the harness runs with no arguments.
const DEFAULT_CONFIG: &str = include_str!("../../server/content.toml");

/// Parsed command-line arguments.
struct Args {
    games: u64,
    seed: u64,
    transport: TransportKind,
    strategies: Vec<String>,
    config_path: Option<String>,
    report_path: Option<String>,
}

impl Args {
    /// Parse argv, applying defaults. Returns an error message on bad input.
    fn parse() -> Result<Args, String> {
        let mut args = Args {
            games: 1000,
            seed: 0,
            transport: TransportKind::InProcess,
            strategies: BASELINE_NAMES.iter().map(|s| s.to_string()).collect(),
            config_path: None,
            report_path: None,
        };
        let mut it = std::env::args().skip(1);
        while let Some(flag) = it.next() {
            let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
            match flag.as_str() {
                "--games" => {
                    args.games = value()?
                        .parse()
                        .map_err(|_| "invalid --games".to_string())?
                }
                "--seed" => {
                    args.seed = value()?.parse().map_err(|_| "invalid --seed".to_string())?
                }
                "--transport" => {
                    args.transport = match value()?.as_str() {
                        "in-process" => TransportKind::InProcess,
                        "websocket" => TransportKind::WebSocket,
                        other => return Err(format!("unknown transport: {other}")),
                    }
                }
                "--strategies" => {
                    args.strategies = value()?.split(',').map(|s| s.trim().to_string()).collect()
                }
                "--config" => args.config_path = Some(value()?),
                "--report" => args.report_path = Some(value()?),
                "-h" | "--help" => return Err(HELP.to_string()),
                other => return Err(format!("unknown flag: {other}\n\n{HELP}")),
            }
        }
        if args.strategies.len() != 4 {
            return Err(format!(
                "--strategies needs exactly 4 comma-separated names (one per seat), got {}",
                args.strategies.len()
            ));
        }
        Ok(args)
    }
}

const HELP: &str = "\
bot-harness — Boiling Point balance batch runner

Usage:
  bot-harness [--games N] [--seed N] [--transport in-process|websocket]
              [--strategies a,b,c,d] [--config PATH] [--report PATH]

Options:
  --games N         Number of complete games to play (default 1000)
  --seed N          Root seed for the deterministic RNG tree (default 0)
  --transport T     in-process (default, reproducible) or websocket (smaller, real wire)
  --strategies LIST 4 comma-separated seat strategies (default: cautious,aggressor,diplomat,random)
                    Available: cautious, aggressor, diplomat, random
  --config PATH     Content config TOML (default: the embedded server config)
  --report PATH     Write the machine-readable JSON report to PATH
";

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
    let args = Args::parse()?;

    // Load the config text (for both parsing and fingerprinting).
    let config_text = match &args.config_path {
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
        games: args.games,
        seed: args.seed,
        transport: args.transport,
        strategy_names: args.strategies.clone(),
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

    if let Some(path) = &args.report_path {
        std::fs::write(path, report.to_json())
            .map_err(|e| format!("could not write report {path}: {e}"))?;
        eprintln!("Wrote JSON report to {path}");
    }

    Ok(())
}
