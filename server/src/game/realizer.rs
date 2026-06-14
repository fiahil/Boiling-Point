//! The Apothecary realizer (`boom2-apothecary`): composes each player's decks
//! from their public recipe, server-side, **re-rolled every game**.
//!
//! Buckets feed *availability*, not distribution: a recipe's buckets define the
//! eligible pool; the realizer decides amounts, to a **fixed size** (the
//! configured pantry/grimoire sizes), under the caps in
//! [`crate::config::ApothecaryConfig`] — toolkit ≤ cap, Treasure (max-point
//! ingredients) ≤ cap, god-tier spells ≤ cap. Premium caps are **absolute**:
//! when a roll would bust a cap the slot falls through to **commons** (the
//! Sage + Mint families for the pantry; the union of non-god role-groups for
//! the grimoire), so a bigger or greedier pick-set adds commons, never premium,
//! and any legal pick-set yields a legal deck.
//!
//! The realized cards and their draw order are a server secret — hidden from
//! everyone including the owner, who learns the deck as they draw it. Only the
//! recipe (the bucket sets + reserves) is public.
//!
//! Determinism: one seeded RNG per deck; buckets are canonicalized (sorted,
//! deduped) so submission order never changes the roll; the same
//! `(seed, recipe, content)` always realizes the identical deck — replays
//! re-run it from the recorded recipes.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use boiling_point_protocol::CardId;
use boiling_point_protocol::vocab::{Color, GrimoireBucket, PantryBucket, SpellKind};

use crate::config::MAX_POINTS;
use crate::content::card::{BucketCard, PantrySlot};
use crate::content::registry::ContentRegistry;

use super::card::{Ingredient, Spell};

/// Fewest buckets a ledger may take in the draft. `[needs playtesting]`.
pub const DRAFT_PICKS_MIN: u8 = 2;
/// Most buckets a ledger may take in the draft (before the Connoisseur's
/// bonus). `[needs playtesting]`.
pub const DRAFT_PICKS_MAX: u8 = 3;

/// Whether a pantry archetype is Treasure (a maximum-point ingredient) — the
/// attribute the absolute Treasure cap keys on, whatever bucket it came from.
fn is_treasure(card: &BucketCard) -> bool {
    card.points == MAX_POINTS
}

/// Whether a pantry archetype is a toolkit card (off-colour or wild) — the
/// attribute the toolkit cap (the colour anchor's complement) keys on.
fn is_toolkit(card: &BucketCard) -> bool {
    card.slot != PantrySlot::Own
}

/// The pantry commons: the Sage + Mint families (the own-colour staples) — the
/// fall-through filler when a capped slot cannot stay in its bucket. Always
/// non-empty (config validation requires every bucket family populated).
fn pantry_commons(registry: &ContentRegistry) -> Vec<BucketCard> {
    let mut commons: Vec<BucketCard> = registry.bucket_family(PantryBucket::Sage).to_vec();
    commons.extend_from_slice(registry.bucket_family(PantryBucket::Mint));
    commons.retain(|c| !is_treasure(c) && !is_toolkit(c));
    commons
}

/// The grimoire commons: every non-god role-group's spells (minus any the seat
/// may never hold) — the fall-through filler once the god-tier budget is spent.
fn grimoire_commons(excluded: &[SpellKind]) -> Vec<SpellKind> {
    GrimoireBucket::ALL
        .into_iter()
        .filter(|b| !b.is_god_tier())
        .flat_map(|b| b.spells().iter().copied())
        .filter(|k| !excluded.contains(k))
        .collect()
}

/// Canonicalize a bucket set: sorted and deduped, so the roll is independent
/// of submission order (validation upstream already rejects duplicates).
fn canonical<B: Ord + Copy>(buckets: &[B]) -> Vec<B> {
    let mut sorted = buckets.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted
}

/// Per-bucket slot shares of `total`, remainder to the earliest buckets: with
/// 2 buckets the deck is concentrated, with 3 varied — focus vs breadth is the
/// ONLY thing the bucket count changes, never size or legality.
fn shares(total: usize, buckets: usize) -> Vec<usize> {
    let n = buckets.max(1);
    (0..n)
        .map(|i| total / n + usize::from(i < total % n))
        .collect()
}

