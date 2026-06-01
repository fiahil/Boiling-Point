//! Boiling Point Layer-1 bot / balance harness.
//!
//! The constitution makes the headless bot harness a first-class testing layer
//! (Principle II) and the primary instrument for data-informed balance
//! (Principle IV): every `[needs playtesting]` number in the content config is a
//! hypothesis until thousands of bot games confirm a healthy explosion rate and
//! surface degenerate strategies. This crate is that harness.
//!
//! It is a strict *consumer* of `server-release-1`: bots speak only the public
//! [`boiling_point_protocol`] wire types and reason over a player-visible model
//! ([`model`]), so the harness can never see a server secret and doubles as a
//! continuous test of the no-leak contract. Layout:
//!
//! - [`rng`] — the deterministic seed tree (D4).
//! - [`model`] — the bot's player-visible world (D2).
//! - [`strategy`] — the pluggable decision trait and baselines (D3).
//! - [`transport`] — the in-process and WebSocket backends (D1).
//! - [`bot`] — the transport-agnostic play loop and secret audit.
//! - [`runner`] — the seeded batch runner that plays games and collects records.
//! - [`stats`] — balance-statistics aggregation.
//! - [`report`] — the structured report and balance-smell detection (D5).

pub mod bot;
pub mod model;
pub mod report;
pub mod rng;
pub mod runner;
pub mod stats;
pub mod strategy;
pub mod transport;

/// A fatal error in setting up or running the harness.
#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    /// A server message leaked a secret the bot must never receive (a breach of
    /// the Constitution I/II secret-management contract).
    #[error("secret-boundary violation: {0}")]
    SecretLeak(String),
    /// The content config could not be loaded or was invalid.
    #[error("content config error: {0}")]
    Config(String),
    /// A WebSocket connection or handshake failed.
    #[error("websocket error: {0}")]
    WebSocket(String),
    /// A game ended without reaching `GameOver`.
    #[error("game did not complete: {0}")]
    Incomplete(String),
}
