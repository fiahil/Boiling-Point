//! The content registry: the single lookup the rest of the engine consults,
//! built once at startup from validated config with disabled items filtered
//! out. This is the only seam between churning content and the stable loop
//! (Registry pattern).

use std::collections::HashMap;

use boiling_point_protocol::vocab::{ModifierKind, PantryBucket};

use crate::config::ApothecaryConfig;

use super::card::{BucketCard, IngredientDef};
use super::modifier::{Modifier, behavior_for as modifier_behavior};
use super::spell::{SpellDef, SpellValues};

/// One entry in the weighted modifier draw pool.
pub struct ModifierPoolEntry {
    /// The modifier's behaviour.
    pub behavior: Box<dyn Modifier>,
    /// How many copies sit in the pool (its draw weight).
    pub copies: u16,
}

/// Built, validated content ready for play. Holds only enabled items.
pub struct ContentRegistry {
    /// Enabled ingredient archetypes, expanded into instances by the pantry builder.
    ingredients: Vec<IngredientDef>,
    /// Enabled spell archetypes, expanded into instances by the grimoire builder.
    spells: Vec<SpellDef>,
    /// The tunable spell magnitudes.
    spell_values: SpellValues,
    /// Enabled modifier pool (kind → entry) for weighted draws.
    modifiers: HashMap<ModifierKind, ModifierPoolEntry>,
    /// The Apothecary realizer caps (`boom2-apothecary`).
    apothecary: ApothecaryConfig,
    /// Per-bucket pantry card families, in [`PantryBucket::ALL`] order (a
    /// stable base order keeps the realizer's seeded rolls reproducible).
    pantry_families: Vec<(PantryBucket, Vec<BucketCard>)>,
}

impl ContentRegistry {
    /// Assemble a registry from already-validated parts. `enabled_*` inputs must
    /// already exclude disabled items.
    pub fn new(
        ingredients: Vec<IngredientDef>,
        spells: Vec<SpellDef>,
        spell_values: SpellValues,
        enabled_modifiers: &[(ModifierKind, u16)],
        apothecary: ApothecaryConfig,
        pantry_families: Vec<(PantryBucket, Vec<BucketCard>)>,
    ) -> Self {
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
            ingredients,
            spells,
            spell_values,
            modifiers,
            apothecary,
            pantry_families,
        }
    }

    /// The enabled ingredient archetypes that make up each player's pantry.
    pub fn ingredients(&self) -> &[IngredientDef] {
        &self.ingredients
    }

    /// The enabled spell archetypes that make up each player's grimoire.
    pub fn spells(&self) -> &[SpellDef] {
        &self.spells
    }

    /// The tunable spell magnitudes.
    pub fn spell_values(&self) -> &SpellValues {
        &self.spell_values
    }

    /// The behaviour for a modifier kind, if that modifier is enabled.
    pub fn modifier(&self, kind: ModifierKind) -> Option<&dyn Modifier> {
        self.modifiers.get(&kind).map(|e| e.behavior.as_ref())
    }

    /// The weighted modifier draw pool as (kind, copies) pairs, in a deterministic
    /// order.
    ///
    /// The entries live in a `HashMap`, whose iteration order is randomised per
    /// instance; callers shuffle this pile with a *seeded* RNG, so an unstable base
    /// order would make the same seed draw different modifiers across registry
    /// instances (and processes). Sorting by kind first keeps the seeded shuffle —
    /// and thus any `(seed, config)` run — reproducible, without changing the
    /// drawn multiset or its distribution.
    pub fn modifier_pool(&self) -> Vec<(ModifierKind, u16)> {
        let mut pool: Vec<(ModifierKind, u16)> =
            self.modifiers.iter().map(|(k, e)| (*k, e.copies)).collect();
        pool.sort_by_key(|(kind, _)| format!("{kind:?}"));
        pool
    }

    /// Total physical ingredients in one pantry (sum of enabled archetype copies).
    pub fn pantry_size(&self) -> u32 {
        self.ingredients.iter().map(|c| c.copies as u32).sum()
    }

    /// Total physical spells in one grimoire (sum of enabled archetype copies).
    pub fn grimoire_size(&self) -> u32 {
        self.spells.iter().map(|s| s.copies as u32).sum()
    }

    /// The Apothecary realizer caps.
    pub fn apothecary(&self) -> &ApothecaryConfig {
        &self.apothecary
    }

    /// A pantry bucket's eligible card family.
    pub fn bucket_family(&self, bucket: PantryBucket) -> &[BucketCard] {
        self.pantry_families
            .iter()
            .find(|(b, _)| *b == bucket)
            .map(|(_, cards)| cards.as_slice())
            .unwrap_or(&[])
    }
}
