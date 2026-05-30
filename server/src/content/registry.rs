//! The content registry: the single id→definition lookup the rest of the engine
//! consults, built once at startup from validated config with disabled items
//! filtered out. This is the only seam between churning content and the stable
//! loop (Registry pattern).

use std::collections::HashMap;

use boiling_point_protocol::vocab::{EffectKind, ModifierKind};

use super::card::CardDef;
use super::effect::{behavior_for as effect_behavior, Effect};
use super::modifier::{behavior_for as modifier_behavior, Modifier};

/// One entry in the weighted modifier draw pool.
pub struct ModifierPoolEntry {
    /// The modifier's behaviour.
    pub behavior: Box<dyn Modifier>,
    /// How many copies sit in the pool (its draw weight).
    pub copies: u16,
}

/// Built, validated content ready for play. Holds only enabled items.
pub struct ContentRegistry {
    /// Enabled card archetypes, expanded into instances by the deck builder.
    cards: Vec<CardDef>,
    /// Enabled effect behaviours, keyed by kind.
    effects: HashMap<EffectKind, Box<dyn Effect>>,
    /// Enabled modifier pool (kind → entry) for weighted draws.
    modifiers: HashMap<ModifierKind, ModifierPoolEntry>,
}

impl ContentRegistry {
    /// Assemble a registry from already-validated parts. `enabled_*` inputs must
    /// already exclude disabled items.
    pub fn new(
        cards: Vec<CardDef>,
        enabled_effects: &[EffectKind],
        enabled_modifiers: &[(ModifierKind, u16)],
    ) -> Self {
        let mut effects: HashMap<EffectKind, Box<dyn Effect>> = HashMap::new();
        for &kind in enabled_effects {
            effects.entry(kind).or_insert_with(|| effect_behavior(kind));
        }
        let mut modifiers: HashMap<ModifierKind, ModifierPoolEntry> = HashMap::new();
        for &(kind, copies) in enabled_modifiers {
            modifiers
                .entry(kind)
                .and_modify(|e| e.copies += copies)
                .or_insert_with(|| ModifierPoolEntry {
                    behavior: modifier_behavior(kind),
                    copies,
                });
        }
        ContentRegistry {
            cards,
            effects,
            modifiers,
        }
    }

    /// The enabled card archetypes that make up the deck.
    pub fn cards(&self) -> &[CardDef] {
        &self.cards
    }

    /// The behaviour for an effect kind, if that effect is enabled.
    pub fn effect(&self, kind: EffectKind) -> Option<&dyn Effect> {
        self.effects.get(&kind).map(|b| b.as_ref())
    }

    /// The behaviour for a modifier kind, if that modifier is enabled.
    pub fn modifier(&self, kind: ModifierKind) -> Option<&dyn Modifier> {
        self.modifiers.get(&kind).map(|e| e.behavior.as_ref())
    }

    /// The weighted modifier draw pool as (kind, copies) pairs.
    pub fn modifier_pool(&self) -> Vec<(ModifierKind, u16)> {
        self.modifiers.iter().map(|(k, e)| (*k, e.copies)).collect()
    }

    /// Total physical cards in the deck (sum of enabled archetype copies).
    pub fn deck_size(&self) -> u32 {
        self.cards.iter().map(|c| c.copies as u32).sum()
    }
}
