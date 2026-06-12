//! The seat-filler CLI (`boom2-ai-client` task 7.1) — **summon familiars**:
//! AI seats that join real rooms by invite code or matchmaking enqueue and
//! play with either brain. One process can operate several concurrent seats,
//! each with its own brain, settings, and connection — configure them in a
//! TOML file, or run a single seat from flags. Unnamed seats present with
//! familiar names ("Timid Toad (familiar)", the agent as "Homunculus
//! (familiar)") so players never mistake them for humans.
//!
//! ```toml
//! server = "ws://127.0.0.1:8080/ws"
//!
//! [[seats]]
//! name = "Sage Bramble"
//! entry = "enqueue"            # or an invite code like "BREW-7K3F"
//! brain = "bot"                # bot | agent
//! archetype = "political"
//! epsilon = 0.05
//! games = 2
//! emotes = [1, 2, 3]
//!
//! [[seats]]
//! name = "Madame Wick"
//! entry = "enqueue"
//! brain = "agent"
//! model = "claude-haiku-4-5"
//! persona = "a theatrical chandler who bets big on gut feeling"
//! difficulty = "standard"      # relaxed | standard | sharp
//! per_game_usd = 0.25
//! ```

use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;

use boiling_point_ai_client::agent::prompt::Difficulty;
use boiling_point_ai_client::agent::{AgentSettings, SpendCaps};
use boiling_point_ai_client::bot::Archetype;
use boiling_point_ai_client::filler::{
    FillerBrain, FillerSeatSettings, SeatExit, familiar_name, run_filler_process,
};
use boiling_point_ai_client::transport::EntryMode;
use boiling_point_protocol::GroupCode;

/// Summon familiars: AI seats that join real Boiling Point rooms and play.
#[derive(Parser)]
#[command(name = "familiar_summoning")]
struct Cli {
    /// Server WebSocket URL (e.g. ws://127.0.0.1:8080/ws).
    #[arg(long, default_value = "ws://127.0.0.1:8080/ws")]
    server: String,
    /// A multi-seat TOML config (overrides the single-seat flags below).
    #[arg(long)]
    config: Option<PathBuf>,
    /// Single seat: join this invite code (default: enqueue into matchmaking).
    #[arg(long)]
    join: Option<String>,
    /// Single seat: display name (default: the brain's familiar name,
    /// e.g. "Silver-tongued Raven (familiar)").
    #[arg(long)]
    name: Option<String>,
    /// Single seat: bot archetype (cautious|aggressive|political|random).
    #[arg(long, default_value = "political")]
    archetype: String,
    /// Single seat: blunder epsilon (0..=1).
    #[arg(long, default_value_t = 0.05)]
    epsilon: f64,
    /// Single seat: games to play before leaving.
    #[arg(long, default_value_t = 1)]
    games: u32,
    /// Single seat: run the Claude agent brain instead of a bot.
    #[arg(long, default_value_t = false)]
    agent: bool,
    /// Single seat: agent model id.
    #[arg(long)]
    model: Option<String>,
}

/// The TOML config shapes.
#[derive(Deserialize)]
struct FileConfig {
    server: Option<String>,
    seats: Vec<FileSeat>,
}

#[derive(Deserialize)]
struct FileSeat {
    /// Optional — defaults to the brain's familiar name.
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    entry: Option<String>,
    brain: String,
    #[serde(default)]
    archetype: Option<String>,
    #[serde(default)]
    epsilon: Option<f64>,
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    games: Option<u32>,
    #[serde(default)]
    emotes: Option<Vec<u16>>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    persona: Option<String>,
    #[serde(default)]
    difficulty: Option<String>,
    #[serde(default)]
    per_game_usd: Option<f64>,
    #[serde(default)]
    per_process_usd: Option<f64>,
}

fn entry_mode(raw: Option<&str>) -> EntryMode {
    match raw {
        None | Some("enqueue") => EntryMode::Enqueue,
        Some(code) => EntryMode::Join(GroupCode(code.to_string())),
    }
}

