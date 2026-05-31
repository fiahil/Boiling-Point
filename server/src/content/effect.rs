//! Special-effect behaviour, modelled with the Strategy pattern.
//!
//! Each effect is a strategy object implementing [`Effect`]; the game loop never
//! matches on a concrete effect, it only asks for the behaviour keyed by an
//! [`EffectKind`] tag and runs it against an [`EffectCtx`] port. Adding or
//! retuning an effect therefore never touches the loop.

use boiling_point_protocol::vocab::{Color, EffectKind};

/// Where an effect falls in the fixed within-wave resolution order. The resolver
/// runs categories in this ascending order against a settled, pre-wave snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectCategory {
    /// Round-state flag set when played (e.g. Shield); resolves with card-add.
    State,
    /// Volatility adjustments (Dampen, Volatile Surge).
    Volatility,
    /// Colour/identity changes (Copycat).
    Identity,
    /// Point changes (Double Down).
    Points,
    /// Removal (Recall).
    Removal,
    /// Information reads, reported last (Peek, Expose).
    Information,
}

/// The operations an effect may perform on the resolving pot. The game engine
/// implements this port over its authoritative `WaveResolution`; content depends
/// only on this abstraction (never on engine internals).
pub trait EffectCtx {
    /// The colour of the card carrying the currently-resolving effect.
    fn card_color(&self) -> Color;
    /// Adjust the cauldron's running volatility by `delta` (may be negative).
    fn adjust_volatility(&mut self, delta: i16);
    /// Privately reveal the exact boiling point to the player who played this effect.
    fn reveal_boiling_point_to_actor(&mut self);
    /// Mark the acting player as shielded for the round.
    fn shield_actor(&mut self);
    /// Reveal one random face-down pot card to the whole table.
    fn expose_random_card(&mut self);
    /// Set this card's colour to the dominant colour of the pre-wave pot.
    fn adopt_pre_wave_dominant_color(&mut self);
    /// Retrieve one of the acting player's own previously-played cards.
    fn recall_own_card(&mut self);
    /// Double the pre-wave point total of `color` (additive against the snapshot).
    fn double_pre_wave_color_points(&mut self, color: Color);
}

/// A special effect's behaviour. Implementors are stateless strategy objects.
pub trait Effect: Send + Sync {
    /// The tag identifying this effect on the wire and in config.
    fn kind(&self) -> EffectKind;
    /// Which resolution category (and therefore order) this effect belongs to.
    fn category(&self) -> EffectCategory;
    /// Apply this effect against the resolving pot via the engine-provided port.
    fn resolve(&self, ctx: &mut dyn EffectCtx);
}

/// Volatility magnitudes are starting defaults — `[needs playtesting]`.
const DAMPEN_REDUCTION: i16 = -2;
const SURGE_BONUS: i16 = 2;

/// Privately reveals the boiling point to the player who played it.
pub struct Peek;
impl Effect for Peek {
    fn kind(&self) -> EffectKind {
        EffectKind::Peek
    }
    fn category(&self) -> EffectCategory {
        EffectCategory::Information
    }
    fn resolve(&self, ctx: &mut dyn EffectCtx) {
        ctx.reveal_boiling_point_to_actor();
    }
}

/// Reduces total cauldron volatility (a safety play).
pub struct Dampen;
impl Effect for Dampen {
    fn kind(&self) -> EffectKind {
        EffectKind::Dampen
    }
    fn category(&self) -> EffectCategory {
        EffectCategory::Volatility
    }
    fn resolve(&self, ctx: &mut dyn EffectCtx) {
        ctx.adjust_volatility(DAMPEN_REDUCTION);
    }
}

/// Adds extra volatility on top of the card's base (a weapon).
pub struct VolatileSurge;
impl Effect for VolatileSurge {
    fn kind(&self) -> EffectKind {
        EffectKind::VolatileSurge
    }
    fn category(&self) -> EffectCategory {
        EffectCategory::Volatility
    }
    fn resolve(&self, ctx: &mut dyn EffectCtx) {
        ctx.adjust_volatility(SURGE_BONUS);
    }
}

/// Grants the acting player explosion immunity this round (forfeiting scoring if safe).
pub struct Shield;
impl Effect for Shield {
    fn kind(&self) -> EffectKind {
        EffectKind::Shield
    }
    fn category(&self) -> EffectCategory {
        EffectCategory::State
    }
    fn resolve(&self, ctx: &mut dyn EffectCtx) {
        ctx.shield_actor();
    }
}

/// Reveals one random face-down pot card to the whole table.
pub struct Expose;
impl Effect for Expose {
    fn kind(&self) -> EffectKind {
        EffectKind::Expose
    }
    fn category(&self) -> EffectCategory {
        EffectCategory::Information
    }
    fn resolve(&self, ctx: &mut dyn EffectCtx) {
        ctx.expose_random_card();
    }
}

/// Adopts the dominant colour already in the pot from previous waves.
pub struct Copycat;
impl Effect for Copycat {
    fn kind(&self) -> EffectKind {
        EffectKind::Copycat
    }
    fn category(&self) -> EffectCategory {
        EffectCategory::Identity
    }
    fn resolve(&self, ctx: &mut dyn EffectCtx) {
        ctx.adopt_pre_wave_dominant_color();
    }
}

/// Retrieves one of the player's own previously-played cards.
pub struct Recall;
impl Effect for Recall {
    fn kind(&self) -> EffectKind {
        EffectKind::Recall
    }
    fn category(&self) -> EffectCategory {
        EffectCategory::Removal
    }
    fn resolve(&self, ctx: &mut dyn EffectCtx) {
        ctx.recall_own_card();
    }
}

/// Doubles the points of its colour already in the pot from previous waves.
pub struct DoubleDown;
impl Effect for DoubleDown {
    fn kind(&self) -> EffectKind {
        EffectKind::DoubleDown
    }
    fn category(&self) -> EffectCategory {
        EffectCategory::Points
    }
    fn resolve(&self, ctx: &mut dyn EffectCtx) {
        let color = ctx.card_color();
        ctx.double_pre_wave_color_points(color);
    }
}

/// Construct the behaviour strategy object for an effect kind.
pub fn behavior_for(kind: EffectKind) -> Box<dyn Effect> {
    match kind {
        EffectKind::Peek => Box::new(Peek),
        EffectKind::Dampen => Box::new(Dampen),
        EffectKind::VolatileSurge => Box::new(VolatileSurge),
        EffectKind::Shield => Box::new(Shield),
        EffectKind::Expose => Box::new(Expose),
        EffectKind::Copycat => Box::new(Copycat),
        EffectKind::Recall => Box::new(Recall),
        EffectKind::DoubleDown => Box::new(DoubleDown),
    }
}

/// Every effect kind the rules reference — used by config validation to ensure
/// none is silently missing.
pub const ALL_EFFECT_KINDS: [EffectKind; 8] = [
    EffectKind::Peek,
    EffectKind::Dampen,
    EffectKind::VolatileSurge,
    EffectKind::Shield,
    EffectKind::Expose,
    EffectKind::Copycat,
    EffectKind::Recall,
    EffectKind::DoubleDown,
];
