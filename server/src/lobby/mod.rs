//! Lobby: anonymous session auth, invite codes, room actors, and the room
//! registry. The auto-match queue is a later task; invite-link rooms ship here.

pub mod codes;
pub mod matchmaking;
pub mod registry;
pub mod room;
pub mod session;

pub use matchmaking::MatchQueue;
pub use registry::RoomRegistry;
pub use room::{RoomCommand, RoomHandle};
pub use session::SessionStore;
