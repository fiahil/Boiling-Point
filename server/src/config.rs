//! Content/balance configuration: the file-driven schema, fail-fast validation,
//! and registry assembly.
//!
//! All tunable counts and ratios live here, not in the loop. The whole config is
//! validated at startup and the server refuses to run on an invalid config — a
//! typo fails loudly at boot, never mid-game.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use boiling_point_protocol::vocab::{ModifierKind, SpellKind};

use crate::content::card::{IngredientDef, PantrySlot};
use crate::content::registry::ContentRegistry;
use crate::content::spell::{SpellDef, SpellValues};

/// Players per table — fixed at four.
pub const PLAYERS: u16 = 4;
/// Ingredient hand floor — topped up to this at the start of every wave.
pub const INGREDIENT_HAND: u16 = 3;
/// Rounds per game.
pub const ROUND_COUNT: u8 = 5;
/// Modifiers drawn over a game (one each at the start of rounds 2–5).
pub const MODIFIER_DRAWS: u32 = 4;
/// Maximum ingredient volatility.
pub const MAX_VOLATILITY: u8 = 7;
/// Maximum ingredient points.
pub const MAX_POINTS: u8 = 3;

/// Top-level content configuration, deserialised from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentConfig {
    /// Pantry size and the colour-anchor band.
    pub pantry: PantryConfig,
    /// Ingredient archetypes (`[[ingredient]]` tables).
    #[serde(default)]
    pub ingredient: Vec<IngredientConfig>,
    /// Grimoire size and the round-start spell draw.
    pub grimoire: GrimoireConfig,
    /// Spell pool entries (`[[spell]]` tables).
    #[serde(default)]
    pub spell: Vec<SpellConfig>,
    /// Tunable spell magnitudes.
    pub spell_values: SpellValues,
    /// Modifier pool entries (`[[modifier]]` tables).
    #[serde(default)]
    pub modifier: Vec<ModifierConfig>,
    /// Preset emote palette (`[[emote]]` tables).
    #[serde(default)]
    pub emote: Vec<EmoteConfig>,
    /// Wave timing budgets.
    pub timing: TimingConfig,
    /// Boiling-point range.
    pub boiling_point: BoilingPointConfig,
}

/// Pantry-wide settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PantryConfig {
    /// Declared pantry size per player (enabled copies must sum to this).
    pub size: u16,
    /// Minimum share of the pantry in Own-colour slots (the colour anchor).
    pub own_min: f64,
    /// Maximum share of the pantry in Own-colour slots.
    pub own_max: f64,
}

/// An ingredient archetype.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngredientConfig {
    /// How the colour is assigned per seat (Own / OffColor / Wild).
    pub slot: PantrySlot,
    /// Explosion risk (0–7).
    pub volatility: u8,
    /// Point value as a colored Vote (0–3).
    pub points: u8,
    /// Number of physical copies in each pantry.
    pub copies: u16,
    /// Whether this archetype is enabled (excluded from the pantry if false).
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Grimoire-wide settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrimoireConfig {
    /// Declared grimoire size per player (enabled copies must sum to this).
    pub size: u16,
    /// Spells drawn at every round start (hoarded; no in-round replenish except Forage).
    pub spells_per_round: u8,
}

/// A spell pool entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellConfig {
    /// Which of the fifteen spells.
    pub kind: SpellKind,
    /// Copies in each grimoire.
    pub copies: u16,
    /// Whether this spell is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A modifier pool entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifierConfig {
    /// The modifier kind.
    pub kind: ModifierKind,
    /// Copies in the draw pool (its weight).
    pub copies: u16,
    /// Whether this modifier is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A preset emote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmoteConfig {
    /// Wire id for the emote.
    pub id: u16,
    /// Human-readable name (for clients).
    pub name: String,
    /// Whether this emote is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Wave timer budgets in milliseconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    /// Budget for the first wave of a round.
    pub wave1_ms: u32,
    /// Budget for subsequent waves.
    pub wave_ms: u32,
}

