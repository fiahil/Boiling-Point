//! Per-player domain: identity/colour and the private hand. Hands are owned by
//! the game (server-side) and never serialised onto a broadcast.

use boiling_point_protocol::vocab::{Color, HandCard};
use boiling_point_protocol::{CardId, PlayerId};

use super::card::Card;

/// A seated player: stable id, assigned colour, and display name.
#[derive(Debug, Clone)]
pub struct Player {
    /// Stable id.
    pub id: PlayerId,
    /// Assigned player colour (Ruby/Sapphire/Emerald/Amethyst).
    pub color: Color,
    /// Chosen display name.
    pub display_name: String,
}

/// A player's private hand of cards.
#[derive(Debug, Default, Clone)]
pub struct Hand {
    cards: Vec<Card>,
}

impl Hand {
    /// An empty hand.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of cards held.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Whether the hand is empty.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Whether the hand contains a card with this id.
    pub fn contains(&self, id: CardId) -> bool {
        self.cards.iter().any(|c| c.id == id)
    }

    /// Remove and return the card with this id, if present.
    pub fn take(&mut self, id: CardId) -> Option<Card> {
        let pos = self.cards.iter().position(|c| c.id == id)?;
        Some(self.cards.remove(pos))
    }

    /// Add cards to the hand (e.g. a refill, or a recalled card).
    pub fn add(&mut self, cards: impl IntoIterator<Item = Card>) {
        self.cards.extend(cards);
    }

    /// The hand projected to the wire view (sent only to its owner).
    pub fn views(&self) -> Vec<HandCard> {
        self.cards
            .iter()
            .map(|c| HandCard {
                id: c.id,
                view: c.view(),
            })
            .collect()
    }
}
