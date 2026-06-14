//! The `bench_dashboard` binary (tasks 4.1/4.3): two subcommands.
//!
//! ```text
//! # CI, per merge: criterion output -> one history record (append to bench-data)
//! bench_dashboard collect --criterion-dir target/criterion --commit "$SHA" --out record.json
//!
//! # CI + local: full history + study reports -> one self-contained page
//! bench_dashboard render --history bench-data/criterion --studies bench-data/studies --out benches.html
//! ```

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use boiling_point_bench_dashboard::history::{HistoryRecord, load_dir};
use boiling_point_bench_dashboard::render::{load_studies_dir, render};

/// Collect criterion estimates into the bench history and render the dashboard.
#[derive(Parser)]
#[command(name = "bench_dashboard")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Extract one history record from a criterion run.
    Collect {
        /// The criterion output directory (default `target/criterion`).
        #[arg(long, default_value = "target/criterion")]
        criterion_dir: PathBuf,
        /// The commit these benches ran against (short SHA, or `unknown`).
        #[arg(long, default_value = "unknown")]
        commit: String,
        /// Where to write the record JSON (default stdout).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Render the full history + study reports to one self-contained HTML page.
    Render {
        /// Directory of history records (the `bench-data` criterion records).
        #[arg(long)]
        history: Option<PathBuf>,
        /// Directory of balance-study report JSONs.
        #[arg(long)]
        studies: Option<PathBuf>,
        /// Output HTML path (default `benches.html`).
        #[arg(long, default_value = "benches.html")]
        out: PathBuf,
    },
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("bench_dashboard: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Collect {
            criterion_dir,
            commit,
            out,
        } => {
            let record = HistoryRecord::build(&criterion_dir, commit)?;
            let json = record.to_json();
            match out {
                Some(path) => {
                    if let Some(parent) = path.parent()
                        && !parent.as_os_str().is_empty()
                    {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    std::fs::write(&path, json).map_err(|e| e.to_string())?;
                    eprintln!(
                        "bench_dashboard: wrote {} ({} bench(es))",
                        path.display(),
                        record.benches.len()
                    );
                }
                None => println!("{json}"),
            }
        }
        Command::Render {
            history,
            studies,
            out,
        } => {
            let records = match &history {
                Some(dir) => load_dir(dir)?,
                None => Vec::new(),
            };
            let reports = match &studies {
                Some(dir) => load_studies_dir(dir)?,
                None => Vec::new(),
            };
            let html = render(&records, &reports);
            std::fs::write(&out, html).map_err(|e| e.to_string())?;
            eprintln!(
                "bench_dashboard: wrote {} ({} record(s), {} study report(s))",
                out.display(),
                records.len(),
                reports.len()
            );
        }
    }
    Ok(())
}
