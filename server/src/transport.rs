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
    use std::collections::HashSet;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message as TMsg;

    async fn start_server() -> String {
        let palette: HashSet<u16> = [1u16, 2, 3].into_iter().collect();
        let state = AppState {
            sessions: Arc::new(SessionStore::new()),
            rooms: Arc::new(RoomRegistry::new(Arc::new(palette))),
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