/// The (inclusive) hidden boiling-point range before modifiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoilingPointConfig {
    /// Lowest possible base boiling point.
    pub min: u8,
    /// Highest possible base boiling point.
    pub max: u8,
}

fn default_true() -> bool {
    true
}

/// A configuration that failed validation, with a specific, actionable reason.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ConfigError {
    /// Enabled ingredient copies do not sum to the declared pantry size.
    #[error("enabled ingredient copies sum to {got} but pantry.size is {expected}")]
    PantryCountMismatch {
        /// Declared pantry size.
        expected: u16,
        /// Actual sum of enabled copies.
        got: u16,
    },
    /// Own-colour share is outside the configured anchor band.
    #[error("own-colour ratio {ratio:.3} outside [{min:.3}, {max:.3}]")]
    OwnRatioOutOfBounds {
        /// Observed ratio.
        ratio: f64,
        /// Configured minimum.
        min: f64,
        /// Configured maximum.
        max: f64,
    },
    /// An ingredient's volatility or points are out of range.
    #[error(
        "ingredient attributes out of range: volatility {volatility} (max {MAX_VOLATILITY}) / points {points} (max {MAX_POINTS})"
    )]
    IngredientOutOfRange {
        /// The offending volatility.
        volatility: u8,
        /// The offending points.
        points: u8,
    },
    /// Enabled spell copies do not sum to the declared grimoire size.
    #[error("enabled spell copies sum to {got} but grimoire.size is {expected}")]
    GrimoireCountMismatch {
        /// Declared grimoire size.
        expected: u16,
        /// Actual sum of enabled copies.
        got: u16,
    },
    /// A rules-required spell is absent from the config entirely.
    #[error("spell {0:?} is required by the rules but absent from config")]
    MissingSpell(SpellKind),
    /// The round-start spell draw is zero (no spell economy at all).
    #[error("grimoire.spells_per_round must be at least 1")]
    NoSpellDraw,
    /// Too few enabled modifiers to draw one per applicable round.
    #[error("modifier pool has {got} enabled copies but needs at least {need}")]
    ModifierPoolTooSmall {
        /// Enabled modifier copies.
        got: u32,
        /// Minimum required.
        need: u32,
    },
    /// The pantry is too small to cover the opening ingredient deal.
    #[error("pantry size {size} cannot cover the ingredient hand of {need}")]
    PantryTooSmallForDeal {
        /// Declared pantry size.
        size: u16,
        /// Ingredients needed for the opening top-up.
        need: u16,
    },
    /// The boiling-point range is empty or inverted.
    #[error("boiling point range [{min}, {max}] is invalid")]
    BadBoilingPointRange {
        /// Configured minimum.
        min: u8,
        /// Configured maximum.
        max: u8,
    },
    /// The emote palette has no enabled emotes (no comms channel).
    #[error("the emote palette has no enabled emotes")]
    EmptyEmotePalette,
    /// Two emotes share an id.
    #[error("duplicate emote id {0} in the palette")]
    DuplicateEmoteId(u16),
    /// The TOML failed to parse.
    #[error("config parse error: {0}")]
    Parse(String),
}

/// The checked-in default content config (the same TOML the server binary
/// embeds), exposed so harnesses boot against exactly the shipped balance.
pub const DEFAULT_CONTENT_TOML: &str = include_str!("../content.toml");

impl ContentConfig {
    /// Parse a config from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, ConfigError> {
        toml::from_str(s).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// The checked-in default content config, parsed (panics only if the
    /// embedded TOML is invalid, which the server's own tests gate).
    pub fn default_content() -> Self {
        ContentConfig::from_toml(DEFAULT_CONTENT_TOML).expect("embedded content.toml is valid")
    }

    /// Enabled ingredient archetypes only.
    fn enabled_ingredients(&self) -> impl Iterator<Item = &IngredientConfig> {
        self.ingredient.iter().filter(|c| c.enabled)
    }

