//! The server-side concrete cards: physical instances with stable ids and their
//! authoritative attributes. Distinct from the protocol's views, which are the
//! *revealed* projections sent to clients.
//!
//! Two card types (the boom2 split): an [`Ingredient`] goes into the cauldron
//! (colour · volatility · points); a [`Spell`] is an active effect that is never
//! in the pot and carries no points or volatility of its own.

use boiling_point_protocol::CardId;
use boiling_point_protocol::vocab::{Color, IngredientView, SpellKind};

/// A concrete ingredient instance in a pantry, a hand, or the pot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ingredient {
    /// Stable instance id (assigned when the pantries are built).
    pub id: CardId,
    /// The ingredient's printed colour (per-seat instantiation of its slot).
    pub color: Color,
    /// Explosion risk contributed (0–7).
    pub volatility: u8,
    /// Point value when played as a colored Vote (0–3).
    pub points: u8,
}

impl Ingredient {
    /// Project to the public, revealed view sent over the wire.
    pub fn view(&self) -> IngredientView {
        IngredientView {
            color: self.color,
            volatility: self.volatility,
            points: self.points,
        }
    }
}

/// A concrete spell instance in a grimoire or a hand. Consumed on cast (Instant)
/// or on fire / round end (Active).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spell {
    /// Stable instance id (assigned when the grimoires are built).
    pub id: CardId,
    /// Which of the fifteen spells this is.
    pub kind: SpellKind,
}
