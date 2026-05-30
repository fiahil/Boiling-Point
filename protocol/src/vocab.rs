//! Shared wire vocabulary: the public value-types and tag enums that appear in
//! messages.
//!
//! These are *tags and views*, not behaviour. The behaviour keyed by
//! [`EffectKind`]/[`ModifierKind`] lives entirely in the server's content
//! module — the protocol only names them so the depile and reveals can describe
//! what happened.

use serde::{Deserialize, Serialize};

use crate::ids::CardId;

/// A card's colour — whose interests it serves. `Wild` belongs to no player and
/// never wins dominance, but its points still swell the pot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    /// Ruby Red player colour.
    Ruby,
    /// Sapphire Blue player colour.
    Sapphire,
    /// Emerald Green player colour.
    Emerald,
    /// Amethyst Purple player colour.
    Amethyst,
    /// Colourless wild — points but no dominance.
    Wild,
}

impl Color {
    /// The four player colours, excluding `Wild`.
    pub const PLAYER_COLORS: [Color; 4] = [
        Color::Ruby,
        Color::Sapphire,
        Color::Emerald,
        Color::Amethyst,
    ];
}

/// The eight special-effect kinds. The protocol uses this only as a tag (e.g. in
/// the depile); the resolving behaviour lives in `server::content::effect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    /// Privately learn the exact boiling point.
    Peek,
    /// Reduce total cauldron volatility.
    Dampen,
    /// Add extra volatility on top of the card's base.
    VolatileSurge,
    /// Immunity to this round's explosion, forfeiting scoring if it resolves safely.
    Shield,
    /// Reveal one random face-down pot card to the whole table.
    Expose,
    /// Adopt the dominant colour already in the pot from previous waves.
    Copycat,
    /// Retrieve one of the player's own previously-played cards.
    Recall,
    /// Double the points of its colour already in the pot from previous waves.
    DoubleDown,
}

/// The six cauldron-modifier kinds. A tag only; offsets/multipliers live in
/// `server::content::modifier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModifierKind {
    /// Cauldron starts with extra volatility.
    Residue,
    /// Boiling point lowered (explosions more likely).
    ThinIce,
    /// Boiling point raised (explosions rarer).
    DeepCauldron,
    /// Colourless per-card bonus to the pot total.
    BountifulBrew,
    /// All pot points multiplied — win and explosion alike.
    DoubleStakes,
    /// The lowest-point colour present wins instead of the highest.
    Reversal,
}

/// The fully-revealed attributes of a card, as shown in a hand (to its owner) or
/// at the depile (to everyone). Cards in the cauldron are NOT sent as `CardView`
/// during play — they are hidden until revealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardView {
    /// The card's colour.
    pub color: Color,
    /// Explosion risk this card contributes (1–3).
    pub volatility: u8,
    /// Point value for scoring (0–3).
    pub points: u8,
    /// The card's special effect, if any.
    pub effect: Option<EffectKind>,
}

/// A card in a player's own hand: its id (for committing) plus its visible attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandCard {
    /// Stable id used to commit this card.
    pub id: CardId,
    /// The card's revealed attributes (a hand is private to its owner).
    pub view: CardView,
}
