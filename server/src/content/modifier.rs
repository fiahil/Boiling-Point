//! Cauldron-modifier behaviour, modelled with the Strategy pattern.
//!
//! Every modifier exposes a single pure offset or multiplier, so the escalation
//! engine can stack them by plain arithmetic and contradictions simply cancel.
//! Modifier behaviour is a distinct content kind from cards and effects.

use boiling_point_protocol::vocab::ModifierKind;

/// A cauldron modifier's pure contribution to the round's parameters. Defaults
/// are no-ops so each concrete modifier overrides exactly one method.
pub trait Modifier: Send + Sync {
    /// The tag identifying this modifier on the wire and in config.
    fn kind(&self) -> ModifierKind;
    /// Additive shift to the boiling point (negative = more dangerous).
    fn boiling_point_delta(&self) -> i16 {
        0
    }
    /// Volatility the cauldron starts the round with.
    fn start_volatility(&self) -> u8 {
        0
    }
    /// Colourless bonus added to the pot total per card played.
    fn pot_point_bonus_per_card(&self) -> u32 {
        0
    }
    /// Multiplier applied to the final pot value (win and explosion alike).
    fn pot_multiplier(&self) -> u32 {
        1
    }
    /// Whether dominance is reversed (lowest colour present wins).
    fn reverses_dominance(&self) -> bool {
        false
    }
}

// Default magnitudes — all `[needs playtesting]`.
const RESIDUE_START: u8 = 3;
const THIN_ICE_DELTA: i16 = -4;
const DEEP_CAULDRON_DELTA: i16 = 4;
const BOUNTIFUL_PER_CARD: u32 = 1;
const DOUBLE_STAKES_MULT: u32 = 2;

/// Cauldron starts with extra volatility.
pub struct Residue;
impl Modifier for Residue {
    fn kind(&self) -> ModifierKind {
        ModifierKind::Residue
    }
    fn start_volatility(&self) -> u8 {
        RESIDUE_START
    }
}

/// Boiling point lowered — explosions far more likely.
pub struct ThinIce;
impl Modifier for ThinIce {
    fn kind(&self) -> ModifierKind {
        ModifierKind::ThinIce
    }
    fn boiling_point_delta(&self) -> i16 {
        THIN_ICE_DELTA
    }
}

/// Boiling point raised — explosions rare.
pub struct DeepCauldron;
impl Modifier for DeepCauldron {
    fn kind(&self) -> ModifierKind {
        ModifierKind::DeepCauldron
    }
    fn boiling_point_delta(&self) -> i16 {
        DEEP_CAULDRON_DELTA
    }
}

/// Colourless +1 to the pot total per card played — swells value and blast, not dominance.
pub struct BountifulBrew;
impl Modifier for BountifulBrew {
    fn kind(&self) -> ModifierKind {
        ModifierKind::BountifulBrew
    }
    fn pot_point_bonus_per_card(&self) -> u32 {
        BOUNTIFUL_PER_CARD
    }
}

/// All pot points doubled — the win and the explosion loss.
pub struct DoubleStakes;
impl Modifier for DoubleStakes {
    fn kind(&self) -> ModifierKind {
        ModifierKind::DoubleStakes
    }
    fn pot_multiplier(&self) -> u32 {
        DOUBLE_STAKES_MULT
    }
}

/// The lowest-point colour present in the pot wins instead of the highest.
pub struct Reversal;
impl Modifier for Reversal {
    fn kind(&self) -> ModifierKind {
        ModifierKind::Reversal
    }
    fn reverses_dominance(&self) -> bool {
        true
    }
}

/// Construct the behaviour strategy object for a modifier kind.
pub fn behavior_for(kind: ModifierKind) -> Box<dyn Modifier> {
    match kind {
        ModifierKind::Residue => Box::new(Residue),
        ModifierKind::ThinIce => Box::new(ThinIce),
        ModifierKind::DeepCauldron => Box::new(DeepCauldron),
        ModifierKind::BountifulBrew => Box::new(BountifulBrew),
        ModifierKind::DoubleStakes => Box::new(DoubleStakes),
        ModifierKind::Reversal => Box::new(Reversal),
    }
}
