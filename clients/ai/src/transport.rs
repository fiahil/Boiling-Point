//! Two transports, one codec (D2): the real WebSocket wire and the in-process
//! byte-frame channels, both moving the **same encoded bytes** through the
//! production [`codec`]. A seat cannot tell them apart.
//!
//! Entry-handshake discipline: a connection's first frame MUST be an entry
//! message (join by code, create, or enqueue); nothing — not even a heartbeat —
//! is sent before [`enter`] completes. The WebSocket sender also paces table
//! actions past the server's per-connection rate limit so back-to-back
//! commit/cast/lock-in sends are never silently dropped.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use boiling_point_protocol::client::PROTOCOL_VERSION;
use boiling_point_protocol::vocab::Color;
use boiling_point_protocol::{ClientMessage, GroupCode, PlayerId, ServerMessage, codec};

use crate::ClientError;

/// A bidirectional connection in wire vocabulary: `ClientMessage`s out,
/// `ServerMessage`s in, the codec applied underneath on both implementations.
/// `recv` yields `None` once the connection/game ends.
pub trait Connection: Send {
    /// Await the next server message, or `None` at end of stream.
    fn recv(&mut self) -> impl std::future::Future<Output = Option<ServerMessage>> + Send;
    /// Encode and send a client message.
    fn send(
        &mut self,
        msg: &ClientMessage,
    ) -> impl std::future::Future<Output = Result<(), ClientError>> + Send;
}

/// The in-process transport: byte channels (e.g. the server's headless seam)
/// carrying encoded wire frames. Encoding/decoding happens HERE, through the
/// production codec, so in-process games exercise the same bytes as the socket.
pub struct ChannelConnection {
    /// Encoded client→server frames.
    to_server: mpsc::Sender<Vec<u8>>,
    /// Encoded server→client frames.
    from_server: mpsc::Receiver<Vec<u8>>,
}

impl ChannelConnection {
    /// Wrap a pair of byte-frame endpoints.
    pub fn new(to_server: mpsc::Sender<Vec<u8>>, from_server: mpsc::Receiver<Vec<u8>>) -> Self {
        ChannelConnection {
            to_server,
            from_server,
        }
    }
}

impl Connection for ChannelConnection {
    async fn recv(&mut self) -> Option<ServerMessage> {
        while let Some(bytes) = self.from_server.recv().await {
            // Mirror the socket: an undecodable frame is skipped, not fatal.
            if let Ok(msg) = codec::decode::<ServerMessage>(&bytes) {
                return Some(msg);
            }
        }
        None
    }

    async fn send(&mut self, msg: &ClientMessage) -> Result<(), ClientError> {
        let bytes = codec::encode(msg).map_err(|e| ClientError::Transport(e.to_string()))?;
        self.to_server
            .send(bytes)
            .await
            .map_err(|_| ClientError::Transport("in-process channel closed".into()))
    }
}

/// Whether a message is subject to the server's per-connection table-action
/// rate limit (mirrors the server's `is_table_action`).
fn is_table_action(msg: &ClientMessage) -> bool {
    matches!(
        msg,
        ClientMessage::CommitIngredient { .. }
            | ClientMessage::CastSpell { .. }
            | ClientMessage::CommitPass
            | ClientMessage::LockIn
            | ClientMessage::Emote { .. }
    )
}

/// The real-wire transport: MessagePack over WebSocket, exactly as any
/// production client. Table-action sends are paced just past the server's
/// 100ms per-connection rate limit so a commit+cast+lock-in burst is delivered
/// rather than dropped.
pub struct WsConnection {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// Minimum spacing between table-action sends.
    pace: Duration,
    /// When the last table action was sent.
    last_action: tokio::time::Instant,
}

impl WsConnection {
    /// Connect to a server's `/ws` endpoint.
    pub async fn connect(url: &str) -> Result<Self, ClientError> {
        let (stream, _resp) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        Ok(WsConnection {
            stream,
            pace: Duration::from_millis(110),
            last_action: tokio::time::Instant::now() - Duration::from_secs(1),
        })
    }
}

impl Connection for WsConnection {
    async fn recv(&mut self) -> Option<ServerMessage> {
        while let Some(frame) = self.stream.next().await {
            match frame {
                Ok(WsMessage::Binary(bytes)) => {
                    if let Ok(msg) = codec::decode::<ServerMessage>(bytes.as_ref()) {
                        return Some(msg);
                    }
                }
                Ok(WsMessage::Close(_)) | Err(_) => return None,
                _ => continue, // ping/pong/text are ignored
            }
        }
        None
    }

    async fn send(&mut self, msg: &ClientMessage) -> Result<(), ClientError> {
        if is_table_action(msg) {
            let due = self.last_action + self.pace;
            tokio::time::sleep_until(due).await;
            self.last_action = tokio::time::Instant::now();
        }
        let bytes = codec::encode(msg).map_err(|e| ClientError::Transport(e.to_string()))?;
        self.stream
            .send(WsMessage::Binary(bytes.into()))
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))
    }
}

/// How a seat enters the server (the permitted first frames).
#[derive(Debug, Clone)]
pub enum EntryMode {
    /// Join an existing group by invite code.
    Join(GroupCode),
    /// Create a fresh group (learn its code from `GroupJoined`).
    Create,
    /// Enter the matchmaking queue.
    Enqueue,
}

/// What a completed entry handshake established.
#[derive(Debug, Clone)]
pub struct Joined {
    /// The seat's server-assigned identity.
    pub player: PlayerId,
    /// The seat's colour.
    pub color: Color,
    /// The group's invite code.
    pub group_code: GroupCode,
    /// The session token to replay on reconnection.
    pub session_token: String,
}

/// Perform the entry handshake: the FIRST frame on the connection is the entry
/// message (never a heartbeat), then await `GroupJoined`. Any pre-join `Error`
/// fails the entry.
pub async fn enter<C: Connection>(
    conn: &mut C,
    mode: &EntryMode,
    display_name: &str,
    session_token: Option<String>,
) -> Result<Joined, ClientError> {
    let entry = match mode {
        EntryMode::Join(code) => ClientMessage::JoinGroup {
            protocol_version: PROTOCOL_VERSION,
            display_name: display_name.to_string(),
            session_token,
            group_code: code.clone(),
        },
        EntryMode::Create => ClientMessage::CreateGroup {
            protocol_version: PROTOCOL_VERSION,
            display_name: display_name.to_string(),
            session_token,
        },
        EntryMode::Enqueue => ClientMessage::EnqueueMatch {
            protocol_version: PROTOCOL_VERSION,
            display_name: display_name.to_string(),
            session_token,
        },
    };
    conn.send(&entry).await?;
    loop {
        match conn.recv().await {
            Some(ServerMessage::GroupJoined {
                group_code,
                your_player_id,
                your_color,
                session_token,
                ..
            }) => {
                return Ok(Joined {
                    player: your_player_id,
                    color: your_color,
                    group_code,
                    session_token,
                });
            }
            Some(ServerMessage::Error { code, message }) => {
                return Err(ClientError::Transport(format!(
                    "entry rejected: {code:?}: {message}"
                )));
            }
            Some(_) => continue, // lobby chatter before our join confirmation
            None => {
                return Err(ClientError::Transport(
                    "connection closed before GroupJoined".into(),
                ));
            }
        }
    }
}
