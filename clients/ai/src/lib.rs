//! Boiling Point AI client (change `boom2-ai-client`).
//!
//! One Rust client core serving two masters: **harness mode** (the Principle IV
//! reinstatement — seeded batch runs over an in-process server) and
//! **seat-filler mode** (AI seats in real rooms over WebSocket). Two brains plug
//! into one [`brain::Brain`] trait: the deterministic [`bot`] brain (instant,
//! zero-cost, reproducible) and the Claude-driven [`agent`] brain (persona play
//! under a hard timeliness/cost contract).
//!
//! **Firewall (design D2):** this crate depends on `protocol/` alone — never on
//! the server crate or its domain types (the `harness` feature is the one
//! sanctioned exception, scoped to the batch-runner host, and even there the
//! seat↔server boundary carries encoded wire frames only). The [`view`] model is
//! rebuilt from received messages and structurally cannot hold a secret. The
//! decision loop ([`seat`]) submits only actions enumerated by the server's
//! decision frames and races every brain against a latency budget so a seat
//! never stalls a wave.

pub mod agent;
pub mod bot;
pub mod brain;
pub mod observe;
pub mod policy;
pub mod seat;
pub mod transport;
pub mod view;

/// Errors an AI-client seat can hit.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// A server message disclosed something the seat must never receive — the
    /// live secret-boundary check failed (this is a server bug, not a client one).
    #[error("secret boundary breach: {0}")]
    SecretLeak(String),
    /// The transport failed (connect/handshake/socket).
    #[error("transport: {0}")]
    Transport(String),
    /// Invalid configuration.
    #[error("config: {0}")]
    Config(String),
    /// The game/connection ended before `GameOver`.
    #[error("incomplete game: {0}")]
    Incomplete(String),
}
