//! Spell content: the grimoire's composition and the tunable spell magnitudes.
//!
//! Spell *identity* (which of the 15 kinds, its Instant/Active mode, its target
//! kind) is static design metadata living on
//! [`boiling_point_protocol::vocab::SpellKind`]; what churns during balance
//! playtesting — how many copies of each spell a grimoire holds, and each
//! magnitude (Dampen −3, Hex +5, …) — lives here, file-driven, so retuning a
//! spell never touches the engine.

use serde::{Deserialize, Serialize};

use boiling_point_protocol::vocab::SpellKind;

/// A spell archetype as defined in content config: which spell and how many
/// copies of it each player's grimoire holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellDef {
    /// Which of the fifteen spells.
    pub kind: SpellKind,
    /// Number of copies in the grimoire.
    pub copies: u16,
}

/// The tunable spell magnitudes — all `[needs playtesting]`, scaled to the
/// 0–7 volatility range and P≈10 pots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpellValues {
    /// Dampen: cauldron volatility reduction.
    pub dampen: u8,
    /// Surge: cauldron volatility increase.
    pub surge: u8,
    /// Cap: the most a Capped detonator can lose.
    pub cap_max: u8,
    /// Hex: extra damage the hexed player takes on any explosion.
    pub hex_bonus: u8,
    /// Harvest: bonus on top of a won pot.
    pub harvest_bonus: u8,
    /// Forage: spells drawn.
    pub forage_draws: u8,
}
