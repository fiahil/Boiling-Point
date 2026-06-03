//! End-to-end tests against the **real authoritative server**, started
//! in-process on an ephemeral port. These exercise the actual wire: real
//! MessagePack frames out of the server's game loop, decoded by the client and
//! folded into [`App`], then rendered. No mock, no fixtures.
//!
//! (The server is started exactly as its own `start_server` test helper does.)

use std::sync::Arc;
use std::time::Duration;

use boiling_point_protocol::{
    ClientMessage,
    client::PROTOCOL_VERSION,
    codec,
    ids::{CardId, RoomCode},
    server::ServerMessage,
};
use boiling_point_server::{
    config::ContentConfig,
    lobby::{MatchQueue, RoomRegistry, SessionStore},
    transport::{AppState, app},
};
use boiling_point_tui::App;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message as TMsg,
};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Start the real server on an ephemeral port; returns its `ws://…/ws` URL.
async fn start_server() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let toml = std::fs::read_to_string(format!("{manifest}/../server/content.toml"))
        .expect("read server content.toml");
    let mut config = ContentConfig::from_toml(&toml).expect("parse content config");
    config.timing.wave1_ms = 120; // snappy waves for the test
    config.timing.wave_ms = 120;
    let registry = Arc::new(config.build_registry().expect("build registry"));
    let config = Arc::new(config);
    let rooms = Arc::new(RoomRegistry::new(registry, config));
    let queue = Arc::new(MatchQueue::new(rooms.clone()));
    let state = AppState {
        sessions: Arc::new(SessionStore::new()),
        rooms,
        queue,
        conn_timeout: Duration::from_secs(60),
        pool: None,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app(state)).await.unwrap();
    });
    format!("ws://{addr}/ws")
}

async fn connect(url: &str) -> Ws {
    let (ws, _) = connect_async(url).await.expect("ws connect");
    ws
}

async fn send(ws: &mut Ws, msg: &ClientMessage) {
    ws.send(TMsg::Binary(codec::encode(msg).unwrap()))
        .await
        .unwrap();
}

async fn recv(ws: &mut Ws) -> Option<ServerMessage> {
    while let Some(frame) = ws.next().await {
        match frame {
            Ok(TMsg::Binary(b)) => return Some(codec::decode(b.as_ref()).unwrap()),
            Ok(TMsg::Close(_)) | Err(_) => return None,
            _ => continue,
        }
    }
    None
}

/// Pick the next card to commit from a locally tracked hand (or pass when empty).
fn next_play(hand: &mut Vec<CardId>) -> ClientMessage {
    match hand.pop() {
        Some(card) => ClientMessage::CommitCard { card },
        None => ClientMessage::CommitPass,
    }
}

/// A bot seat: plays one card per wave until the game ends. Seat 0 sends the
/// room's invite code back over `code_tx` and returns the final rendered screen
/// of an `App` fed the real message stream.
async fn play_seat(
    url: String,
    entry: ClientMessage,
    mut code_tx: Option<oneshot::Sender<String>>,
    capture: bool,
) -> Option<String> {
    let mut ws = connect(&url).await;
    send(&mut ws, &entry).await;
    let mut app = App::new();
    let mut hand: Vec<CardId> = Vec::new();
    while let Some(msg) = recv(&mut ws).await {
        if capture {
            app.on_server(&msg);
        }
        match &msg {
            ServerMessage::RoomJoined { room_code, .. } => {
                if let Some(tx) = code_tx.take() {
                    let _ = tx.send(room_code.0.clone());
                }
            }
            ServerMessage::YourHand { cards } => {
                hand = cards.iter().map(|c| c.id).collect();
            }
            ServerMessage::WaveOpened { .. } => {
                let play = next_play(&mut hand);
                send(&mut ws, &play).await;
            }
            ServerMessage::GameOver { .. } => break,
            _ => {}
        }
    }
    capture.then(|| render(&app))
}

fn render(app: &App) -> String {
    let buf = app.render_to_buffer(100, 34);
    let area = buf.area;
    let mut s = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    s
}

fn create(name: &str) -> ClientMessage {
    ClientMessage::CreateRoom {
        protocol_version: PROTOCOL_VERSION,
        display_name: name.into(),
        session_token: None,
    }
}

fn join(name: &str, code: &str) -> ClientMessage {
    ClientMessage::JoinRoom {
        protocol_version: PROTOCOL_VERSION,
        display_name: name.into(),
        session_token: None,
        room_code: RoomCode(code.into()),
    }
}

/// Real server, real wire: create a room and confirm the client decodes
/// `RoomJoined` and renders the lobby with the server-issued invite code.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_create_room_handshake() {
    let url = start_server().await;
    let mut ws = connect(&url).await;
    send(&mut ws, &create("quentin")).await;

    let mut app = App::new();
    let mut code = None;
    // Read until we see RoomJoined (or give up after a few frames).
    for _ in 0..8 {
        match recv(&mut ws).await {
            Some(msg) => {
                app.on_server(&msg);
                if let ServerMessage::RoomJoined { room_code, .. } = &msg {
                    code = Some(room_code.0.clone());
                    break;
                }
            }
            None => break,
        }
    }

    let code = code.expect("server issued a RoomJoined with an invite code");
    assert!(code.starts_with("BREW-"), "unexpected code: {code}");
    let s = render(&app);
    assert!(s.contains("Lobby"), "expected the lobby screen");
    assert!(s.contains(&code), "expected the real invite code on screen");
    assert!(!s.contains("boiling"), "no boiling-point leak in the lobby");
}

/// Real server, real wire: four clients fill a table, the server auto-starts,
/// and a full game plays out. The captured client (seat 0) — driven only by the
/// real message stream — renders through to the game-over standings.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn live_full_game_reaches_game_over() {
    let url = start_server().await;
    let (code_tx, code_rx) = oneshot::channel();

    // Seat 0 creates the room, reports the code, and captures the rendered game.
    let seat0 = tokio::spawn(play_seat(url.clone(), create("seat0"), Some(code_tx), true));

    let code = tokio::time::timeout(Duration::from_secs(5), code_rx)
        .await
        .expect("got an invite code in time")
        .expect("code channel");

    // Seats 1–3 join by code; the fourth join auto-starts the game.
    let joiners: Vec<_> = (1..4)
        .map(|i| {
            let (u, c) = (url.clone(), code.clone());
            tokio::spawn(play_seat(u, join(&format!("seat{i}"), &c), None, false))
        })
        .collect();

    let final_screen = tokio::time::timeout(Duration::from_secs(20), seat0)
        .await
        .expect("game finished within the timeout")
        .expect("seat0 task")
        .expect("seat0 captured a screen");

    for j in joiners {
        let _ = j.await;
    }

    assert!(
        final_screen.contains("FINAL STANDINGS")
            || final_screen.contains("GAME OVER")
            || final_screen.contains("game over"),
        "expected a game-over screen after a full real game; got:\n{final_screen}"
    );
}
