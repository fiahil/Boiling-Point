//! Regular card content: the dealt unit of the game.
//!
//! A `Card` is pure data (colour, volatility, points, and an optional effect
//! tag). It is a distinct content kind from [`super::modifier`] behaviour and
//! [`super::effect`] behaviour — these are never merged into one union (the loop
//! depends only on these stable shapes, never on a specific named card).

use boiling_point_protocol::vocab::{Color, EffectKind};

/// A card archetype as defined in content config: its attributes plus how many
/// physical copies exist in the deck and whether it is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardDef {
    /// The card's colour.
    pub color: Color,
    /// Explosion risk contributed (1–3).
    pub volatility: u8,
    /// Point value for scoring (0–3).
    pub points: u8,
    /// The special effect this card carries, if any.
    pub effect: Option<EffectKind>,
    /// Number of physical copies of this archetype in the deck.
    pub copies: u16,
}

impl CardDef {
    /// Whether this card carries a special effect.
    pub fn is_effect(&self) -> bool {
        self.effect.is_some()
    }

    /// Whether this is a colourless wild card.
    pub fn is_wild(&self) -> bool {
        self.color == Color::Wild
    }
}
