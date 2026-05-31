//! The cauldron pot: the cards played so far this round, their effective colours
//! and points (after effects), the running volatility, and per-colour point
//! bonuses (from Double Down). Holds the authoritative running state a round
//! accumulates across waves.

use std::collections::HashMap;

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::vocab::Color;

use super::card::Card;

/// One card in the pot, with its *effective* colour and points (which effects
/// such as Copycat may change from the card's printed values).
#[derive(Debug, Clone, Copy)]
pub struct PotCard {
    /// Who played the card.
    pub player: PlayerId,
    /// The underlying card.
    pub card: Card,
    /// Effective colour (Copycat may override the printed colour).
    pub color: Color,
    /// Effective base points contributed by this card.
    pub points: u8,
}

/// The accumulating pot for a round.
#[derive(Debug, Default)]
pub struct Pot {
    /// Cards in play order.
    pub cards: Vec<PotCard>,
    /// Running volatility total (after Dampen/Volatile Surge).
    pub volatility: i32,
    /// Additive per-colour point bonuses contributed by Double Down.
    pub color_bonus: HashMap<Color, u32>,
}

impl Pot {
    /// A fresh pot seeded with `start_volatility` (e.g. from Residue).
    pub fn new(start_volatility: i32) -> Self {
        Pot {
            cards: Vec::new(),
            volatility: start_volatility,
            color_bonus: HashMap::new(),
        }
    }

    /// Total points attributed to a colour: its cards' points plus any Double
    /// Down bonus. (Wild contributes to pot value but is asked for separately.)
    pub fn color_points(&self, color: Color) -> u32 {
        let base: u32 = self
            .cards
            .iter()
            .filter(|p| p.color == color)
            .map(|p| p.points as u32)
            .sum();
        base + self.color_bonus.get(&color).copied().unwrap_or(0)
    }

    /// Whether any card of `color` is present in the pot.
    pub fn color_present(&self, color: Color) -> bool {
        self.cards.iter().any(|p| p.color == color)
    }

    /// The dominant player colour by points (ties broken by colour order). Used
    /// for the Copycat pre-wave snapshot; `None` if no player colour is present.
    pub fn dominant_player_color(&self) -> Option<Color> {
        Color::PLAYER_COLORS
            .into_iter()
            .filter(|c| self.color_present(*c))
            .max_by_key(|c| self.color_points(*c))
    }

    /// The pot's total base point value: all cards' points plus all colour bonuses.
    pub fn base_points(&self) -> u32 {
        let cards: u32 = self.cards.iter().map(|p| p.points as u32).sum();
        let bonuses: u32 = self.color_bonus.values().sum();
        cards + bonuses
    }

    /// Number of cards in the pot.
    pub fn card_count(&self) -> u32 {
        self.cards.len() as u32
    }
}
