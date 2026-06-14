//! The Brewer bends (change `boom2-brewers`): per-Brewer rule hooks the engine
//! consults, plus the pre-game disjoint-pair deal.
//!
//! Each of the 12 Brewers bends exactly one combat-core rule, under the design
//! discipline (docs/06_boom2/02 §C3): one readable sentence, reads for the
//! whole table, **no free explosions** (half damage is the absolute ceiling —
//! enforced in [`detonator_damage`]) and **no free perfect information**
//! (the Eavesdropper learns nothing unless someone Peeks — enforced in
//! [`crate::game::runner::Game::resolve_wave`]'s conditional piggyback).
//!
//! Where the bends live:
//! - **Featherhand** — the fatal-wave liability sort ([`crate::game::round`]).
//! - **Cinderwright** — half detonator damage here; the no-Ward rule via the
//!   ward-free grimoire ([`crate::game::deck::Grimoire::build_excluding`]) plus
//!   frame/engine validation.
//! - **Channeler** — the per-wave spell allowance ([`spell_limit`]).
//! - **Forager** — the ingredient hand floor ([`hand_floor`]).
//! - **Eavesdropper** — the Peek piggyback (engine wave resolution).
//! - **Broker** — the winning-split rounding ([`crate::game::scoring`]).
//! - **Lurker** — the once-per-round deferred commit (the staged wave in
//!   [`crate::game::round`] / the session's two-step collection).
//! - **Connoisseur / Reservist** — the Apothecary-draft bends
//!   (`boom2-apothecary` activated the seams): the draft frame's allowances
//!   ([`extra_buckets`], [`reserve_allowance`]); the Cinderwright's no-Ward
//!   rule also filters Ironbark from their draft ([`excluded_buckets`]).
//! - **Herbalist / Distiller / Alchemist** (compounding) — consulted by
//!   [`crate::game::compounding`]: the Herbalist fires a completed combo twice
//!   ([`HERBALIST_COMBO_MULTIPLIER`]), the Distiller treats the pot as
//!   [`DISTILLER_POT_BONUS`] cards larger for count-thresholds, the Alchemist's
//!   fired combo adds [`ALCHEMIST_COMBO_VOLATILITY`] to the pot.
//!
//! Every magnitude is `[needs playtesting]` (Principle IV); the persona ×
//! Brewer harness matrix gates them.

use boiling_point_protocol::vocab::{Brewer, GrimoireBucket, SpellKind};

use crate::config::INGREDIENT_HAND;

/// The Forager's ingredient hand floor (everyone else uses
/// [`INGREDIENT_HAND`]). `[needs playtesting]`.
pub const FORAGER_HAND: u16 = 4;

/// The Channeler's per-wave spell allowance (everyone else casts at most one).
/// `[needs playtesting]`.
pub const CHANNELER_SPELLS_PER_WAVE: u8 = 2;

/// The three ward spells the Cinderwright can never play (their grimoire is
/// built without them; frames never enumerate them; the engine drops them).
pub const WARDS: [SpellKind; 3] = [SpellKind::Cap, SpellKind::Halve, SpellKind::Redirect];

/// Extra buckets the Connoisseur drafts in one ledger (the 4th bucket;
/// consulted by [`extra_buckets`] since `boom2-apothecary`).
/// `[needs playtesting]`.
pub const CONNOISSEUR_EXTRA_BUCKETS: u8 = 1;

/// Grimoire reserves (exact spells locked at the draft) — two for the
/// Reservist, one for everyone else (consulted by [`reserve_allowance`] since
/// `boom2-apothecary`). `[needs playtesting]`.
pub const RESERVIST_RESERVES: u8 = 2;

/// How many times a Herbalist's named combo fires when it completes — twice,
/// for double the payoff (everyone else fires it once, `boom2-compounding`).
/// `[needs playtesting]`.
pub const HERBALIST_COMBO_MULTIPLIER: u8 = 2;

/// Inert seam (`boom2-compounding`): extra cards the Distiller's
/// count-threshold cards see in the pot. `[needs playtesting]`.
pub const DISTILLER_POT_BONUS: u32 = 2;

/// Inert seam (`boom2-compounding`): volatility a fired Alchemist combo adds
/// to the pot. `[needs playtesting]`.
pub const ALCHEMIST_COMBO_VOLATILITY: u8 = 2;

/// The per-wave spell allowance for a seat: the Channeler's two, one for
/// everyone else (including no Brewer, e.g. the sync test runner).
pub fn spell_limit(brewer: Option<Brewer>) -> u8 {
    match brewer {
        Some(Brewer::Channeler) => CHANNELER_SPELLS_PER_WAVE,
        _ => 1,
    }
}

/// The ingredient hand floor for a seat: the Forager tops up to 4, everyone
/// else to the [`INGREDIENT_HAND`] floor.
pub fn hand_floor(brewer: Option<Brewer>) -> usize {
    match brewer {
        Some(Brewer::Forager) => FORAGER_HAND as usize,
        _ => INGREDIENT_HAND as usize,
    }
}

