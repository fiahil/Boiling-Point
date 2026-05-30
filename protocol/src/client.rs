//! Messages sent from a client (or bot) to the server.
//!
//! All client messages are fire-and-forget: the server validates each one and
//! either applies it or replies with an [`crate::server::Error`], never partially.

use serde::{Deserialize, Serialize};

use crate::ids::{CardId, EmoteId, RoomCode};

/// The protocol version a client speaks, sent on the first (entry) message so
/// the server can reject incompatible clients before sharing any state.
pub type ProtocolVersion = u16;

/// The current wire protocol version.
pub const PROTOCOL_VERSION: ProtocolVersion = 1;

/// A message from client to server. Enum-tagged so a JSON fallback stays
/// human-readable for debugging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Join an existing room by its invite code (entry message; carries the version).
    JoinRoom {
        /// Protocol version the client speaks.
        protocol_version: ProtocolVersion,
        /// Display name to use at the table.
        display_name: String,
        /// Prior session token, to resume an existing identity if presented.
        session_token: Option<String>,
        /// The invite code of the room to join.
        room_code: RoomCode,
    },
    /// Create a fresh room and receive its invite code (entry message).
    CreateRoom {
        /// Protocol version the client speaks.
        protocol_version: ProtocolVersion,
        /// Display name to use at the table.
        display_name: String,
        /// Prior session token, to resume an existing identity if presented.
        session_token: Option<String>,
    },
    /// Enter the auto-match queue to be assembled into a table of four (entry message).
    EnqueueMatch {
        /// Protocol version the client speaks.
        protocol_version: ProtocolVersion,
        /// Display name to use at the table.
        display_name: String,
        /// Prior session token, to resume an existing identity if presented.
        session_token: Option<String>,
    },
    /// Commit a card into the current wave (hidden until the wave reveals).
    CommitCard {
        /// The hand card to commit.
        card: CardId,
    },
    /// Commit to passing this wave (permanent lockout for the round).
    CommitPass,
    /// Lock in the current selection; if all active players lock in, the wave closes early.
    LockIn,
    /// Send a preset emote to the table (the only communication channel).
    Emote {
        /// The palette emote to send.
        emote: EmoteId,
    },
    /// Liveness keepalive.
    Heartbeat,
}
