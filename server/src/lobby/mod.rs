//! Lobby: anonymous session auth, invite codes, group actors, and the group
//! registry. The auto-match queue is a later task; invite-link groups ship here.

pub mod accounts;
pub mod codes;
pub mod group;
pub mod matchmaking;
pub mod policy;
pub mod registry;
pub mod session;

pub use accounts::{Account, AccountError, AccountStore, OAuthVerifier, SignInCredential};
pub use group::{GroupCommand, GroupHandle};
pub use matchmaking::MatchQueue;
pub use policy::{Candidate, FirstCome, MatchPolicy, SkillBased};
pub use registry::{GameSeedSource, GroupRegistry, QueuedSeeds};
pub use session::SessionStore;