    /// Enabled spell entries only.
    fn enabled_spells(&self) -> impl Iterator<Item = &SpellConfig> {
        self.spell.iter().filter(|s| s.enabled)
    }

    /// Validate the whole config, returning the first specific failure. This is
    /// the fail-fast gate run before the server binds a port.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Pantry counts sum to the declared size.
        let total: u16 = self.enabled_ingredients().map(|c| c.copies).sum();
        if total != self.pantry.size {
            return Err(ConfigError::PantryCountMismatch {
                expected: self.pantry.size,
                got: total,
            });
        }

        // Pantry large enough for the opening top-up (reshuffle covers later exhaustion).
        if self.pantry.size < INGREDIENT_HAND {
            return Err(ConfigError::PantryTooSmallForDeal {
                size: self.pantry.size,
                need: INGREDIENT_HAND,
            });
        }

        // Attribute ranges: volatility 0-7, points 0-3.
        for c in self.enabled_ingredients() {
            if c.volatility > MAX_VOLATILITY || c.points > MAX_POINTS {
                return Err(ConfigError::IngredientOutOfRange {
                    volatility: c.volatility,
                    points: c.points,
                });
            }
        }

        // The colour anchor: Own share within the band (~75%).
        let own_copies: u16 = self
            .enabled_ingredients()
            .filter(|c| c.slot == PantrySlot::Own)
            .map(|c| c.copies)
            .sum();
        let own_ratio = own_copies as f64 / total.max(1) as f64;
        if own_ratio < self.pantry.own_min || own_ratio > self.pantry.own_max {
            return Err(ConfigError::OwnRatioOutOfBounds {
                ratio: own_ratio,
                min: self.pantry.own_min,
                max: self.pantry.own_max,
            });
        }

        // Grimoire counts sum to the declared size.
        let spell_total: u16 = self.enabled_spells().map(|s| s.copies).sum();
        if spell_total != self.grimoire.size {
            return Err(ConfigError::GrimoireCountMismatch {
                expected: self.grimoire.size,
                got: spell_total,
            });
        }

        // Every rules-required spell is present in config (enabled or explicitly disabled).
        for kind in SpellKind::ALL {
            let present = self.spell.iter().any(|s| s.kind == kind);
            if !present {
                return Err(ConfigError::MissingSpell(kind));
            }
        }

        // The spell economy exists at all.
        if self.grimoire.spells_per_round == 0 {
            return Err(ConfigError::NoSpellDraw);
        }

        // Modifier pool big enough to draw one per applicable round.
        let modifier_copies: u32 = self
            .modifier
            .iter()
            .filter(|m| m.enabled)
            .map(|m| m.copies as u32)
            .sum();
        if modifier_copies < MODIFIER_DRAWS {
            return Err(ConfigError::ModifierPoolTooSmall {
                got: modifier_copies,
                need: MODIFIER_DRAWS,
            });
        }

        // Boiling-point range sane.
        if self.boiling_point.min == 0 || self.boiling_point.min > self.boiling_point.max {
            return Err(ConfigError::BadBoilingPointRange {
                min: self.boiling_point.min,
                max: self.boiling_point.max,
            });
        }

        // Emote palette: at least one enabled emote, with unique ids.
        let mut seen_emotes = HashSet::new();
        for emote in self.emote.iter().filter(|e| e.enabled) {
            if !seen_emotes.insert(emote.id) {
                return Err(ConfigError::DuplicateEmoteId(emote.id));
            }
        }
        if seen_emotes.is_empty() {
            return Err(ConfigError::EmptyEmotePalette);
        }

