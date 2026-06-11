//! Shared wire vocabulary: the public value-types and tag enums that appear in
//! messages.
//!
//! These are *tags and views*, not behaviour. The behaviour keyed by
//! [`SpellKind`]/[`ModifierKind`] lives entirely in the server's content
//! module — the protocol only names them (plus the static metadata clients need
//! to render a spell: its timing [`SpellMode`] and its [`TargetKind`]).
//!
//! The v2 card model (change `boom2-combat-core`) splits cards into two types:
//! **ingredients** (colour · volatility 0–7 · points 0–3) that go into the
//! cauldron, and **spells** (active effects, never in the pot, no points and no
//! volatility of their own).

use serde::{Deserialize, Serialize};

use crate::ids::CardId;

/// A card's colour — whose interests it serves. `Wild` belongs to no player,
/// never wins dominance, and scores no points (points score only on colored
/// Votes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    /// Ruby Red player colour.
    Ruby,
    /// Sapphire Blue player colour.
    Sapphire,
    /// Emerald Green player colour.
    Emerald,
    /// Amethyst Purple player colour.
    Amethyst,
    /// Colourless wild — volatility only, no points, no dominance.
    Wild,
}

impl Color {
    /// The four player colours, excluding `Wild`.
    pub const PLAYER_COLORS: [Color; 4] = [
        Color::Ruby,
        Color::Sapphire,
        Color::Emerald,
        Color::Amethyst,
    ];
}

/// When a spell's effect happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpellMode {
    /// Fires on cast, then spent — visible to the table at cast.
    Instant,
    /// Primed face-down on cast; fires on its trigger, then spent. Hidden until
    /// it fires; an unfired Active is a wasted bet.
    Active,
}

/// What a spell must be aimed at when cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetKind {
    /// No target.
    None,
    /// A player at the table (other than the caster).
    Player,
    /// One of the four player colours.
    Color,
}

/// The chosen target a cast rides with, matching the spell's [`TargetKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SpellTarget {
    /// A player target (Redirect, Hex).
    Player {
        /// The targeted player.
        player: crate::ids::PlayerId,
    },
    /// A colour target (Double Down, Sour).
    Color {
        /// The targeted player colour.
        color: Color,
    },
}

/// The fifteen grimoire spells. The protocol uses this as a tag plus static
/// metadata (mode, target kind); magnitudes and resolution live in the server's
/// content module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpellKind {
    /// Privately learn the exact boiling point. (Info, Instant)
    Peek,
    /// Reveal one face-down pot ingredient to the table. (Info, Instant)
    Expose,
    /// Privately learn the dominant colour and its point lead. (Info, Instant)
    Assay,
    /// Reduce cauldron volatility. (Volatility, Instant)
    Dampen,
    /// Add cauldron volatility. (Volatility, Instant)
    Surge,
    /// The cauldron cannot explode next wave (table-wide). (Volatility, Instant)
    Quench,
    /// As a detonator, eat at most a small fixed loss. (Ward, Active)
    Cap,
    /// As a detonator, eat half the loss. (Ward, Active)
    Halve,
    /// As a detonator, shove the loss onto a chosen player (cascades). (Ward, Active)
    Redirect,
    /// Double one colour's points in the pot. (Score, Instant)
    DoubleDown,
    /// Halve one chosen colour's points in the pot. (Score, Instant)
    Sour,
    /// If your colour wins the pot, gain a bonus. (Cash-in, Active)
    Harvest,
    /// Discard your last-added pot ingredient (its points and volatility leave). (Economy, Instant)
    Skim,
    /// Draw two spells — the only in-round replenisher. (Economy, Instant)
    Forage,
    /// A chosen player takes extra damage on any explosion this round. (Offense, Active)
    Hex,
}

impl SpellKind {
    /// Every spell kind, in a stable order (the full 15-spell grimoire).
    pub const ALL: [SpellKind; 15] = [
        SpellKind::Peek,
        SpellKind::Expose,
        SpellKind::Assay,
        SpellKind::Dampen,
        SpellKind::Surge,
        SpellKind::Quench,
        SpellKind::Cap,
        SpellKind::Halve,
        SpellKind::Redirect,
        SpellKind::DoubleDown,
        SpellKind::Sour,
        SpellKind::Harvest,
        SpellKind::Skim,
        SpellKind::Forage,
        SpellKind::Hex,
    ];

    /// This spell's timing mode (static design metadata, identical for all copies).
    pub fn mode(self) -> SpellMode {
        match self {
            SpellKind::Cap
            | SpellKind::Halve
            | SpellKind::Redirect
            | SpellKind::Harvest
            | SpellKind::Hex => SpellMode::Active,
            _ => SpellMode::Instant,
        }
    }

    /// What this spell must target when cast (static design metadata).
    pub fn target_kind(self) -> TargetKind {
        match self {
            SpellKind::Redirect | SpellKind::Hex => TargetKind::Player,
            SpellKind::DoubleDown | SpellKind::Sour => TargetKind::Color,
            _ => TargetKind::None,
        }
    }
}

/// The six cauldron-modifier kinds. A tag only; offsets/multipliers live in
/// `server::content::modifier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModifierKind {
    /// Cauldron starts with extra volatility.
    Residue,
    /// Boiling point lowered (explosions more likely).
    ThinIce,
    /// Boiling point raised (explosions rarer).
    DeepCauldron,
    /// Colourless per-card bonus to the pot total.
    BountifulBrew,
    /// All pot points multiplied — win and explosion alike.
    DoubleStakes,
    /// The lowest-point colour present wins instead of the highest.
    Reversal,
}

/// The fully-revealed attributes of an ingredient, as shown in a hand (to its
/// owner), on an Expose, or at the depile (to everyone). Ingredients in the
/// cauldron are NOT sent as `IngredientView` during play — they are hidden
/// until revealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngredientView {
    /// The ingredient's printed colour.
    pub color: Color,
    /// Explosion risk this ingredient contributes (0–7).
    pub volatility: u8,
    /// Point value when played as a colored Vote (0–3). Zero when played colorless.
    pub points: u8,
}

/// An ingredient in a player's own hand: its id (for committing) plus its
/// visible attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandIngredient {
    /// Stable id used to commit this ingredient.
    pub id: CardId,
    /// The ingredient's revealed attributes (a hand is private to its owner).
    pub view: IngredientView,
}

/// A spell in a player's own grimoire hand: its id (for casting) plus its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandSpell {
    /// Stable id used to cast this spell.
    pub id: CardId,
    /// Which of the fifteen spells this is.
    pub kind: SpellKind,
}
