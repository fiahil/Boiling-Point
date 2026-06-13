//! The authoritative game engine — the stable loop that operates on the domain
//! model and the content abstractions, never on specific named content.
//!
//! This module owns the secrets the wire must never carry (the decks, the pot's
//! true contents, the primed Actives, the boiling point). Submodules:
//! - [`card`]: the concrete ingredient and spell instances,
//! - [`deck`]: the per-player pantry (top-up-to-3) and grimoire (round-start draw),
//! - [`pot`]: the accumulating round pot (running total, colour adjustments),
//! - [`modifiers`]: the cumulative modifier stack,
//! - [`brewers`]: the per-Brewer rule bends (`boom2-brewers`),
//! - [`realizer`]: the Apothecary deck realizer (`boom2-apothecary`) — recipes
//!   to fixed-size, capped, colour-anchored decks, re-rolled per game,
//! - [`spells`]: Instant resolution, Active prime/fire, ward/Hex/Harvest fires,
//! - [`round`]: the wave loop, detonator identification, and the sorted depile,
//! - [`scoring`]: the P-symmetry (safe-brew dominance, detonator explosion).

pub mod brewers;
pub mod card;
pub mod deathmatch;
pub mod deck;
pub mod modifiers;
pub mod pot;
pub mod realizer;
pub mod round;
pub mod runner;
pub mod scoring;
pub mod spells;
pub mod state;

pub use card::{Ingredient, Spell};
pub use deathmatch::{DeathmatchDecider, DeathmatchResult, run_deathmatch};
pub use deck::{Grimoire, Pantry};
pub use modifiers::ActiveModifiers;
pub use pot::{Pot, PotIngredient};
pub use round::{
    DepileData, DepileItem, Round, RoundEnd, SpellChoice, WaveAction, WaveChoice, WaveInput,
};
pub use runner::{Decider, Game, GameOutcome, RoundLog};
pub use scoring::{ExplosionResult, SafeScore, ScoringContext, explosion, pot_value, score_safe};
pub use spells::{CastCommit, PrimedSpell};
pub use state::{Hand, Player};
