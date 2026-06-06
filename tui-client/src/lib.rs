//! Boiling Point terminal client.
//!
//! An untrusted ratatui renderer over the wire [`boiling_point_protocol`]: it
//! renders server state and turns key presses into client intents, owning no
//! game logic and holding no secrets (Constitution I). [`App`] and its pure
//! reducers are the testable core; [`run`] wires them to a terminal and either a
//! live server, a recorded replay, or an in-process mock.

pub mod app;
pub mod fixtures;
pub mod replay;

mod net;
mod palette;
mod term;
mod ui;
mod view;

pub use app::App;

use std::error::Error;
use std::fs::File;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use boiling_point_protocol::{ClientMessage, ServerMessage};
use clap::Parser;
use crossterm::event::{Event, EventStream};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

use std::io::Stdout;

/// Frame/tick cadence (~30 fps): drives the countdown, depile, and toasts.
const TICK_MS: u32 = 33;
/// How often to send a keep-alive Heartbeat. The server drops a connection that
/// is silent past its idle timeout (90 s by default), and a player who has
/// passed/locked out — or is just watching a long wave — sends nothing
/// otherwise, so a periodic heartbeat keeps the seat alive. Well under the
/// server window to tolerate jitter.
const HEARTBEAT_MS: u64 = 20_000;
/// Spacing between scripted messages in replay/mock modes.
const FEED_MS: u64 = 700;
/// Server WebSocket URL used when no transport flag is given. Connecting to a
/// live server is the default; `--mock` selects the offline demo instead.
const DEFAULT_SERVER_URL: &str = "ws://127.0.0.1:8080/ws";

/// Where the client gets its server messages from.
enum Mode {
    /// Live WebSocket to a server at this URL.
    Connect(String),
    /// Replay a recorded JSON-lines message stream.
    Replay(PathBuf),
    /// Drive an in-process scripted demo game (no server, no network).
    Mock,
}

/// Command-line options for the terminal client.
#[derive(Debug, Parser)]
#[command(
    name = "boiling-point-tui",
    version,
    about = "Boiling Point terminal client — an untrusted renderer over the wire protocol."
)]
struct Cli {
    /// Connect to a live server at this WebSocket URL. Connecting is the default
    /// transport; this flag only overrides the URL (default ws://127.0.0.1:8080/ws).
    #[arg(long, value_name = "URL", conflicts_with = "replay")]
    connect: Option<String>,
    /// Replay a recorded JSON-lines server-message stream instead of connecting.
    #[arg(long, value_name = "PATH")]
    replay: Option<PathBuf>,
    /// Drive an in-process scripted demo game with no server or network, instead
    /// of connecting to the live server.
    #[arg(long, conflicts_with_all = ["connect", "replay"])]
    mock: bool,
    /// Record incoming server messages to this file (JSON lines).
    #[arg(long, value_name = "PATH")]
    record: Option<PathBuf>,
    /// Display name to use at the table.
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
    /// On connect, immediately enter the matchmaking queue, skipping the entry
    /// menu. Intended for scripted launches (see scripts/playtest.sh).
    #[arg(long, requires = "connect")]
    enqueue: bool,
}

/// Parsed command-line options, resolved into a single transport [`Mode`].
struct Options {
    mode: Mode,
    record: Option<PathBuf>,
    name: Option<String>,
    /// Auto-enter the matchmaking queue on connect (scripted launch).
    enqueue: bool,
}

impl Options {
    fn from_args() -> Options {
        let cli = Cli::parse();
        // Connecting to the live server is the default. `--mock` selects the
        // offline demo, `--replay` plays back a recorded stream, and `--connect`
        // overrides the server URL.
        let mode = if cli.mock {
            Mode::Mock
        } else if let Some(path) = cli.replay {
            Mode::Replay(path)
        } else {
            Mode::Connect(
                cli.connect
                    .unwrap_or_else(|| DEFAULT_SERVER_URL.to_string()),
            )
        };
        Options {
            mode,
            record: cli.record,
            name: cli.name,
            enqueue: cli.enqueue,
        }
    }
}

