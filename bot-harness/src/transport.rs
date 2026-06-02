//! The transport abstraction (D1): two backends behind one [`BotConnection`].
//!
//! A bot's decision loop ([`crate::bot::run_bot`]) is written once against this
//! trait and is oblivious to how bytes move. The default backend is
//! [`InProcess`] — the server's own game loop driven over in-memory channels, for
//! the fast, deterministic batch. [`WebSocket`] runs the *real* wire path
//! (MessagePack over a socket) for a smaller honesty-check batch. Both carry the
//! identical [`ServerMessage`]/[`ClientMessage`] types, so neither the bot nor a
//! strategy can tell them apart.

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use boiling_point_protocol::{ClientMessage, PlayerId, ServerMessage, codec};
use boiling_point_server::lobby::RoomCommand;

/// A bidirectional channel to the server, in the bot's own vocabulary.
///
/// Implementations move [`ClientMessage`]s out and [`ServerMessage`]s in; `recv`
/// yields `None` once the game/connection ends.
pub trait BotConnection {
    /// Await the next server message, or `None` at end of stream.
    fn recv(&mut self) -> impl std::future::Future<Output = Option<ServerMessage>>;
    /// Send a client message (best effort — a closed connection is silently dropped).
    fn send(&mut self, msg: ClientMessage) -> impl std::future::Future<Output = ()>;
}

/// In-process backend: talks straight to `session::run_game` over mpsc channels,
/// with no socket or serialization. Outbound client messages are wrapped as the
/// room's [`RoomCommand::Action`]s the way the real transport layer wraps them.
pub struct InProcess {
    /// This seat's player id, stamped onto every outbound action.
    player: PlayerId,
    /// Channel into the game loop's shared command receiver.
    tx: mpsc::Sender<RoomCommand>,
    /// This seat's private inbound message stream.
    rx: mpsc::Receiver<ServerMessage>,
}

impl InProcess {
    /// Build an in-process connection for one seat.
    pub fn new(
        player: PlayerId,
        tx: mpsc::Sender<RoomCommand>,
        rx: mpsc::Receiver<ServerMessage>,
    ) -> Self {
        InProcess { player, tx, rx }
    }
}

impl BotConnection for InProcess {
    async fn recv(&mut self) -> Option<ServerMessage> {
        self.rx.recv().await
    }

    async fn send(&mut self, msg: ClientMessage) {
        let _ = self
            .tx
            .send(RoomCommand::Action {
                player: self.player,
                msg,
            })
            .await;
    }
}

/// WebSocket backend: the real wire path, encoding/decoding MessagePack exactly as
/// a production client does. Skips non-binary frames; `recv` ends on close/error.
pub struct WebSocket {
    /// The underlying duplex socket stream.
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl WebSocket {
    /// Connect to a server's `/ws` endpoint and wrap the socket.
    pub async fn connect(url: &str) -> Result<Self, tokio_tungstenite::tungstenite::Error> {
        let (stream, _resp) = tokio_tungstenite::connect_async(url).await?;
        Ok(WebSocket { stream })
    }
}

impl BotConnection for WebSocket {
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

    async fn send(&mut self, msg: ClientMessage) {
        if let Ok(bytes) = codec::encode(&msg) {
            let _ = self.stream.send(WsMessage::Binary(bytes)).await;
        }
    }
}
