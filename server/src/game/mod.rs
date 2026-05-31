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
pub mod deathmatch;
pub mod deck;
pub mod modifiers;
pub mod pot;
pub mod resolve;
pub mod round;
pub mod runner;
pub mod scoring;
pub mod state;

pub use card::Card;
pub use deathmatch::{DeathmatchDecider, DeathmatchResult, run_deathmatch};
pub use deck::Deck;
pub use modifiers::ActiveModifiers;
pub use pot::{Pot, PotCard};
pub use resolve::{WaveOutcome, resolve_wave};
pub use round::{DepileData, DepileItem, Round, RoundEnd, WaveChoice, WaveInput, WaveReport};
pub use runner::{Decider, Game, GameOutcome, RoundLog};
pub use scoring::{ExplosionResult, SafeScore, ScoringContext, explosion, pot_value, score_safe};
pub use state::{Hand, Player};