/// Run the terminal client. Parses arguments, sets up the terminal, and drives
/// the render/input/message loop until the user quits. Returns a process exit
/// code.
pub fn run() -> ExitCode {
    let opts = Options::from_args();
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to start runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match rt.block_on(async_main(opts)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn async_main(opts: Options) -> Result<(), Box<dyn Error>> {
    let mut app = App::new();
    if let Some(n) = opts.name {
        app.set_name(n);
    }

    let (server_rx, intent_tx, connect_mode) = match opts.mode {
        Mode::Connect(url) => {
            let (rx, tx) = net::connect(&url).await.map_err(|e| {
                format!(
                    "could not connect to {url}: {e}\n\
                     Is the server running? Start it with `cargo run -p boiling-point-server`, \
                     or pass --mock for the offline demo."
                )
            })?;
            (rx, Some(tx), true)
        }
        Mode::Replay(path) => (spawn_feeder(replay::load(&path)?), None, false),
        Mode::Mock => (spawn_feeder(fixtures::demo_game()), None, false),
    };

    // Scripted launch: enter the matchmaking queue immediately on connect, so a
    // launcher can table the player with bots without any menu interaction.
    if opts.enqueue
        && let Some(tx) = intent_tx.as_ref()
    {
        for intent in app.auto_enqueue() {
            app.log_outgoing(&intent);
            let _ = tx.send(intent).await;
        }
    }

    let record_file = match opts.record {
        Some(p) => Some(File::create(p)?),
        None => None,
    };

    let mut terminal = term::init()?;
    let result = event_loop(
        &mut terminal,
        &mut app,
        server_rx,
        intent_tx,
        connect_mode,
        record_file,
    )
    .await;
    let _ = term::restore();
    result
}

/// Spawn a task that feeds a fixed message list into a channel on a timer
/// (replay/mock). The channel closes when the list is exhausted.
fn spawn_feeder(msgs: Vec<ServerMessage>) -> mpsc::Receiver<ServerMessage> {
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(async move {
        for m in msgs {
            tokio::time::sleep(Duration::from_millis(FEED_MS)).await;
            if tx.send(m).await.is_err() {
                break;
            }
        }
    });
    rx
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    mut server_rx: mpsc::Receiver<ServerMessage>,
    intent_tx: Option<mpsc::Sender<ClientMessage>>,
    connect_mode: bool,
    mut record: Option<File>,
) -> Result<(), Box<dyn Error>> {
    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(TICK_MS as u64));
    let mut heartbeat = tokio::time::interval(Duration::from_millis(HEARTBEAT_MS));
    let mut server_done = false;

    loop {
        terminal.draw(|f| app.render(f))?;
        if app.should_quit() {
            break;
        }

        tokio::select! {
            maybe_ev = events.next() => match maybe_ev {
                Some(Ok(Event::Key(key))) => {
                    for intent in app.on_key(key) {
                        app.log_outgoing(&intent);
                        if let Some(tx) = &intent_tx {
                            let _ = tx.send(intent).await;
                        }
                    }
                }
                Some(Ok(_)) => {}              // resize/mouse/focus: redraw handles it
                Some(Err(_)) | None => app.request_quit(),
            },
            maybe_msg = server_rx.recv(), if !server_done => match maybe_msg {
                Some(msg) => {
                    if let Some(f) = record.as_mut() {
                        let _ = replay::append(f, &msg);
                    }
                    app.on_server(&msg);
                }
                None => {
                    server_done = true;
                    if connect_mode {
                        app.on_disconnect();
                    }
                }
            },
            _ = tick.tick() => app.on_tick(TICK_MS),
            _ = heartbeat.tick(), if !server_done => {
                // Keep-alive so the server's idle timeout never drops a quiet seat
                // (passed/locked-out or just watching). Sent straight to the socket,
                // not through on_key, so it stays out of the debug message log.
                //
                // Only once we've entered: the server's first frame MUST be an entry
                // message, and `interval`'s first tick fires immediately, so an
                // un-gated heartbeat would beat the user's menu choice onto the wire
                // and the server would reject it and drop the socket.
                if app.has_entered()
                    && let Some(tx) = &intent_tx
                {
                    let _ = tx.send(ClientMessage::Heartbeat).await;
                }
            }
        }
    }
    Ok(())
}