/// Spell kinds a seat's grimoire is built without (and may never cast): the
/// Cinderwright's wards; empty for everyone else.
pub fn excluded_spells(brewer: Option<Brewer>) -> &'static [SpellKind] {
    match brewer {
        Some(Brewer::Cinderwright) => &WARDS,
        _ => &[],
    }
}

/// Extra buckets ONE ledger may take past the draft's standard maximum: the
/// Connoisseur's 4th bucket; zero for everyone else (`boom2-apothecary`).
pub fn extra_buckets(brewer: Option<Brewer>) -> u8 {
    match brewer {
        Some(Brewer::Connoisseur) => CONNOISSEUR_EXTRA_BUCKETS,
        _ => 0,
    }
}

/// Grimoire reserves a seat may lock at the draft: the Reservist's two, one
/// for everyone else (`boom2-apothecary`).
pub fn reserve_allowance(brewer: Option<Brewer>) -> u8 {
    match brewer {
        Some(Brewer::Reservist) => RESERVIST_RESERVES,
        _ => 1,
    }
}

/// Grimoire buckets a seat is never offered in the draft: the Cinderwright can
/// never hold a Ward, and Ironbark's whole family is the three wards — an
/// offered-but-dead bucket would violate the frame's exact-enumeration rule.
pub fn excluded_buckets(brewer: Option<Brewer>) -> &'static [GrimoireBucket] {
    match brewer {
        Some(Brewer::Cinderwright) => &[GrimoireBucket::Ironbark],
        _ => &[],
    }
}

/// A detonator's damage after their Brewer bend: the Cinderwright takes half,
/// rounded **up** — the discipline's hard ceiling (at most *to* half damage,
/// never below it, never immunity: `ceil(1/2) = 1`). Everyone else takes the
/// full share (wards then apply normally; the Cinderwright holds none).
pub fn detonator_damage(brewer: Option<Brewer>, share: u32) -> u32 {
    match brewer {
        Some(Brewer::Cinderwright) => share.div_ceil(2),
        _ => share,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The discipline guardrail: the Cinderwright's reduction is exactly to
    /// half (rounded up) — never more than half off, and never full immunity.
    #[test]
    fn cinderwright_damage_is_capped_at_half_never_zero() {
        for share in 1..=50u32 {
            let taken = detonator_damage(Some(Brewer::Cinderwright), share);
            assert!(taken * 2 >= share, "below the half-damage ceiling: {share}");
            assert!(taken > 0, "free explosion at share {share}");
            assert!(taken <= share);
        }
        assert_eq!(detonator_damage(Some(Brewer::Cinderwright), 0), 0);
        // Everyone else takes the full share.
        for brewer in Brewer::ALL {
            if brewer != Brewer::Cinderwright {
                assert_eq!(detonator_damage(Some(brewer), 9), 9);
            }
        }
        assert_eq!(detonator_damage(None, 9), 9);
    }

    /// Only the Channeler gets a second cast; only the Forager a deeper hand;
    /// only the Cinderwright a spell exclusion (exactly the three wards).
    #[test]
    fn bends_apply_to_exactly_their_brewer() {
        for brewer in Brewer::ALL {
            let b = Some(brewer);
            assert_eq!(
                spell_limit(b),
                if brewer == Brewer::Channeler { 2 } else { 1 }
            );
            assert_eq!(hand_floor(b), if brewer == Brewer::Forager { 4 } else { 3 });
            let excluded = excluded_spells(b);
            if brewer == Brewer::Cinderwright {
                assert_eq!(excluded, &WARDS);
            } else {
                assert!(excluded.is_empty());
            }
        }
        assert_eq!(spell_limit(None), 1);
        assert_eq!(hand_floor(None), INGREDIENT_HAND as usize);
    }

    /// The draft bends (`boom2-apothecary`): only the Connoisseur gets the 4th
    /// bucket, only the Reservist a second reserve, and only the Cinderwright
    /// loses a bucket — exactly Ironbark, whose family is exactly the wards.
    #[test]
    fn draft_bends_apply_to_exactly_their_brewer() {
        for brewer in Brewer::ALL {
            let b = Some(brewer);
            assert_eq!(
                extra_buckets(b),
                if brewer == Brewer::Connoisseur {
                    CONNOISSEUR_EXTRA_BUCKETS
                } else {
                    0
                }
            );
            assert_eq!(
                reserve_allowance(b),
                if brewer == Brewer::Reservist {
                    RESERVIST_RESERVES
                } else {
                    1
                }
            );
            let excluded = excluded_buckets(b);
            if brewer == Brewer::Cinderwright {
                assert_eq!(excluded, &[GrimoireBucket::Ironbark]);
            } else {
                assert!(excluded.is_empty());
            }
        }
        assert_eq!(extra_buckets(None), 0);
        assert_eq!(reserve_allowance(None), 1);
        assert!(excluded_buckets(None).is_empty());
        // The no-Ward rule and the no-Ironbark rule are the same rule: the
        // bucket's family is exactly the ward list.
        assert_eq!(GrimoireBucket::Ironbark.spells(), &WARDS);
    }
}
