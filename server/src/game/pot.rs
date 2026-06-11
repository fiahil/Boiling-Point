//! The cauldron pot: the ingredients played so far this round (with the wave
//! each landed in and how it was played), the cauldron-level volatility deltas
//! from spells, and the per-colour point adjustments from score spells. Holds
//! the authoritative running state a round accumulates across waves.

use std::collections::HashMap;

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::vocab::Color;

use super::card::Ingredient;

/// One ingredient in the pot, with how it was played.
#[derive(Debug, Clone, Copy)]
pub struct PotIngredient {
    /// Who played it.
    pub player: PlayerId,
    /// The underlying ingredient.
    pub ingredient: Ingredient,
    /// Whether it was played colorless (a go-neutral push): volatility counts,
    /// points do not, and it serves no colour.
    pub colorless: bool,
    /// The 1-based wave it landed in (detonator liability is fatal-wave-scoped).
    pub wave_number: u8,
    /// Whether an Expose has already revealed it (each card is exposed at most once).
    pub exposed: bool,
}

impl PotIngredient {
    /// The colour this card serves: its printed colour as a Vote, or `None` when
    /// played colorless / printed wild.
    pub fn effective_color(&self) -> Option<Color> {
        if self.colorless || self.ingredient.color == Color::Wild {
            None
        } else {
            Some(self.ingredient.color)
        }
    }

    /// The points this card contributes to its colour (zero unless a colored Vote).
    pub fn effective_points(&self) -> u8 {
        if self.effective_color().is_some() {
            self.ingredient.points
        } else {
            0
        }
    }

    /// The volatility this card contributes. Printed value for now — the seam
    /// where compounding (boom2-compounding) will adjust per-card volatility;
    /// the detonator sort and the explosion check both use this.
    pub fn effective_volatility(&self) -> u8 {
        self.ingredient.volatility
    }
}

/// The accumulating pot for a round.
#[derive(Debug, Default)]
pub struct Pot {
    /// Ingredients in play order.
    pub cards: Vec<PotIngredient>,
    /// Volatility the cauldron started with (e.g. from Residue).
    pub start_volatility: i32,
    /// Net cauldron-level volatility delta from spells (Dampen/Surge). Sits in
    /// the running total, never on any single card.
    pub spell_volatility_delta: i32,
    /// Signed per-colour point adjustments from score spells (Double Down +,
    /// Sour −), applied against pre-wave snapshots.
    pub color_adjust: HashMap<Color, i64>,
}

impl Pot {
    /// A fresh pot seeded with `start_volatility` (e.g. from Residue).
    pub fn new(start_volatility: i32) -> Self {
        Pot {
            start_volatility,
            ..Default::default()
        }
    }

    /// Total points attributed to a colour: its Votes' points plus any spell
    /// adjustment, floored at zero (Sour can never push a colour negative).
    pub fn color_points(&self, color: Color) -> u32 {
        let base: i64 = self
            .cards
            .iter()
            .filter(|p| p.effective_color() == Some(color))
            .map(|p| p.effective_points() as i64)
            .sum();
        let adjusted = base + self.color_adjust.get(&color).copied().unwrap_or(0);
        adjusted.max(0) as u32
    }

    /// Whether any Vote of `color` is present in the pot.
    pub fn color_present(&self, color: Color) -> bool {
        self.cards
            .iter()
            .any(|p| p.effective_color() == Some(color))
    }

    /// The pot value **P**: the sum of colored Vote points across the four
    /// player colours. Colorless and wild plays contribute nothing.
    pub fn vote_points(&self) -> u32 {
        Color::PLAYER_COLORS
            .into_iter()
            .map(|c| self.color_points(c))
            .sum()
    }

    /// The cauldron's running volatility total: start + every card's effective
    /// volatility + the spell delta, floored at zero. This is what the
    /// explosion check compares against the boiling point.
    pub fn total_volatility(&self) -> i32 {
        let cards: i32 = self
            .cards
            .iter()
            .map(|p| p.effective_volatility() as i32)
            .sum();
        (self.start_volatility + cards + self.spell_volatility_delta).max(0)
    }

    /// Number of ingredients in the pot.
    pub fn card_count(&self) -> u32 {
        self.cards.len() as u32
    }

    /// Remove the most recent ingredient `player` added (the Skim target):
    /// its points and volatility leave the pot. Returns the removed card.
    pub fn remove_last_of(&mut self, player: PlayerId) -> Option<PotIngredient> {
        let pos = self.cards.iter().rposition(|p| p.player == player)?;
        Some(self.cards.remove(pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::CardId;
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn entry(
        player: PlayerId,
        color: Color,
        vol: u8,
        pts: u8,
        colorless: bool,
        wave: u8,
    ) -> PotIngredient {
        PotIngredient {
            player,
            ingredient: Ingredient {
                id: CardId(0),
                color,
                volatility: vol,
                points: pts,
            },
            colorless,
            wave_number: wave,
            exposed: false,
        }
    }

    /// Points score only on colored Votes: a colorless play and a wild both add
    /// volatility but zero points.
    #[test]
    fn colorless_and_wild_score_zero() {
        let mut pot = Pot::new(0);
        pot.cards.push(entry(pid(1), Color::Ruby, 2, 3, false, 1)); // a Vote
        pot.cards.push(entry(pid(2), Color::Ruby, 4, 3, true, 1)); // colorless push
        pot.cards.push(entry(pid(3), Color::Wild, 7, 3, false, 1)); // a wild
        assert_eq!(pot.color_points(Color::Ruby), 3);
        assert_eq!(pot.vote_points(), 3);
        assert_eq!(pot.total_volatility(), 13); // all volatility still counts
    }

    /// Spell deltas sit in the running total, not on any card.
    #[test]
    fn spell_delta_adjusts_total_only() {
        let mut pot = Pot::new(0);
        pot.cards.push(entry(pid(1), Color::Ruby, 3, 1, false, 1));
        pot.spell_volatility_delta = -3;
        assert_eq!(pot.total_volatility(), 0);
        assert_eq!(pot.cards[0].effective_volatility(), 3); // the card is untouched
        pot.spell_volatility_delta = 5;
        assert_eq!(pot.total_volatility(), 8);
    }

    /// Colour adjustments floor at zero (Sour cannot push a colour negative).
    #[test]
    fn color_adjust_floors_at_zero() {
        let mut pot = Pot::new(0);
        pot.cards.push(entry(pid(1), Color::Ruby, 1, 2, false, 1));
        pot.color_adjust.insert(Color::Ruby, -5);
        assert_eq!(pot.color_points(Color::Ruby), 0);
    }

    /// Skim removes the caster's most recent card, with its points and volatility.
    #[test]
    fn remove_last_of_takes_the_latest() {
        let mut pot = Pot::new(0);
        pot.cards.push(entry(pid(1), Color::Ruby, 1, 1, false, 1));
        pot.cards.push(entry(pid(1), Color::Ruby, 5, 2, false, 2));
        pot.cards
            .push(entry(pid(2), Color::Emerald, 2, 1, false, 2));
        let removed = pot.remove_last_of(pid(1)).expect("a card");
        assert_eq!(removed.ingredient.volatility, 5);
        assert_eq!(pot.total_volatility(), 3);
        assert_eq!(pot.color_points(Color::Ruby), 1);
    }
}