/// Realize one seat's pantry from its recipe buckets: the configured fixed
/// size, the colour anchor held via the toolkit cap, Treasure capped
/// absolutely, shuffled draw order. `next_id` keeps instance ids unique across
/// every deck in the game.
pub fn realize_pantry(
    registry: &ContentRegistry,
    buckets: &[PantryBucket],
    own_color: Color,
    next_id: &mut u32,
    seed: u64,
) -> Vec<Ingredient> {
    let mut rng = StdRng::seed_from_u64(seed);
    let deck_size = registry.pantry_size() as usize;
    let caps = registry.apothecary();
    let mut treasure_left = caps.treasure_cap as usize;
    let mut toolkit_left = caps.toolkit_cap as usize;

    let buckets = canonical(buckets);
    let mut chosen: Vec<BucketCard> = Vec::with_capacity(deck_size);
    for (bucket, share) in buckets.iter().zip(shares(deck_size, buckets.len())) {
        let family = registry.bucket_family(*bucket);
        for _ in 0..share {
            // Roll only among the archetypes the remaining budgets afford; a
            // bucket with nothing affordable left defers the slot to commons.
            let affordable: Vec<&BucketCard> = family
                .iter()
                .filter(|c| !is_treasure(c) || treasure_left > 0)
                .filter(|c| !is_toolkit(c) || toolkit_left > 0)
                .collect();
            if affordable.is_empty() {
                continue;
            }
            let card = *affordable[rng.gen_range(0..affordable.len())];
            if is_treasure(&card) {
                treasure_left -= 1;
            }
            if is_toolkit(&card) {
                toolkit_left -= 1;
            }
            chosen.push(card);
        }
    }
    // Every unfilled slot (capped-out rolls; a degenerate empty bucket set)
    // falls through to commons: bigger decks add commons, never premium
    // (commons are own-colour non-treasure by construction).
    let commons = pantry_commons(registry);
    while chosen.len() < deck_size {
        chosen.push(commons[rng.gen_range(0..commons.len())]);
    }

    // Instantiate colours per seat (like the fixed deal: off-colour slots
    // cycle deterministically through the other player colours) and shuffle
    // the draw order.
    let off_colors: Vec<Color> = Color::PLAYER_COLORS
        .into_iter()
        .filter(|c| *c != own_color)
        .collect();
    let mut off_cursor = 0usize;
    let mut cards: Vec<Ingredient> = chosen
        .into_iter()
        .map(|c| {
            let color = match c.slot {
                PantrySlot::Own => own_color,
                PantrySlot::OffColor => {
                    let color = off_colors[off_cursor % off_colors.len()];
                    off_cursor += 1;
                    color
                }
                PantrySlot::Wild => Color::Wild,
            };
            let card = Ingredient {
                id: CardId(*next_id),
                color,
                volatility: c.volatility,
                points: c.points,
                compounding: c.compounding,
            };
            *next_id += 1;
            card
        })
        .collect();
    cards.shuffle(&mut rng);
    cards
}

