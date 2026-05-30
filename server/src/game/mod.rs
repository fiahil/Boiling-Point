//! The authoritative game engine — the stable loop that operates on the domain
//! model and the content abstractions, never on specific named content.
//!
//! This module owns the secrets the wire must never carry (the deck, the pot's
//! true contents, the boiling point). Submodules:
//! - [`card`]: the concrete card instance,
//! - [`deck`]: build / deal-to-5 / reshuffle,
//! - [`pot`]: the accumulating round pot,
//! - [`modifiers`]: the cumulative modifier stack,
//! - [`resolve`]: the within-wave effect resolver,
//! - [`scoring`]: dominance, winner-takes-all, and explosions.

pub mod card;
pub mod deck;
pub mod modifiers;
pub mod pot;
pub mod resolve;
pub mod scoring;

pub use card::Card;
pub use deck::Deck;
pub use modifiers::ActiveModifiers;
pub use pot::{Pot, PotCard};
pub use resolve::{resolve_wave, WaveOutcome};
pub use scoring::{explosion, pot_value, score_safe, ExplosionResult, SafeScore, ScoringContext};
