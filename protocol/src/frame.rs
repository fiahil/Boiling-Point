//! Decision frames: the server-enumerated pending decision + legal action set.
//!
//! For every decision a player owes, the server sends a
//! [`crate::server::ServerMessage::DecisionFrame`] carrying the decision kind and
//! its **complete legal action set** (change `boom2-ai-client`, capability
//! `boom-decision-frame`). The set is exact in both directions: every enumerated
//! action passes server validation, and every action that would pass validation
//! is enumerated. Brains and rendering clients therefore *choose among options*
//! instead of re-deriving rules — legality is computed once, on the server
//! (Principle I inverted: the validator's verdict, published before the action).
//!
//! A frame may contain only information its recipient is permitted to have: the
//! player's own hand-derived options and public table facts. No boiling point,
//! no opponents' hidden cards, no deck contents.
//!
//! [`PendingDecision`] is an extensible tagged enum. The v2 surface will grow
//! Brewer-pick and Apothecary-draft kinds when `boom2-brewers` /
//! `boom2-apothecary` land their vocabulary; today's combat core owes exactly
//! one decision kind, the per-wave commit.

use serde::{Deserialize, Serialize};

use crate::ids::{CardId, PlayerId};
use crate::vocab::{Color, HandIngredient, SpellKind, SpellTarget};

/// The legal targets a castable spell may be aimed at, matching the spell's
/// [`crate::vocab::TargetKind`]. Targeted spells enumerate every legal choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TargetOptions {
    /// The spell takes no target (cast it bare).
    None,
    /// The spell targets a player: exactly these players are legal.
    Players {
        /// The legal player targets (seated, never the caster).
        players: Vec<PlayerId>,
    },
    /// The spell targets a colour: exactly these colours are legal.
    Colors {
        /// The legal colour targets (player colours, never Wild).
        colors: Vec<Color>,
    },
}

impl TargetOptions {
    /// Whether `target` is one of the enumerated legal choices for this spell.
    pub fn permits(&self, target: Option<SpellTarget>) -> bool {
        match (self, target) {
            (TargetOptions::None, None) => true,
            (TargetOptions::Players { players }, Some(SpellTarget::Player { player })) => {
                players.contains(&player)
            }
            (TargetOptions::Colors { colors }, Some(SpellTarget::Color { color })) => {
                colors.contains(&color)
            }
            _ => false,
        }
    }
}

/// One castable spell in a wave-commit frame: the hand card, its kind (so a
/// client needs no lookup), and its enumerated legal targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastableSpell {
    /// The grimoire hand card to cast.
    pub spell: CardId,
    /// Which of the fifteen spells it is.
    pub kind: SpellKind,
    /// The complete set of legal targets for this cast.
    pub targets: TargetOptions,
}

/// One playable ingredient in a wave-commit frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayableIngredient {
    /// The hand ingredient (id + the owner-visible attributes).
    pub ingredient: HandIngredient,
    /// Whether it may be played colorless (volatility only, zero points).
    /// Always true in the current rules; carried so a future hook that
    /// restricts the colorless Vote stays expressible without a shape change.
    pub colorless_allowed: bool,
}

/// The pending decision a player owes, with its complete legal action set.
///
/// Extensible by design: Brewer pick and Apothecary draft kinds join this enum
/// when their owning changes land their protocol vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PendingDecision {
    /// The per-wave commit: a mandatory ingredient-or-pass plus at most one
    /// optional spell. An empty `spells` list means no further cast is legal
    /// this wave (none held, or one already cast).
    WaveCommit {
        /// Every hand ingredient that may be committed this wave.
        playable: Vec<PlayableIngredient>,
        /// Whether passing is legal (always true for an active player).
        can_pass: bool,
        /// Every spell that may still be cast this wave, with its legal targets.
        spells: Vec<CastableSpell>,
    },
}

impl PendingDecision {
    /// Whether a `CommitIngredient { card, colorless }` submission is inside
    /// this frame's legal set.
    pub fn permits_play(&self, card: CardId, colorless: bool) -> bool {
        match self {
            PendingDecision::WaveCommit { playable, .. } => playable
                .iter()
                .any(|p| p.ingredient.id == card && (!colorless || p.colorless_allowed)),
        }
    }

    /// Whether a `CommitPass` submission is inside this frame's legal set.
    pub fn permits_pass(&self) -> bool {
        match self {
            PendingDecision::WaveCommit { can_pass, .. } => *can_pass,
        }
    }

    /// Whether a `CastSpell { spell, target }` submission is inside this
    /// frame's legal set.
    pub fn permits_cast(&self, spell: CardId, target: Option<SpellTarget>) -> bool {
        match self {
            PendingDecision::WaveCommit { spells, .. } => spells
                .iter()
                .any(|s| s.spell == spell && s.targets.permits(target)),
        }
    }
}