        Ok(())
    }

    /// Build the [`ContentRegistry`] from this config. Validates first, so a bad
    /// config can never produce a registry.
    pub fn build_registry(&self) -> Result<ContentRegistry, ConfigError> {
        self.validate()?;

        let ingredients: Vec<IngredientDef> = self
            .enabled_ingredients()
            .map(|c| IngredientDef {
                slot: c.slot,
                volatility: c.volatility,
                points: c.points,
                copies: c.copies,
            })
            .collect();

        let spells: Vec<SpellDef> = self
            .enabled_spells()
            .map(|s| SpellDef {
                kind: s.kind,
                copies: s.copies,
            })
            .collect();

        let enabled_modifiers: Vec<(ModifierKind, u16)> = self
            .modifier
            .iter()
            .filter(|m| m.enabled)
            .map(|m| (m.kind, m.copies))
            .collect();

        Ok(ContentRegistry::new(
            ingredients,
            spells,
            self.spell_values,
            &enabled_modifiers,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The checked-in default config, used as a known-valid baseline.
    const DEFAULT: &str = include_str!("../content.toml");

    /// A valid config validates and builds a registry with all enabled content.
    #[test]
    fn default_config_validates_and_builds() {
        let cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        cfg.validate().expect("validate");
        let reg = cfg.build_registry().expect("build");
        assert_eq!(reg.pantry_size(), 30);
        assert_eq!(reg.grimoire_size(), 20);
        // All fifteen spells are enabled in the default grimoire.
        for kind in SpellKind::ALL {
            assert!(
                reg.spells().iter().any(|s| s.kind == kind),
                "missing spell {kind:?}"
            );
        }
        // Modifier pool has the full 20 copies.
        let pool: u32 = reg.modifier_pool().iter().map(|(_, c)| *c as u32).sum();
        assert_eq!(pool, 20);
    }

    /// A count that no longer sums to the declared pantry size aborts.
    #[test]
    fn pantry_count_mismatch_aborts() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        cfg.pantry.size += 1;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::PantryCountMismatch { .. })
        ));
    }

    /// Eroding the colour anchor aborts (the ~75% own-colour band is load-bearing).
    #[test]
    fn own_ratio_out_of_band_aborts() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        // Flip every Own archetype to OffColor: own ratio collapses to 0.
        for c in cfg.ingredient.iter_mut() {
            if c.slot == PantrySlot::Own {
                c.slot = PantrySlot::OffColor;
            }
        }
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::OwnRatioOutOfBounds { .. })
        ));
    }

    /// Volatility above 7 aborts.
    #[test]
    fn out_of_range_volatility_aborts() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        cfg.ingredient[0].volatility = 8;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::IngredientOutOfRange { .. })
        ));
    }

    /// A grimoire that loses a required spell kind aborts.
    #[test]
    fn missing_spell_kind_aborts() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        let removed = cfg
            .spell
            .iter()
            .position(|s| s.kind == SpellKind::Hex)
            .expect("hex present");
        let copies = cfg.spell[removed].copies;
        cfg.spell.remove(removed);
        cfg.grimoire.size -= copies;
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::MissingSpell(SpellKind::Hex))
        );
    }

    /// An empty modifier pool aborts (cannot draw one per round).
    #[test]
    fn empty_modifier_pool_aborts() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        for m in cfg.modifier.iter_mut() {
            m.enabled = false;
        }
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::ModifierPoolTooSmall { .. })
        ));
    }

    /// An emote palette with nothing enabled aborts (no comms channel).
    #[test]
    fn empty_emote_palette_aborts() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        for e in cfg.emote.iter_mut() {
            e.enabled = false;
        }
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::EmptyEmotePalette)
        ));
    }

    /// Disabling a content item excludes it from the registry without code changes.
    #[test]
    fn disabling_a_spell_toggles_it_off() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        let mut removed = 0u16;
        for s in cfg.spell.iter_mut() {
            if s.kind == SpellKind::Quench {
                s.enabled = false;
                removed += s.copies;
            }
        }
        cfg.grimoire.size -= removed; // keep counts consistent
        let reg = cfg.build_registry().expect("build");
        assert!(!reg.spells().iter().any(|s| s.kind == SpellKind::Quench));
        assert!(reg.spells().iter().any(|s| s.kind == SpellKind::Peek));
    }
}
