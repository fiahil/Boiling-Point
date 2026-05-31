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

mod clipboard;
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
use crossterm::event::{Event, EventStream};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use std::io::Stdout;

/// Frame/tick cadence (~30 fps): drives the countdown, depile, and toasts.
const TICK_MS: u32 = 33;
/// Spacing between scripted messages in replay/mock modes.
const FEED_MS: u64 = 700;

/// Where the client gets its server messages from.
enum Mode {
    /// Live WebSocket to a server at this URL.
    Connect(String),
    /// Replay a recorded JSON-lines message stream.
    Replay(PathBuf),
    /// Drive an in-process scripted demo game (no server, no network).
    Mock,
}

/// Parsed command-line options.
struct Options {
    mode: Mode,
    record: Option<PathBuf>,
    name: Option<String>,
}

impl Options {
    fn from_args() -> Options {
        let mut mode = Mode::Mock;
        let mut record = None;
        let mut name = None;
        let mut args = std::env::args().skip(1);
        while let Some(a) = args.next() {
            match a.as_str() {
                "--connect" => {
                    if let Some(u) = args.next() {
                        mode = Mode::Connect(u);
                    }
                }
                "--replay" => {
                    if let Some(p) = args.next() {
                        mode = Mode::Replay(p.into());
                    }
                }
                "--mock" => mode = Mode::Mock,
                "--record" => {
                    if let Some(p) = args.next() {
                        record = Some(p.into());
                    }
                }
                "--name" => name = args.next(),
                _ => {}
            }
        }
        Options { mode, record, name }
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
            let (rx, tx) = net::connect(&url).await?;
            (rx, Some(tx), true)
        }
        Mode::Replay(path) => (spawn_feeder(replay::load(&path)?), None, false),
        Mode::Mock => (spawn_feeder(fixtures::demo_game()), None, false),
    };

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
        }
    }
    Ok(())
}
