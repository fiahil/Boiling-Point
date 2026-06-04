//! Lobby: anonymous session auth, invite codes, group actors, and the group
//! registry. The auto-match queue is a later task; invite-link groups ship here.

pub mod codes;
pub mod group;
pub mod matchmaking;
pub mod registry;
pub mod session;

pub use group::{GroupCommand, GroupHandle};
pub use matchmaking::MatchQueue;
pub use registry::GroupRegistry;
pub use session::SessionStore;
