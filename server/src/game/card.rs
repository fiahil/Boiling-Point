//! The server-side concrete card: a physical instance with a stable id and its
//! authoritative attributes. Distinct from the protocol's `CardView`, which is
//! the *revealed* projection sent to clients.

use boiling_point_protocol::CardId;
use boiling_point_protocol::vocab::{CardView, Color, EffectKind};

/// A concrete card instance in the deck, a hand, or the pot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    /// Stable instance id (assigned when the deck is built).
    pub id: CardId,
    /// The card's colour.
    pub color: Color,
    /// Explosion risk contributed (1–3).
    pub volatility: u8,
    /// Point value for scoring (0–3).
    pub points: u8,
    /// The card's special effect, if any.
    pub effect: Option<EffectKind>,
}

impl Card {
    /// Project to the public, revealed view sent over the wire.
    pub fn view(&self) -> CardView {
        CardView {
            color: self.color,
            volatility: self.volatility,
            points: self.points,
            effect: self.effect,
        }
    }
}
