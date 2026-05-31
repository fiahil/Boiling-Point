//! Stable identifier newtypes that appear on the wire.
//!
//! These are deliberately thin wrappers (not behaviour-bearing domain types) so
//! the protocol crate stays a pure transport vocabulary shared by every side.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable, server-issued identity for a player (persists across reconnects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(pub Uuid);

impl PlayerId {
    /// Mint a fresh random player id.
    pub fn new() -> Self {
        PlayerId(Uuid::new_v4())
    }
}

impl Default for PlayerId {
    fn default() -> Self {
        Self::new()
    }
}

/// Short, human-readable invite code for a room (e.g. `BREW-7K3F`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomCode(pub String);

/// Identifier for a single physical card instance within a game.
///
/// Lets a client reference a card in their hand when committing, without the
/// server ever trusting the client's view of that card's attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardId(pub u32);

/// Identifier for a preset emote in the configured table-talk palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmoteId(pub u16);