/// Realize one seat's grimoire from its recipe buckets and reserves: the
/// configured fixed size, reserves placed first (a reserve guarantees its
/// named spell), god-tier capped absolutely, `excluded` kinds (the
/// Cinderwright's wards) never realized, shuffled draw order.
pub fn realize_grimoire(
    registry: &ContentRegistry,
    buckets: &[GrimoireBucket],
    reserves: &[SpellKind],
    excluded: &[SpellKind],
    next_id: &mut u32,
    seed: u64,
) -> Vec<Spell> {
    let mut rng = StdRng::seed_from_u64(seed);
    let deck_size = registry.grimoire_size() as usize;
    let caps = registry.apothecary();
    let mut god_left = caps.god_tier_cap as usize;
    let god = GrimoireBucket::GOD_TIER_SPELLS;

    // Reserves first: each locks its named spell into the deck (consuming god
    // budget where due). A reserve the cap cannot afford degrades to a normal
    // commons roll — unreachable under the shipped allowances (≤2 reserves,
    // cap 2) but the cap stays absolute under any tuning.
    let mut kinds: Vec<SpellKind> = Vec::new();
    for &kind in reserves.iter().take(deck_size) {
        if excluded.contains(&kind) {
            continue;
        }
        if god.contains(&kind) {
            if god_left == 0 {
                continue;
            }
            god_left -= 1;
        }
        kinds.push(kind);
    }

    let buckets = canonical(buckets);
    let remaining = deck_size - kinds.len();
    for (bucket, share) in buckets.iter().zip(shares(remaining, buckets.len())) {
        let family: Vec<SpellKind> = bucket
            .spells()
            .iter()
            .copied()
            .filter(|k| !excluded.contains(k))
            .collect();
        for _ in 0..share {
            let affordable: Vec<SpellKind> = family
                .iter()
                .copied()
                .filter(|k| !god.contains(k) || god_left > 0)
                .collect();
            if affordable.is_empty() {
                continue;
            }
            let kind = affordable[rng.gen_range(0..affordable.len())];
            if god.contains(&kind) {
                god_left -= 1;
            }
            kinds.push(kind);
        }
    }
    // Every unfilled slot falls through to the non-god commons.
    let commons = grimoire_commons(excluded);
    while kinds.len() < deck_size {
        kinds.push(commons[rng.gen_range(0..commons.len())]);
    }

    let mut spells: Vec<Spell> = kinds
        .into_iter()
        .map(|kind| {
            let spell = Spell {
                id: CardId(*next_id),
                kind,
            };
            *next_id += 1;
            spell
        })
        .collect();
    spells.shuffle(&mut rng);
    spells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::brewers::WARDS;

    fn registry() -> ContentRegistry {
        let cfg = crate::config::ContentConfig::from_toml(include_str!("../../content.toml"))
            .expect("parse");
        cfg.build_registry().expect("build")
    }

    fn own_count(deck: &[Ingredient], own: Color) -> usize {
        deck.iter().filter(|c| c.color == own).count()
    }

    fn treasure_count(deck: &[Ingredient]) -> usize {
        deck.iter().filter(|c| c.points == MAX_POINTS).count()
    }

    fn toolkit_count(deck: &[Ingredient], own: Color) -> usize {
        deck.iter().filter(|c| c.color != own).count()
    }

    fn god_count(deck: &[Spell]) -> usize {
        deck.iter()
            .filter(|s| GrimoireBucket::GOD_TIER_SPELLS.contains(&s.kind))
            .count()
    }

    /// Any pick-set realizes to the full fixed size with every cap held —
    /// swept across bucket counts (2 and 3) and many seeds.
    #[test]
    fn realized_pantry_is_fixed_size_and_capped() {
        let reg = registry();
        let picks: [&[PantryBucket]; 3] = [
            &[PantryBucket::Sage, PantryBucket::Nightshade],
            &[PantryBucket::Mint, PantryBucket::Ochre, PantryBucket::Wisp],
            &[
                PantryBucket::Saffron,
                PantryBucket::Bilberry,
                PantryBucket::Honey,
            ],
        ];
        for buckets in picks {
            for seed in 0..50u64 {
                let mut next_id = 0;
                let deck = realize_pantry(&reg, buckets, Color::Ruby, &mut next_id, seed);
                assert_eq!(deck.len(), 30, "fixed size regardless of picks");
                assert!(toolkit_count(&deck, Color::Ruby) <= 7, "toolkit cap");
                assert!(treasure_count(&deck) <= 3, "treasure cap");
                // The colour anchor: the toolkit cap holds own colour ≥ ~75%.
                assert!(own_count(&deck, Color::Ruby) >= 23, "colour anchor");
                assert!(deck.iter().all(|c| c.volatility <= 7 && c.points <= 3));
            }
        }
    }

    /// The spec scenario: a player picking ONLY toolkit and treasure buckets
    /// still receives a legal, colour-anchored pantry (commons fill the gap).
    #[test]
    fn toolkit_and_treasure_only_picks_still_anchor() {
        let reg = registry();
        let buckets = [
            PantryBucket::Saffron,
            PantryBucket::Ochre,
            PantryBucket::Wisp,
        ];
        for seed in 0..50u64 {
            let mut next_id = 0;
            let deck = realize_pantry(&reg, &buckets, Color::Emerald, &mut next_id, seed);
            assert_eq!(deck.len(), 30);
            assert!(toolkit_count(&deck, Color::Emerald) <= 7);
            assert!(treasure_count(&deck) <= 3);
            assert!(own_count(&deck, Color::Emerald) >= 23);
        }
    }

    /// Toolkit is optional: no toolkit bucket ⇒ a pure-Loyalist ~100%
    /// own-colour pantry (well within the anchor).
    #[test]
    fn no_toolkit_bucket_means_pure_own_color() {
        let reg = registry();
        let buckets = [
            PantryBucket::Sage,
            PantryBucket::Mint,
            PantryBucket::Bramble,
        ];
        let mut next_id = 0;
        let deck = realize_pantry(&reg, &buckets, Color::Sapphire, &mut next_id, 9);
        assert_eq!(toolkit_count(&deck, Color::Sapphire), 0);
        assert_eq!(own_count(&deck, Color::Sapphire), 30);
    }

    /// The spec scenario: both god-tier buckets still realize at most 2 god
    /// spells in a full 20-spell grimoire (the absolute Peek-economy cap).
    #[test]
    fn god_tier_stays_absolutely_capped() {
        let reg = registry();
        let buckets = [GrimoireBucket::Eyebright, GrimoireBucket::Ironbark];
        for seed in 0..50u64 {
            let mut next_id = 0;
            let deck = realize_grimoire(&reg, &buckets, &[], &[], &mut next_id, seed);
            assert_eq!(deck.len(), 20, "fixed size regardless of picks");
            assert!(god_count(&deck) <= 2, "god-tier cap is absolute");
        }
    }

    /// A reserve guarantees its named spell; the rest rolls within the picked
    /// buckets (or commons). A god-tier reserve counts against the cap.
    #[test]
    fn reserve_guarantees_the_named_spell() {
        let reg = registry();
        let buckets = [GrimoireBucket::Ironbark, GrimoireBucket::Brimstone];
        for seed in 0..50u64 {
            let mut next_id = 0;
            let deck = realize_grimoire(
                &reg,
                &buckets,
                &[SpellKind::Redirect],
                &[],
                &mut next_id,
                seed,
            );
            assert_eq!(deck.len(), 20);
            assert!(
                deck.iter().any(|s| s.kind == SpellKind::Redirect),
                "the reserved spell must be realized (seed {seed})"
            );
            assert!(god_count(&deck) <= 2);
        }
        // The Reservist's two locks both land.
        let mut next_id = 0;
        let deck = realize_grimoire(
            &reg,
            &buckets,
            &[SpellKind::Redirect, SpellKind::Hex],
            &[],
            &mut next_id,
            7,
        );
        assert!(deck.iter().any(|s| s.kind == SpellKind::Redirect));
        assert!(deck.iter().any(|s| s.kind == SpellKind::Hex));
    }

    /// Excluded kinds (the Cinderwright's wards) are never realized — not from
    /// buckets, reserves, or commons; the deck still reaches full size.
    #[test]
    fn excluded_spells_are_never_realized() {
        let reg = registry();
        let buckets = [GrimoireBucket::Hoarfrost, GrimoireBucket::Wormwood];
        for seed in 0..20u64 {
            let mut next_id = 0;
            let deck = realize_grimoire(&reg, &buckets, &[], &WARDS, &mut next_id, seed);
            assert_eq!(deck.len(), 20);
            assert!(deck.iter().all(|s| !WARDS.contains(&s.kind)));
        }
    }

    /// Same `(seed, recipe)` ⇒ the identical deck, regardless of bucket
    /// submission order; a different seed re-rolls it (the every-game novelty).
    #[test]
    fn realization_is_seeded_and_order_independent() {
        let reg = registry();
        let forward = [
            PantryBucket::Sage,
            PantryBucket::Nightshade,
            PantryBucket::Wisp,
        ];
        let backward = [
            PantryBucket::Wisp,
            PantryBucket::Nightshade,
            PantryBucket::Sage,
        ];
        let deal = |buckets: &[PantryBucket], seed: u64| {
            let mut next_id = 0;
            realize_pantry(&reg, buckets, Color::Ruby, &mut next_id, seed)
        };
        assert_eq!(deal(&forward, 42), deal(&backward, 42));
        assert_ne!(deal(&forward, 42), deal(&forward, 43), "re-rolled per game");

        let grim = |buckets: &[GrimoireBucket], seed: u64| {
            let mut next_id = 0;
            realize_grimoire(&reg, buckets, &[SpellKind::Peek], &[], &mut next_id, seed)
        };
        let fwd = [GrimoireBucket::Eyebright, GrimoireBucket::Mandrake];
        let bwd = [GrimoireBucket::Mandrake, GrimoireBucket::Eyebright];
        assert_eq!(grim(&fwd, 5), grim(&bwd, 5));
    }

    /// Bucket count trades focus for breadth only: 2 and 3 buckets both fill
    /// the same fixed size, the 2-bucket deck drawing from fewer archetypes.
    #[test]
    fn bucket_count_changes_focus_not_size() {
        let reg = registry();
        let mut next_id = 0;
        let two = realize_pantry(
            &reg,
            &[PantryBucket::Sage, PantryBucket::Nightshade],
            Color::Ruby,
            &mut next_id,
            3,
        );
        let three = realize_pantry(
            &reg,
            &[
                PantryBucket::Sage,
                PantryBucket::Nightshade,
                PantryBucket::Chalk,
            ],
            Color::Ruby,
            &mut next_id,
            3,
        );
        assert_eq!(two.len(), three.len());
        let archetypes = |deck: &[Ingredient]| {
            let mut kinds: Vec<(u8, u8)> = deck.iter().map(|c| (c.volatility, c.points)).collect();
            kinds.sort();
            kinds.dedup();
            kinds.len()
        };
        assert!(
            archetypes(&two) <= archetypes(&three),
            "more buckets ⇒ at least as much variety"
        );
    }
}
