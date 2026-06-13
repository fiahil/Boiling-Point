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

/// The twelve Brewers — public asymmetric player identities, each bending
/// exactly one combat-core rule (change `boom2-brewers`). The protocol names
/// them and carries the static metadata a client needs to render a pick (the
/// one-sentence bent rule); the bends themselves live in the server engine.
/// Every player's chosen Brewer is public from before the first wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Brewer {
    /// In the fatal-wave volatility sort, your cards count as the lowest at
    /// their value (you slip out of ties).
    Featherhand,
    /// As a detonator you take half damage — but you can never play a Ward. 🌶️
    Cinderwright,
    /// You draft a 4th bucket in one ledger. (Inert until `boom2-apothecary`.)
    Connoisseur,
    /// Your grimoire holds two reserves — lock two exact spells. (Inert until
    /// `boom2-apothecary`.)
    Reservist,
    /// You may play two spells per wave. 🌶️
    Channeler,
    /// You top up ingredients to 4 each wave, not 3.
    Forager,
    /// Your named combos fire from a single half. (Full effect with
    /// `boom2-compounding`.)
    Herbalist,
    /// Your count-threshold cards treat the pot as 2 cards larger. (Full
    /// effect with `boom2-compounding`.)
    Distiller,
    /// When one of your combos fires it also adds volatility to the pot. 🌶️
    /// (Full effect with `boom2-compounding`.)
    Alchemist,
    /// Whenever anyone casts Peek, you secretly learn the boiling point too.
    Eavesdropper,
    /// When you split a won pot you round up, not down.
    Broker,
    /// Once per round you may commit your card after the wave reveals. 🌶️
    Lurker,
}

impl Brewer {
    /// Every Brewer, in a stable order (the full pool of 12).
    pub const ALL: [Brewer; 12] = [
        Brewer::Featherhand,
        Brewer::Cinderwright,
        Brewer::Connoisseur,
        Brewer::Reservist,
        Brewer::Channeler,
        Brewer::Forager,
        Brewer::Herbalist,
        Brewer::Distiller,
        Brewer::Alchemist,
        Brewer::Eavesdropper,
        Brewer::Broker,
        Brewer::Lurker,
    ];

    /// The stable display/config name (used by harness specs and reports).
    pub fn name(self) -> &'static str {
        match self {
            Brewer::Featherhand => "Featherhand",
            Brewer::Cinderwright => "Cinderwright",
            Brewer::Connoisseur => "Connoisseur",
            Brewer::Reservist => "Reservist",
            Brewer::Channeler => "Channeler",
            Brewer::Forager => "Forager",
            Brewer::Herbalist => "Herbalist",
            Brewer::Distiller => "Distiller",
            Brewer::Alchemist => "Alchemist",
            Brewer::Eavesdropper => "Eavesdropper",
            Brewer::Broker => "Broker",
            Brewer::Lurker => "Lurker",
        }
    }

    /// Parse a name back into a Brewer.
    pub fn by_name(name: &str) -> Option<Brewer> {
        Brewer::ALL.into_iter().find(|b| b.name() == name)
    }

    /// The one-sentence bent rule, as shown at the table (static design
    /// metadata — the discipline's "one readable sentence").
    pub fn bent_rule(self) -> &'static str {
        match self {
            Brewer::Featherhand => {
                "In the fatal-wave volatility sort, your cards count as the lowest at their value — you slip out of every tie."
            }
            Brewer::Cinderwright => {
                "When you're a detonator you take half damage — but you can never play a Ward."
            }
            Brewer::Connoisseur => "You draft a 4th bucket in one ledger.",
            Brewer::Reservist => {
                "Your grimoire holds two reserves — lock two exact spells, not one."
            }
            Brewer::Channeler => "You may play two spells per wave, not one.",
            Brewer::Forager => "You top up ingredients to 4 each wave, not 3.",
            Brewer::Herbalist => {
                "Your named combos fire from a single half — you never need both ingredients in the pot."
            }
            Brewer::Distiller => {
                "Your count-threshold cards treat the pot as 2 cards larger — payoffs come online sooner."
            }
            Brewer::Alchemist => {
                "When one of your combos fires it also adds volatility to the pot — chemistry as a weapon."
            }
            Brewer::Eavesdropper => {
                "Whenever anyone casts Peek, you secretly learn the boiling point too."
            }
            Brewer::Broker => "When you split a pot you round up, not down.",
            Brewer::Lurker => "Once per round you may commit your card after the wave reveals.",
        }
    }
}

