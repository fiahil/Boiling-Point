//! Content/balance configuration: the file-driven schema, fail-fast validation,
//! and registry assembly.
//!
//! All tunable counts and ratios live here, not in the loop. The whole config is
//! validated at startup and the server refuses to run on an invalid config
//! (Constraint #3) — a typo fails loudly at boot, never mid-game.

use std::collections::HashSet;

use serde::Deserialize;

use boiling_point_protocol::vocab::{Color, EffectKind, ModifierKind};

use crate::content::card::CardDef;
use crate::content::effect::ALL_EFFECT_KINDS;
use crate::content::registry::ContentRegistry;

/// Players per table — fixed at four in v1.
pub const PLAYERS: u16 = 4;
/// Hand refill floor dealt at the start of each round.
pub const HAND_SIZE: u16 = 5;
/// Rounds per game.
pub const ROUND_COUNT: u8 = 5;
/// Modifiers drawn over a game (one each at the start of rounds 2–5).
pub const MODIFIER_DRAWS: u32 = 4;

/// Top-level content configuration, deserialised from TOML.
#[derive(Debug, Clone, Deserialize)]
pub struct ContentConfig {
    /// Deck size and ratio bounds.
    pub deck: DeckConfig,
    /// Card archetypes (`[[card]]` tables).
    #[serde(default)]
    pub card: Vec<CardConfig>,
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

/// Deck-wide settings.
#[derive(Debug, Clone, Deserialize)]
pub struct DeckConfig {
    /// Declared total number of physical cards (enabled copies must sum to this).
    pub size: u16,
    /// Allowed ratio bands for effect and wild cards.
    pub ratio_bounds: RatioBounds,
}

/// Allowed proportions of effect and wild cards in the deck.
#[derive(Debug, Clone, Deserialize)]
pub struct RatioBounds {
    /// Minimum share of the deck that must be effect cards.
    pub effect_min: f64,
    /// Maximum share of the deck that may be effect cards.
    pub effect_max: f64,
    /// Minimum share of the deck that must be wild cards.
    pub wild_min: f64,
    /// Maximum share of the deck that may be wild cards.
    pub wild_max: f64,
}

/// A card archetype.
#[derive(Debug, Clone, Deserialize)]
pub struct CardConfig {
    /// The card's colour.
    pub color: Color,
    /// Explosion risk (1–3).
    pub volatility: u8,
    /// Point value (0–3).
    pub points: u8,
    /// The special effect this card carries, if any.
    #[serde(default)]
    pub effect: Option<EffectKind>,
    /// Number of physical copies.
    pub copies: u16,
    /// Whether this archetype is enabled (excluded from the deck if false).
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A modifier pool entry.
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
pub struct TimingConfig {
    /// Budget for the first wave of a round.
    pub wave1_ms: u32,
    /// Budget for subsequent waves.
    pub wave_ms: u32,
}

/// The (inclusive) hidden boiling-point range before modifiers.
#[derive(Debug, Clone, Deserialize)]
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
    /// Enabled card copies do not sum to the declared deck size.
    #[error("enabled card copies sum to {got} but deck.size is {expected}")]
    CountMismatch {
        /// Declared deck size.
        expected: u16,
        /// Actual sum of enabled copies.
        got: u16,
    },
    /// Effect-card ratio is outside the configured band.
    #[error("effect ratio {ratio:.3} outside [{min:.3}, {max:.3}]")]
    EffectRatioOutOfBounds {
        /// Observed ratio.
        ratio: f64,
        /// Configured minimum.
        min: f64,
        /// Configured maximum.
        max: f64,
    },
    /// Wild-card ratio is outside the configured band.
    #[error("wild ratio {ratio:.3} outside [{min:.3}, {max:.3}]")]
    WildRatioOutOfBounds {
        /// Observed ratio.
        ratio: f64,
        /// Configured minimum.
        min: f64,
        /// Configured maximum.
        max: f64,
    },
    /// A rules-required effect is absent from the config entirely.
    #[error("effect {0:?} is required by the rules but absent from config")]
    MissingEffect(EffectKind),
    /// Too few enabled modifiers to draw one per applicable round.
    #[error("modifier pool has {got} enabled copies but needs at least {need}")]
    ModifierPoolTooSmall {
        /// Enabled modifier copies.
        got: u32,
        /// Minimum required.
        need: u32,
    },
    /// The deck is too small to cover the initial deal.
    #[error("deck size {size} cannot cover the initial deal of {need}")]
    DeckTooSmallForDeal {
        /// Declared deck size.
        size: u16,
        /// Cards needed for the opening deal.
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

impl ContentConfig {
    /// Parse a config from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, ConfigError> {
        toml::from_str(s).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Enabled card archetypes only.
    fn enabled_cards(&self) -> impl Iterator<Item = &CardConfig> {
        self.card.iter().filter(|c| c.enabled)
    }

    /// Sum of enabled card copies (the realised deck size).
    fn enabled_copies(&self) -> u16 {
        self.enabled_cards().map(|c| c.copies).sum()
    }

    /// Validate the whole config, returning the first specific failure. This is
    /// the fail-fast gate run before the server binds a port.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Counts sum to the declared deck size.
        let total = self.enabled_copies();
        if total != self.deck.size {
            return Err(ConfigError::CountMismatch {
                expected: self.deck.size,
                got: total,
            });
        }

        // Deck large enough for the opening deal (reshuffle covers later exhaustion).
        let need = PLAYERS * HAND_SIZE;
        if self.deck.size < need {
            return Err(ConfigError::DeckTooSmallForDeal {
                size: self.deck.size,
                need,
            });
        }

        // Effect ratio within bounds.
        let effect_copies: u16 = self
            .enabled_cards()
            .filter(|c| c.effect.is_some())
            .map(|c| c.copies)
            .sum();
        let effect_ratio = effect_copies as f64 / total.max(1) as f64;
        let b = &self.deck.ratio_bounds;
        if effect_ratio < b.effect_min || effect_ratio > b.effect_max {
            return Err(ConfigError::EffectRatioOutOfBounds {
                ratio: effect_ratio,
                min: b.effect_min,
                max: b.effect_max,
            });
        }

        // Wild ratio within bounds.
        let wild_copies: u16 = self
            .enabled_cards()
            .filter(|c| c.color == Color::Wild)
            .map(|c| c.copies)
            .sum();
        let wild_ratio = wild_copies as f64 / total.max(1) as f64;
        if wild_ratio < b.wild_min || wild_ratio > b.wild_max {
            return Err(ConfigError::WildRatioOutOfBounds {
                ratio: wild_ratio,
                min: b.wild_min,
                max: b.wild_max,
            });
        }

        // Every rules-required effect is present in config (enabled or explicitly disabled).
        for kind in ALL_EFFECT_KINDS {
            let present = self.card.iter().any(|c| c.effect == Some(kind));
            if !present {
                return Err(ConfigError::MissingEffect(kind));
            }
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

        let cards: Vec<CardDef> = self
            .enabled_cards()
            .map(|c| CardDef {
                color: c.color,
                volatility: c.volatility,
                points: c.points,
                effect: c.effect,
                copies: c.copies,
            })
            .collect();

        // Enabled effect kinds = those carried by at least one enabled card.
        let mut enabled_effects: Vec<EffectKind> = cards.iter().filter_map(|c| c.effect).collect();
        enabled_effects.sort_by_key(|k| format!("{k:?}"));
        enabled_effects.dedup();

        let enabled_modifiers: Vec<(ModifierKind, u16)> = self
            .modifier
            .iter()
            .filter(|m| m.enabled)
            .map(|m| (m.kind, m.copies))
            .collect();

        Ok(ContentRegistry::new(
            cards,
            &enabled_effects,
            &enabled_modifiers,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::vocab::EffectKind;

    /// The checked-in default config, used as a known-valid baseline.
    const DEFAULT: &str = include_str!("../content.toml");

    /// A valid config validates and builds a registry with all enabled content.
    #[test]
    fn default_config_validates_and_builds() {
        let cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        cfg.validate().expect("validate");
        let reg = cfg.build_registry().expect("build");
        assert_eq!(reg.deck_size(), 90);
        // All eight effects are enabled in the default deck.
        for kind in ALL_EFFECT_KINDS {
            assert!(reg.effect(kind).is_some(), "missing effect {kind:?}");
        }
        // Modifier pool has the full 20 copies.
        let pool: u32 = reg.modifier_pool().iter().map(|(_, c)| *c as u32).sum();
        assert_eq!(pool, 20);
    }

    /// A count that no longer sums to the declared deck size aborts.
    #[test]
    fn count_mismatch_aborts() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        cfg.deck.size += 1;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::CountMismatch { .. })
        ));
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

    /// Duplicate emote ids abort.
    #[test]
    fn duplicate_emote_id_aborts() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        let id = cfg.emote[0].id;
        cfg.emote[1].id = id;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::DuplicateEmoteId(_))
        ));
    }

    /// Disabling a content item excludes it from the registry without code changes.
    #[test]
    fn disabling_a_card_toggles_it_off() {
        let mut cfg = ContentConfig::from_toml(DEFAULT).expect("parse");
        for card in cfg.card.iter_mut() {
            if card.effect == Some(EffectKind::Shield) {
                card.enabled = false;
            }
        }
        cfg.deck.size -= 1; // keep counts consistent (Shield had 1 copy)
        let reg = cfg.build_registry().expect("build");
        assert!(reg.effect(EffectKind::Shield).is_none());
        assert!(reg.effect(EffectKind::Peek).is_some());
    }
}
