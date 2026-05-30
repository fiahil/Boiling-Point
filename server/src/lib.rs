//! Boiling Point authoritative game server.
//!
//! This crate owns the full, authoritative domain model — including secrets the
//! wire must never carry (the boiling point, the deck, every hand) — and the
//! game loop. It depends on the `boiling-point-protocol` crate only for the wire
//! vocabulary it speaks to clients.
//!
//! Layering (built out across the implementation tasks):
//! - [`content`] + [`config`]: the churning, config-driven content behind a
//!   validated startup gate, kept strictly separate from the loop.
//! - (forthcoming) `transport`, `lobby`, `matchmaking`, `game`, `persistence`,
//!   `observability`.

pub mod config;
pub mod content;
pub mod game;
pub mod lobby;
pub mod persistence;
pub mod session;
pub mod transport;
