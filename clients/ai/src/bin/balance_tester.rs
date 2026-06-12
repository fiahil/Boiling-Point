//! The balance-harness batch runner (`boom2-ai-client` tasks 6.x): seeded
//! batches of complete v2 games over the in-process seam, emitting the
//! markdown + JSON balance reports (Principle IV).
//!
//! Defaults to the all-bot baseline cell (zero Claude calls). A matrix sample
//! spec (TOML) selects persona cells and game counts; agent seats additionally
//! require `--allow-agents` (real spend, voids reproducibility).

use std::path::PathBuf;

use clap::Parser;

use boiling_point_ai_client::harness::{
    Report, RunOptions, SampleSpec, Thresholds, TransportKind, run_sample,
};
use boiling_point_server::config::{ContentConfig, DEFAULT_CONTENT_TOML};

/// Seeded balance batches over the headless server (Principle IV).
#[derive(Parser)]
#[command(name = "balance_tester")]
struct Cli {
    /// Games for the default baseline cell (ignored when --spec is given).
    #[arg(long, default_value_t = 1000)]
    games: u64,
    /// Root seed for the run's RNG tree.
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// A TOML matrix sample spec (cells × games × seat assignments).
    #[arg(long)]
    spec: Option<PathBuf>,
    /// Content config TOML (defaults to the server's embedded config).
    #[arg(long)]
    config: Option<PathBuf>,
    /// Transport: in-process (default, reproducible) or websocket (parity).
    #[arg(long, default_value = "in-process")]
    transport: String,
    /// Permit agent (Claude) seats in the spec — spends real money.
    #[arg(long, default_value_t = false)]
    allow_agents: bool,
    /// Report basename: writes `<report>.md` and `<report>.json`.
    #[arg(long)]
    report: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = run(Cli::parse()).await {
        eprintln!("balance_tester: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config_toml = match &cli.config {
        Some(path) => std::fs::read_to_string(path)?,
        None => DEFAULT_CONTENT_TOML.to_string(),
    };
    let config = ContentConfig::from_toml(&config_toml)?;

    let spec = match &cli.spec {
        Some(path) => SampleSpec::from_toml(&std::fs::read_to_string(path)?)?,
        None => SampleSpec::baseline(cli.seed, cli.games),
    };
    let transport = match cli.transport.as_str() {
        "in-process" => TransportKind::InProcess,
        "websocket" => TransportKind::WebSocket,
        other => return Err(format!("unknown transport '{other}'").into()),
    };

    let total_games: u64 = spec.cells.iter().map(|c| c.games).sum();
    eprintln!(
        "balance_tester: {} cell(s), {} games, seed {}, {} transport",
        spec.cells.len(),
        total_games,
        spec.root_seed,
        transport.label(),
    );

    let run = run_sample(
        &config,
        &spec,
        RunOptions {
            transport,
            allow_agents: cli.allow_agents,
        },
    )
    .await?;
    let report = Report::build(&run, &config_toml, Thresholds::default());

    match &cli.report {
        Some(base) => {
            let md = base.with_extension("md");
            let json = base.with_extension("json");
            if let Some(parent) = base.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&md, report.to_markdown())?;
            std::fs::write(&json, report.to_json())?;
            eprintln!(
                "balance_tester: wrote {} and {}",
                md.display(),
                json.display()
            );
        }
        None => println!("{}", report.to_markdown()),
    }
    if !report.smells.is_empty() {
        eprintln!(
            "balance_tester: {} balance smell(s) flagged",
            report.smells.len()
        );
    }
    Ok(())
}
