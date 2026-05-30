//! WebSocket transport: the Axum app, the per-connection read/write tasks, the
//! protocol-version handshake, and per-connection rate limiting.
//!
//! Each connection owns an outbound mpsc channel the room writes to; a writer
//! task serialises those messages to the socket as MessagePack. The first client
//! message must be an entry message (`CreateRoom`/`JoinRoom`) carrying a
//! compatible protocol version, after which the connection is bridged to its
//! room and subsequent messages are forwarded as actions.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use boiling_point_protocol::client::PROTOCOL_VERSION;
use boiling_point_protocol::server::ErrorCode;
use boiling_point_protocol::{codec, ClientMessage, PlayerId, ServerMessage};

use crate::lobby::room::RoomCommand;
use crate::lobby::{RoomRegistry, SessionStore};

/// Minimum spacing between accepted actions on one connection.
const RATE_LIMIT: Duration = Duration::from_millis(100);

/// Shared application state handed to every connection.
#[derive(Clone)]
pub struct AppState {
    /// Anonymous session authentication.
    pub sessions: Arc<SessionStore>,
    /// The live-room registry.
    pub rooms: Arc<RoomRegistry>,
}

/// Build the Axum router for the WebSocket endpoint.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Read the next decodable client message from the socket, skipping non-binary
/// frames and returning `None` on close or error.
async fn next_client_message(stream: &mut SplitStream<WebSocket>) -> Option<ClientMessage> {
    while let Some(frame) = stream.next().await {
        match frame {
            Ok(Message::Binary(data)) => {
                if let Ok(msg) = codec::decode::<ClientMessage>(data.as_ref()) {
                    return Some(msg);
                }
            }
            Ok(Message::Close(_)) | Err(_) => return None,
            _ => continue, // ping/pong/text are ignored
        }
    }
    None
}

/// Drive one WebSocket connection end to end.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<ServerMessage>(64);

    // Writer task: serialise server messages onto the socket.
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let Ok(bytes) = codec::encode(&msg) else {
                break;
            };
            if sink.send(Message::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
    });

    // Handshake. On a rejected handshake, drop the sender and let the writer
    // drain (so the client actually receives the `Error`) rather than aborting.
    let Some(entry) = next_client_message(&mut stream).await else {
        drop(out_tx);
        let _ = writer.await;
        return;
    };
    let Some((player, room_tx)) = handle_entry(&state, entry, &out_tx).await else {
        drop(out_tx);
        let _ = writer.await;
        return;
    };

    // Action loop with per-connection rate limiting.
    let mut last = Instant::now()
        .checked_sub(RATE_LIMIT)
        .unwrap_or_else(Instant::now);
    while let Some(msg) = next_client_message(&mut stream).await {
        let now = Instant::now();
        if now.duration_since(last) < RATE_LIMIT {
            continue; // silently drop excess
        }
        last = now;
        if room_tx
            .send(RoomCommand::Action { player, msg })
            .await
            .is_err()
        {
            break;
        }
    }

    let _ = room_tx.send(RoomCommand::Leave { player }).await;
    writer.abort();
}

/// Validate the entry message, authenticate, and join/create a room. Returns the
/// player's id and the room's command channel, or `None` after sending an error.
async fn handle_entry(
    state: &AppState,
    entry: ClientMessage,
    out: &mpsc::Sender<ServerMessage>,
) -> Option<(PlayerId, mpsc::Sender<RoomCommand>)> {
    let (version, name, token, target) = match entry {
        ClientMessage::JoinRoom {
            protocol_version,
            display_name,
            session_token,
            room_code,
        } => (
            protocol_version,
            display_name,
            session_token,
            Some(room_code),
        ),
        ClientMessage::CreateRoom {
            protocol_version,
            display_name,
            session_token,
        } => (protocol_version, display_name, session_token, None),
        ClientMessage::EnqueueMatch { .. } => {
            send_error(
                out,
                ErrorCode::WrongPhase,
                "matchmaking is not yet available",
            )
            .await;
            return None;
        }
        _ => {
            send_error(
                out,
                ErrorCode::WrongPhase,
                "expected CreateRoom or JoinRoom",
            )
            .await;
            return None;
        }
    };

    if version != PROTOCOL_VERSION {
        send_error(
            out,
            ErrorCode::VersionMismatch,
            &format!("server speaks protocol version {PROTOCOL_VERSION}"),
        )
        .await;
        return None;
    }

    let (player, _token) = state.sessions.authenticate(token.as_deref());

    let room_tx = match target {
        None => state.rooms.create().1,
        Some(code) => match state.rooms.get(&code) {
            Some(tx) => tx,
            None => {
                send_error(out, ErrorCode::UnknownRoom, "no such room").await;
                return None;
            }
        },
    };

    if room_tx
        .send(RoomCommand::Join {
            player,
            name,
            out: out.clone(),
        })
        .await
        .is_err()
    {
        return None;
    }
    Some((player, room_tx))
}

