//! The `balance_study` binary (task 3.2): one command from a study config to a
//! versioned report. On demand, at scale, purely observational — it never gates
//! and (with bot seats) makes zero Claude calls.
//!
//! ```text
//! balance_study --study studies/explosion-band.toml --out target/bench/studies/explosion-band
//! balance_study --games 2000 --seed 42 --out target/bench/studies/baseline   # default baseline
//! ```

use std::path::PathBuf;

use clap::Parser;

use boiling_point_ai_client::harness::{RunOptions, TransportKind};
use boiling_point_balance_study::config::StudyConfig;
use boiling_point_balance_study::runner::run_study;

/// On-demand seeded balance studies over the AI-client harness (Principle IV).
#[derive(Parser)]
#[command(name = "balance_study")]
struct Cli {
    /// A TOML study config (name, question, content config, matrix sample).
    #[arg(long)]
    study: Option<PathBuf>,
    /// Games for the default baseline study (ignored when --study is given).
    #[arg(long, default_value_t = 1000)]
    games: u64,
    /// Root seed for the default baseline study (ignored when --study is given).
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// Transport: in-process (default, reproducible) or websocket (parity).
    #[arg(long, default_value = "in-process")]
    transport: String,
    /// Permit agent (Claude) seats in the sample — spends real money, voids
    /// reproducibility.
    #[arg(long, default_value_t = false)]
    allow_agents: bool,
    /// Report basename: writes `<out>.json` (the dashboard artifact) and
    /// `<out>.md` (the human summary). Absent ⇒ markdown to stdout.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = run(Cli::parse()).await {
        eprintln!("balance_study: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config = match &cli.study {
        Some(path) => StudyConfig::from_toml(&std::fs::read_to_string(path)?)?,
        None => StudyConfig::baseline(cli.seed, cli.games),
    };
    let transport = match cli.transport.as_str() {
        "in-process" => TransportKind::InProcess,
        "websocket" => TransportKind::WebSocket,
        other => return Err(format!("unknown transport '{other}'").into()),
    };

    eprintln!(
        "balance_study: '{}' — {} cell(s), {} games, seed {}, {} transport",
        config.name,
        config.sample.cells.len(),
        config.total_games(),
        config.sample.root_seed,
        transport.label(),
    );

    let report = run_study(
        &config,
        RunOptions {
            transport,
            allow_agents: cli.allow_agents,
        },
    )
    .await?;

    match &cli.out {
        Some(base) => {
            let json = base.with_extension("json");
            let md = base.with_extension("md");
            if let Some(parent) = base.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&json, report.to_json())?;
            std::fs::write(&md, report.to_markdown())?;
            eprintln!(
                "balance_study: wrote {} and {}",
                json.display(),
                md.display()
            );
        }
        None => println!("{}", report.to_markdown()),
    }
    if !report.flags.is_empty() {
        eprintln!("balance_study: {} flag(s) raised", report.flags.len());
    }
    Ok(())
}
