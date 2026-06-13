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
//! [`PendingDecision`] is an extensible tagged enum. The combat core owes the
//! per-wave commit; `boom2-brewers` added the pre-game Brewer pick (the dealt
//! 2-of-pair offer IS the frame) and the Lurker's defer option on the wave
//! commit; `boom2-apothecary` added the two-ledger Apothecary draft (the
//! bucket rosters and allowances ARE the frame).

use serde::{Deserialize, Serialize};

use crate::ids::{CardId, PlayerId};
use crate::vocab::{
    Brewer, Color, GrimoireBucket, HandIngredient, PantryBucket, Recipe, SpellKind, SpellTarget,
};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PendingDecision {
    /// The per-wave commit: a mandatory ingredient-or-pass plus optional
    /// spells (one for most players; the Channeler's frame keeps listing
    /// spells until their second cast). An empty `spells` list means no
    /// further cast is legal this wave (none held, or the allowance spent).
    WaveCommit {
        /// Every hand ingredient that may be committed this wave.
        playable: Vec<PlayableIngredient>,
        /// Whether passing is legal (always true for an active player).
        can_pass: bool,
        /// Every spell that may still be cast this wave, with its legal targets.
        spells: Vec<CastableSpell>,
        /// Whether deferring the commit until after the wave reveals is legal
        /// (the Lurker's once-per-round bend; `false` for everyone else).
        can_defer: bool,
    },
    /// The pre-game Brewer pick: the dealt disjoint pair (the 2-of-pair offer).
    /// Exactly one option must be picked; pairs are disjoint around the table,
    /// so any combination of picks is unique.
    BrewerPick {
        /// The two offered Brewers (this seat's disjoint pair).
        options: Vec<Brewer>,
    },
    /// The pre-game Apothecary draft (`boom2-apothecary`), after the Brewer
    /// pick: take a set of distinct buckets per ledger and (optionally) lock
    /// the grimoire reserve(s). Answered with one
    /// [`crate::client::ClientMessage::SubmitRecipe`]; final on receipt. A
    /// legal recipe is enumerated by this frame's options and allowances —
    /// see [`PendingDecision::permits_recipe`].
    ApothecaryDraft {
        /// The pantry buckets on offer (the full roster).
        pantry_options: Vec<PantryBucket>,
        /// The grimoire buckets on offer (the full roster, minus families this
        /// seat may never hold — the Cinderwright is not offered Ironbark).
        grimoire_options: Vec<GrimoireBucket>,
        /// Minimum buckets per ledger.
        picks_min: u8,
        /// Maximum buckets per ledger.
        picks_max: u8,
        /// How many buckets past `picks_max` ONE ledger (the player's choice)
        /// may take — the Connoisseur's 4th bucket (0 for everyone else).
        bonus_buckets: u8,
        /// How many grimoire reserves may be locked (1; 2 for the Reservist).
        reserves_max: u8,
        /// A legal quick-pick recipe: the one-tap default, also applied
        /// server-side if the timer lapses (the lobby-ethos auto-pick).
        suggested: Recipe,
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
            _ => false,
        }
    }

    /// Whether a `CommitPass` submission is inside this frame's legal set.
    pub fn permits_pass(&self) -> bool {
        match self {
            PendingDecision::WaveCommit { can_pass, .. } => *can_pass,
            _ => false,
        }
    }

    /// Whether a `CastSpell { spell, target }` submission is inside this
    /// frame's legal set.
    pub fn permits_cast(&self, spell: CardId, target: Option<SpellTarget>) -> bool {
        match self {
            PendingDecision::WaveCommit { spells, .. } => spells
                .iter()
                .any(|s| s.spell == spell && s.targets.permits(target)),
            _ => false,
        }
    }

    /// Whether a `CommitDefer` submission is inside this frame's legal set.
    pub fn permits_defer(&self) -> bool {
        match self {
            PendingDecision::WaveCommit { can_defer, .. } => *can_defer,
            _ => false,
        }
    }

    /// Whether a `PickBrewer { brewer }` submission is inside this frame's
    /// legal set.
    pub fn permits_pick(&self, brewer: Brewer) -> bool {
        match self {
            PendingDecision::BrewerPick { options } => options.contains(&brewer),
            _ => false,
        }
    }

    /// Whether a `SubmitRecipe { recipe }` submission is inside this frame's
    /// legal set: distinct buckets drawn from the offered rosters, both ledger
    /// counts within the allowance (one ledger may run one over on the
    /// Connoisseur's bonus), and every reserve a spell of a taken grimoire
    /// bucket, within the reserve allowance.
    pub fn permits_recipe(&self, recipe: &Recipe) -> bool {
        let PendingDecision::ApothecaryDraft {
            pantry_options,
            grimoire_options,
            picks_min,
            picks_max,
            bonus_buckets,
            reserves_max,
            ..
        } = self
        else {
            return false;
        };
        let distinct_offered_pantry = recipe
            .pantry
            .iter()
            .enumerate()
            .all(|(i, b)| pantry_options.contains(b) && !recipe.pantry[..i].contains(b));
        let distinct_offered_grimoire = recipe
            .grimoire
            .iter()
            .enumerate()
            .all(|(i, b)| grimoire_options.contains(b) && !recipe.grimoire[..i].contains(b));
        let (p, g) = (recipe.pantry.len(), recipe.grimoire.len());
        let (min, max) = (*picks_min as usize, *picks_max as usize);
        let bonus = *bonus_buckets as usize;
        // The bonus allowance lands in ONE ledger, never both.
        let counts_ok = p >= min
            && g >= min
            && ((p <= max && g <= max + bonus) || (p <= max + bonus && g <= max));
        let reserves_ok = recipe.reserves.len() <= *reserves_max as usize
            && recipe
                .reserves
                .iter()
                .all(|k| recipe.grimoire.iter().any(|b| b.spells().contains(k)));
        distinct_offered_pantry && distinct_offered_grimoire && counts_ok && reserves_ok
    }
}
