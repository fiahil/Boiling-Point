//! Per-player domain: identity/colour and the private hand. Hands are owned by
//! the game (server-side) and never serialised onto a broadcast.

use boiling_point_protocol::vocab::{Color, HandIngredient, HandSpell};
use boiling_point_protocol::{CardId, PlayerId};

use super::card::{Ingredient, Spell};

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

/// A player's private hand: up to the ingredient floor of ingredients, plus the
/// hoarded grimoire spells (drawn at round start, carried over between rounds).
#[derive(Debug, Default, Clone)]
pub struct Hand {
    ingredients: Vec<Ingredient>,
    spells: Vec<Spell>,
}

impl Hand {
    /// An empty hand.
    pub fn new() -> Self {
        Self::default()
    }

    /// The ingredients currently held.
    pub fn ingredients(&self) -> &[Ingredient] {
        &self.ingredients
    }

    /// The spells currently held.
    pub fn spells(&self) -> &[Spell] {
        &self.spells
    }

    /// Whether the hand holds no ingredients (spells alone cannot keep a player
    /// acting — the wave choice is ingredient-or-pass).
    pub fn no_ingredients(&self) -> bool {
        self.ingredients.is_empty()
    }

    /// Whether the hand contains an ingredient with this id.
    pub fn contains_ingredient(&self, id: CardId) -> bool {
        self.ingredients.iter().any(|c| c.id == id)
    }

    /// Whether the hand contains a spell with this id.
    pub fn contains_spell(&self, id: CardId) -> bool {
        self.spells.iter().any(|s| s.id == id)
    }

    /// Remove and return the ingredient with this id, if present.
    pub fn take_ingredient(&mut self, id: CardId) -> Option<Ingredient> {
        let pos = self.ingredients.iter().position(|c| c.id == id)?;
        Some(self.ingredients.remove(pos))
    }

    /// Remove and return the spell with this id, if present.
    pub fn take_spell(&mut self, id: CardId) -> Option<Spell> {
        let pos = self.spells.iter().position(|s| s.id == id)?;
        Some(self.spells.remove(pos))
    }

    /// Add ingredients to the hand (a top-up).
    pub fn add_ingredients(&mut self, cards: impl IntoIterator<Item = Ingredient>) {
        self.ingredients.extend(cards);
    }

    /// Add spells to the hand (the round-start draw, or a Forage).
    pub fn add_spells(&mut self, spells: impl IntoIterator<Item = Spell>) {
        self.spells.extend(spells);
    }

    /// The ingredient hand projected to the wire view (sent only to its owner).
    pub fn ingredient_views(&self) -> Vec<HandIngredient> {
        self.ingredients
            .iter()
            .map(|c| HandIngredient {
                id: c.id,
                view: c.view(),
            })
            .collect()
    }

    /// The spell hand projected to the wire view (sent only to its owner).
    pub fn spell_views(&self) -> Vec<HandSpell> {
        self.spells
            .iter()
            .map(|s| HandSpell {
                id: s.id,
                kind: s.kind,
            })
            .collect()
    }
}