fn difficulty(raw: Option<&str>) -> Result<Difficulty, String> {
    match raw {
        None | Some("standard") => Ok(Difficulty::Standard),
        Some("relaxed") => Ok(Difficulty::Relaxed),
        Some("sharp") => Ok(Difficulty::Sharp),
        Some(other) => Err(format!("unknown difficulty '{other}'")),
    }
}

fn seat_from_file(seat: FileSeat) -> Result<FillerSeatSettings, String> {
    let brain = match seat.brain.as_str() {
        "bot" => {
            let archetype_name = seat.archetype.as_deref().unwrap_or("political");
            let archetype = Archetype::by_name(archetype_name)
                .ok_or_else(|| format!("unknown archetype '{archetype_name}'"))?;
            FillerBrain::Bot {
                archetype,
                epsilon: seat.epsilon.unwrap_or(0.0),
                seed: seat.seed.unwrap_or_else(rand::random),
            }
        }
        "agent" => {
            let mut settings = AgentSettings::default();
            if let Some(model) = seat.model {
                settings.model = model;
            }
            if let Some(persona) = seat.persona {
                settings.persona = persona;
            }
            settings.difficulty = difficulty(seat.difficulty.as_deref())?;
            let mut caps = SpendCaps::default();
            if let Some(per_game) = seat.per_game_usd {
                caps.per_game_usd = per_game;
            }
            if let Some(per_process) = seat.per_process_usd {
                caps.per_process_usd = per_process;
            }
            settings.spend = caps;
            FillerBrain::Agent(Box::new(settings))
        }
        other => return Err(format!("unknown brain '{other}'")),
    };
    Ok(FillerSeatSettings {
        display_name: seat.name.unwrap_or_else(|| familiar_name(&brain)),
        entry: entry_mode(seat.entry.as_deref()),
        brain,
        games: seat.games.unwrap_or(1),
        emote_palette: seat.emotes.unwrap_or_default(),
        reconnect_attempts: 3,
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    if let Err(e) = run(Cli::parse()).await {
        eprintln!("familiar_summoning: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let (server, seats) = match &cli.config {
        Some(path) => {
            let file: FileConfig = toml::from_str(&std::fs::read_to_string(path)?)?;
            let seats: Result<Vec<_>, _> = file.seats.into_iter().map(seat_from_file).collect();
            (file.server.unwrap_or(cli.server), seats?)
        }
        None => {
            let brain = if cli.agent {
                let mut settings = AgentSettings::default();
                if let Some(model) = cli.model.clone() {
                    settings.model = model;
                }
                FillerBrain::Agent(Box::new(settings))
            } else {
                let archetype = Archetype::by_name(&cli.archetype)
                    .ok_or_else(|| format!("unknown archetype '{}'", cli.archetype))?;
                FillerBrain::Bot {
                    archetype,
                    epsilon: cli.epsilon,
                    seed: rand::random(),
                }
            };
            let seat = FillerSeatSettings {
                display_name: cli.name.clone().unwrap_or_else(|| familiar_name(&brain)),
                entry: entry_mode(cli.join.as_deref()),
                brain,
                games: cli.games,
                ..FillerSeatSettings::default()
            };
            (cli.server.clone(), vec![seat])
        }
    };

    eprintln!("familiar_summoning: {} seat(s) → {server}", seats.len());
    let reports = run_filler_process(&server, seats, None).await;
    let mut failures = 0;
    for report in &reports {
        let games = report.games.len();
        let fallbacks: u32 = report.games.iter().map(|g| g.fallbacks()).sum();
        let decisions: u32 = report.games.iter().map(|g| g.decisions).sum();
        match &report.exit {
            SeatExit::Completed => eprintln!(
                "seat '{}': {games} game(s), {decisions} decisions, {fallbacks} fallbacks — done",
                report.display_name
            ),
            SeatExit::ConnectionLost(reason) => {
                failures += 1;
                eprintln!(
                    "seat '{}': exited after {games} game(s): {reason}",
                    report.display_name
                );
            }
            SeatExit::SecretLeak(detail) => {
                failures += 1;
                eprintln!(
                    "seat '{}': SECRET BOUNDARY BREACH (server bug): {detail}",
                    report.display_name
                );
            }
        }
    }
    if failures > 0 {
        return Err(format!("{failures} seat(s) did not complete cleanly").into());
    }
    Ok(())
}