async fn send_error(out: &mpsc::Sender<ServerMessage>, code: ErrorCode, message: &str) {
    let _ = out
        .send(ServerMessage::Error {
            code,
            message: message.to_string(),
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::client::PROTOCOL_VERSION;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message as TMsg;

    async fn start_server() -> String {
        let mut config =
            crate::config::ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
        // Short wave timers so tests don't wait out the real 30s/10s budgets.
        config.timing.wave1_ms = 250;
        config.timing.wave_ms = 200;
        let registry = Arc::new(config.build_registry().unwrap());
        let config = Arc::new(config);
        let state = AppState {
            sessions: Arc::new(SessionStore::new()),
            rooms: Arc::new(RoomRegistry::new(registry, config)),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app(state)).await.unwrap();
        });
        format!("ws://{addr}/ws")
    }

    fn create(name: &str, version: u16) -> ClientMessage {
        ClientMessage::CreateRoom {
            protocol_version: version,
            display_name: name.into(),
            session_token: None,
        }
    }

    /// A helper that connects and returns the socket.
    type Ws = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    async fn send(ws: &mut Ws, msg: &ClientMessage) {
        ws.send(TMsg::Binary(codec::encode(msg).unwrap()))
            .await
            .unwrap();
    }

    async fn recv(ws: &mut Ws) -> ServerMessage {
        loop {
            match ws.next().await.unwrap().unwrap() {
                TMsg::Binary(b) => return codec::decode(b.as_ref()).unwrap(),
                _ => continue,
            }
        }
    }

    /// Non-panicking receive: `None` once the connection closes.
    async fn recv_opt(ws: &mut Ws) -> Option<ServerMessage> {
        loop {
            match ws.next().await {
                Some(Ok(TMsg::Binary(b))) => return codec::decode(b.as_ref()).ok(),
                Some(Ok(_)) => continue,
                Some(Err(_)) | None => return None,
            }
        }
    }

    fn join(name: &str, code: boiling_point_protocol::RoomCode) -> ClientMessage {
        ClientMessage::JoinRoom {
            protocol_version: PROTOCOL_VERSION,
            display_name: name.into(),
            session_token: None,
            room_code: code,
        }
    }

    /// A trivial auto-player: each round it plays its hand by index, one card per
    /// wave, then passes; it stops at `GameOver`. Returns whether it saw GameOver.
    async fn play_loop(ws: &mut Ws) -> bool {
        let mut hand: Vec<boiling_point_protocol::CardId> = Vec::new();
        let mut idx = 0usize;
        loop {
            let Some(msg) = recv_opt(ws).await else {
                return false;
            };
            match msg {
                ServerMessage::YourHand { cards } => {
                    hand = cards.iter().map(|c| c.id).collect();
                    idx = 0;
                }
                ServerMessage::WaveOpened { .. } => {
                    // Rely on the (short, in tests) wave timer to close; sending a
                    // LockIn here would be dropped by the 100ms rate limit anyway.
                    if idx < hand.len() {
                        send(ws, &ClientMessage::CommitCard { card: hand[idx] }).await;
                        idx += 1;
                    } else {
                        send(ws, &ClientMessage::CommitPass).await;
                    }
                }
                ServerMessage::GameOver { .. } => return true,
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn emotes_validate_broadcast_and_lobby_carries_no_secret() {
        use boiling_point_protocol::EmoteId;
        let url = start_server().await;
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(&mut ws, &create("p1", PROTOCOL_VERSION)).await;
        let joined = recv(&mut ws).await;
        assert!(matches!(joined, ServerMessage::RoomJoined { .. }));
        // A lobby message must not carry any secret (e.g. the boiling point).
        assert!(!codec::encode_json(&joined)
            .unwrap()
            .contains("boiling_point"));

        // A palette emote is echoed back as a broadcast.
        send(&mut ws, &ClientMessage::Emote { emote: EmoteId(1) }).await;
        match recv(&mut ws).await {
            ServerMessage::EmoteBroadcast { emote, .. } => assert_eq!(emote.0, 1),
            other => panic!("expected EmoteBroadcast, got {other:?}"),
        }

        // An off-palette emote is rejected (spaced past the rate limit).
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        send(
            &mut ws,
            &ClientMessage::Emote {
                emote: EmoteId(999),
            },
        )
        .await;
        assert!(matches!(
            recv(&mut ws).await,
            ServerMessage::Error {
                code: ErrorCode::InvalidEmote,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn join_and_leave_are_broadcast() {
        let url = start_server().await;
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::RoomJoined { room_code, .. } = recv(&mut ws1).await {
                break room_code;
            }
        };

        let (mut ws2, _) = connect_async(url).await.unwrap();
        send(&mut ws2, &join("p2", code)).await;

        // The first player sees the second connect.
        let connected = loop {
            if let ServerMessage::PlayerConnectionChanged { connected, .. } = recv(&mut ws1).await {
                break connected;
            }
        };
        assert!(connected);

        // When the second disconnects, the first sees the drop.
        drop(ws2);
        let saw_disconnect = loop {
            match recv_opt(&mut ws1).await {
                Some(ServerMessage::PlayerConnectionChanged {
                    connected: false, ..
                }) => break true,
                Some(_) => continue,
                None => break false,
            }
        };
        assert!(saw_disconnect);
    }

    #[tokio::test]
    async fn four_clients_play_a_full_game_to_game_over() {
        let url = start_server().await;

        // Player 1 creates the room and learns the invite code.
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::RoomJoined { room_code, .. } = recv(&mut ws1).await {
                break room_code;
            }
        };

        // Players 2–4 join by code; the fourth join starts the game.
        let mut joiners = Vec::new();
        for i in 2..=4 {
            let url = url.clone();
            let code = code.clone();
            joiners.push(tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &join(&format!("p{i}"), code)).await;
                loop {
                    if matches!(recv(&mut ws).await, ServerMessage::RoomJoined { .. }) {
                        break;
                    }
                }
                play_loop(&mut ws).await
            }));
        }
        let creator = tokio::spawn(async move { play_loop(&mut ws1).await });

        // The whole game should finish well within the timeout (waves close early
        // once everyone locks in).
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            let mut all = creator.await.unwrap();
            for j in joiners {
                all &= j.await.unwrap();
            }
            all
        })
        .await
        .expect("game completed before timeout");
        assert!(outcome, "every client saw GameOver");
    }

    #[tokio::test]
    async fn create_room_handshake_returns_room_joined() {
        let url = start_server().await;
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(&mut ws, &create("alice", PROTOCOL_VERSION)).await;
        assert!(matches!(
            recv(&mut ws).await,
            ServerMessage::RoomJoined { .. }
        ));
    }

    #[tokio::test]
    async fn incompatible_version_is_rejected() {
        let url = start_server().await;
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(&mut ws, &create("bob", PROTOCOL_VERSION + 1)).await;
        assert!(matches!(
            recv(&mut ws).await,
            ServerMessage::Error {
                code: ErrorCode::VersionMismatch,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn heartbeat_round_trips_after_join() {
        let url = start_server().await;
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(&mut ws, &create("cara", PROTOCOL_VERSION)).await;
        assert!(matches!(
            recv(&mut ws).await,
            ServerMessage::RoomJoined { .. }
        ));
        send(&mut ws, &ClientMessage::Heartbeat).await;
        assert!(matches!(recv(&mut ws).await, ServerMessage::Heartbeat));
    }
}