/// The twelve pantry buckets (change `boom2-apothecary`): the named families a
/// player drafts ingredient *availability* from. A bucket makes a family of
/// cards eligible; it never sets how many appear — the server-side realizer
/// composes the fixed-size deck. The protocol names the buckets and carries the
/// static metadata a client needs to render the draft (the one-line family
/// read); the card families themselves are server content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PantryBucket {
    /// Low-volatility own-colour safe pushes. (Posture)
    Sage,
    /// Balanced mid-volatility / mid-point own-colour. (Posture)
    Mint,
    /// High-volatility own-colour weapons. (Posture)
    Nightshade,
    /// High-point own-colour treasure — the realizer caps these. (Posture)
    Saffron,
    /// Zero-point ghosts: volatility and colour presence, no prize. (Posture)
    Chalk,
    /// Greedy own-colour — high-volatility AND high-point. (Posture)
    Bilberry,
    /// Off-colour cards (kingmake / misdirect). (Toolkit)
    Ochre,
    /// Wilds — colourless pure danger / go-neutral. (Toolkit)
    Wisp,
    /// Named-combo pairs (mechanical teeth land with `boom2-compounding`). (Chemistry)
    Bramble,
    /// Count-threshold cards — score more in big / late pots (teeth land with
    /// `boom2-compounding`). (Chemistry)
    Honey,
    /// Ultra-low-volatility tiptoe cards (dodge detonator liability). (Specialist)
    Hellebore,
    /// Escalating cards — volatility climbs the longer they sit (teeth land
    /// with `boom2-compounding`). (Specialist)
    Embercap,
}

impl PantryBucket {
    /// Every pantry bucket, in a stable order (the full roster of 12).
    pub const ALL: [PantryBucket; 12] = [
        PantryBucket::Sage,
        PantryBucket::Mint,
        PantryBucket::Nightshade,
        PantryBucket::Saffron,
        PantryBucket::Chalk,
        PantryBucket::Bilberry,
        PantryBucket::Ochre,
        PantryBucket::Wisp,
        PantryBucket::Bramble,
        PantryBucket::Honey,
        PantryBucket::Hellebore,
        PantryBucket::Embercap,
    ];

    /// The stable display/config name (used by harness specs and reports).
    pub fn name(self) -> &'static str {
        match self {
            PantryBucket::Sage => "Sage",
            PantryBucket::Mint => "Mint",
            PantryBucket::Nightshade => "Nightshade",
            PantryBucket::Saffron => "Saffron",
            PantryBucket::Chalk => "Chalk",
            PantryBucket::Bilberry => "Bilberry",
            PantryBucket::Ochre => "Ochre",
            PantryBucket::Wisp => "Wisp",
            PantryBucket::Bramble => "Bramble",
            PantryBucket::Honey => "Honey",
            PantryBucket::Hellebore => "Hellebore",
            PantryBucket::Embercap => "Embercap",
        }
    }

    /// Parse a name back into a bucket.
    pub fn by_name(name: &str) -> Option<PantryBucket> {
        PantryBucket::ALL.into_iter().find(|b| b.name() == name)
    }

    /// The one-line family read, as shown in the draft UI (static design
    /// metadata, like a Brewer's bent rule).
    pub fn blurb(self) -> &'static str {
        match self {
            PantryBucket::Sage => "Low-volatility own-colour — safe pushes.",
            PantryBucket::Mint => "Balanced mid-volatility, mid-point own-colour.",
            PantryBucket::Nightshade => "High-volatility own-colour weapons.",
            PantryBucket::Saffron => "High-point own-colour treasure (at most 3 realized).",
            PantryBucket::Chalk => "Zero-point ghosts — volatility and colour presence, no prize.",
            PantryBucket::Bilberry => "Greedy own-colour — high-volatility and high-point.",
            PantryBucket::Ochre => "Off-colour cards — kingmake and misdirect.",
            PantryBucket::Wisp => "Wilds — colourless pure danger, go-neutral.",
            PantryBucket::Bramble => "Named-combo pairs — better together.",
            PantryBucket::Honey => "Count-threshold cards — score more in big pots.",
            PantryBucket::Hellebore => "Ultra-low-volatility tiptoe cards.",
            PantryBucket::Embercap => "Escalating cards — volatility climbs as they sit.",
        }
    }

    /// Whether this is a toolkit bucket (off-colour / wild families): the
    /// realizer's toolkit cap keys on the *cards*, but the Loyalist↔Diplomat
    /// read keys on whether any toolkit bucket was taken at all.
    pub fn is_toolkit(self) -> bool {
        matches!(self, PantryBucket::Ochre | PantryBucket::Wisp)
    }
}

/// The eight grimoire reagent buckets (change `boom2-apothecary`): each makes a
/// role-group of spells eligible; the realizer rolls a random spell within the
/// group per slot. The role-groups are static design metadata ([`Self::spells`]),
/// fixed alongside the 15 spell kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GrimoireBucket {
    /// Peek — know the line. (God-tier)
    Eyebright,
    /// The Wards: Cap, Halve, Redirect — survive the boom. (God-tier)
    Ironbark,
    /// Expose, Assay. (Info)
    Farsight,
    /// Surge, Hex. (Offense)
    Brimstone,
    /// Sour, Skim. (Disruption)
    Wormwood,
    /// Harvest, Double Down. (Cash-in)
    Goldenseal,
    /// Dampen, Quench. (Defense)
    Hoarfrost,
    /// Forage. (Tempo)
    Mandrake,
}

