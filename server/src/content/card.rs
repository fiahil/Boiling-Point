//! Ingredient content: the dealt unit of the pantry.
//!
//! An ingredient definition is pure data (a colour *slot*, volatility, points,
//! copies). It is a distinct content kind from [`super::spell`] and
//! [`super::modifier`] behaviour — these are never merged into one union (the
//! loop depends only on these stable shapes, never on a specific named card).

use serde::{Deserialize, Serialize};

use boiling_point_protocol::vocab::Compounding;

/// How the deck builder instantiates an ingredient's colour for a given seat.
/// The pantry is colour-anchored: the same definitions yield each player a deck
/// identical in composition up to colour choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PantrySlot {
    /// The owning player's own colour (a Vote that scores for them).
    Own,
    /// Another player's colour (the kingmake / misdirect toolkit).
    OffColor,
    /// Colourless wild — volatility only, zero points, no dominance.
    Wild,
}

/// An ingredient archetype as defined in content config: its slot and attributes
/// plus how many physical copies exist in each player's pantry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngredientDef {
    /// How the colour is assigned per seat.
    pub slot: PantrySlot,
    /// Explosion risk contributed (0–7, high values rare).
    pub volatility: u8,
    /// Point value when played as a colored Vote (0–3).
    pub points: u8,
    /// Number of physical copies of this archetype in the pantry.
    pub copies: u16,
}

/// One rollable archetype within an Apothecary bucket family
/// (`boom2-apothecary`). No copy count: a bucket feeds *availability*, and the
/// realizer decides amounts to the fixed deck size under its caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BucketCard {
    /// How the colour is assigned per seat (derived from the bucket).
    pub slot: PantrySlot,
    /// Explosion risk contributed (0–7).
    pub volatility: u8,
    /// Point value when played as a colored Vote (0–3).
    pub points: u8,
    /// The in-pot interaction this archetype carries, if any
    /// (`boom2-compounding` — the Bramble/Honey teeth). `None` for a plain card.
    pub compounding: Option<Compounding>,
}