impl GrimoireBucket {
    /// Every grimoire bucket, in a stable order (the full roster of 8).
    pub const ALL: [GrimoireBucket; 8] = [
        GrimoireBucket::Eyebright,
        GrimoireBucket::Ironbark,
        GrimoireBucket::Farsight,
        GrimoireBucket::Brimstone,
        GrimoireBucket::Wormwood,
        GrimoireBucket::Goldenseal,
        GrimoireBucket::Hoarfrost,
        GrimoireBucket::Mandrake,
    ];

    /// The god-tier spells (the Eyebright + Ironbark families): know vs
    /// survive. The realizer's absolute god-tier cap keys on these kinds.
    pub const GOD_TIER_SPELLS: [SpellKind; 4] = [
        SpellKind::Peek,
        SpellKind::Cap,
        SpellKind::Halve,
        SpellKind::Redirect,
    ];

    /// The stable display/config name (used by harness specs and reports).
    pub fn name(self) -> &'static str {
        match self {
            GrimoireBucket::Eyebright => "Eyebright",
            GrimoireBucket::Ironbark => "Ironbark",
            GrimoireBucket::Farsight => "Farsight",
            GrimoireBucket::Brimstone => "Brimstone",
            GrimoireBucket::Wormwood => "Wormwood",
            GrimoireBucket::Goldenseal => "Goldenseal",
            GrimoireBucket::Hoarfrost => "Hoarfrost",
            GrimoireBucket::Mandrake => "Mandrake",
        }
    }

    /// Parse a name back into a bucket.
    pub fn by_name(name: &str) -> Option<GrimoireBucket> {
        GrimoireBucket::ALL.into_iter().find(|b| b.name() == name)
    }

    /// The one-line family read, as shown in the draft UI (static design
    /// metadata, like a Brewer's bent rule).
    pub fn blurb(self) -> &'static str {
        match self {
            GrimoireBucket::Eyebright => "Peek — know the boiling point.",
            GrimoireBucket::Ironbark => "The Wards: Cap, Halve, Redirect — survive the boom.",
            GrimoireBucket::Farsight => "Expose and Assay — read the pot.",
            GrimoireBucket::Brimstone => "Surge and Hex — push the pot hot and curse a rival.",
            GrimoireBucket::Wormwood => "Sour and Skim — disrupt scores, shed liability.",
            GrimoireBucket::Goldenseal => "Harvest and Double Down — cash a winning pot in.",
            GrimoireBucket::Hoarfrost => "Dampen and Quench — cool the cauldron.",
            GrimoireBucket::Mandrake => "Forage — the only in-round spell replenisher.",
        }
    }

    /// The role-group this bucket makes eligible (static design metadata —
    /// fixed alongside the 15 spell kinds, not server content).
    pub fn spells(self) -> &'static [SpellKind] {
        match self {
            GrimoireBucket::Eyebright => &[SpellKind::Peek],
            GrimoireBucket::Ironbark => &[SpellKind::Cap, SpellKind::Halve, SpellKind::Redirect],
            GrimoireBucket::Farsight => &[SpellKind::Expose, SpellKind::Assay],
            GrimoireBucket::Brimstone => &[SpellKind::Surge, SpellKind::Hex],
            GrimoireBucket::Wormwood => &[SpellKind::Sour, SpellKind::Skim],
            GrimoireBucket::Goldenseal => &[SpellKind::Harvest, SpellKind::DoubleDown],
            GrimoireBucket::Hoarfrost => &[SpellKind::Dampen, SpellKind::Quench],
            GrimoireBucket::Mandrake => &[SpellKind::Forage],
        }
    }

    /// Whether this bucket's family is god-tier (Peek / the Wards) — drafting
    /// it never lifts the realizer's absolute god-tier cap.
    pub fn is_god_tier(self) -> bool {
        matches!(self, GrimoireBucket::Eyebright | GrimoireBucket::Ironbark)
    }
}

/// A player's **recipe** (change `boom2-apothecary`): the buckets they took per
/// ledger plus the grimoire reserve(s) — the locked named spells. The recipe is
/// **public** (the table reads intent); the realized cards and draw order are
/// hidden from everyone, including the owner.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    /// The pantry buckets taken (2–3 distinct; the Connoisseur may take a 4th
    /// in one ledger).
    pub pantry: Vec<PantryBucket>,
    /// The grimoire buckets taken (2–3 distinct; same Connoisseur allowance).
    pub grimoire: Vec<GrimoireBucket>,
    /// The reserved (guaranteed) grimoire spells: at most one, two for the
    /// Reservist; each must belong to a taken bucket's role-group. The pantry
    /// is always pure-roll.
    pub reserves: Vec<SpellKind>,
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
