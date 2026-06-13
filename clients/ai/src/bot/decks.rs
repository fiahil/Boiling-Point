//! Named deck archetypes — the harness's **deck-archetype** axis
//! (`boom2-apothecary`, Principle IV).
//!
//! An archetype is a *planned recipe*: the experimental variable a sample spec
//! pins per seat. Like the Brewer preference, it rides the bot brain rather
//! than a host script: the bot **legalizes** the plan against its own draft
//! frame (a Cinderwright seat is never offered Ironbark, so a fixed scripted
//! recipe would go illegal exactly when the matrix gets interesting) and
//! falls back to the frame's suggested quick-pick if no legal adaptation
//! exists. The four presets are the design doc's legible table reads
//! (docs/06_boom2/02 §D4) plus the pure Loyalist; contents
//! `[needs playtesting]`.

use rand::Rng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use boiling_point_protocol::frame::PendingDecision;
use boiling_point_protocol::vocab::{GrimoireBucket, PantryBucket, Recipe, SpellKind};

/// A named, pre-planned recipe — one cell coordinate on the deck-archetype axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckArchetype {
    /// The aggressive bruiser: hot own-colour weapons and treasure, wards to
    /// survive its own heat (Nightshade+Saffron+Bilberry / Ironbark+Brimstone,
    /// Redirect reserved).
    Warlord,
    /// The survivor: tiptoe cards and cooling, certainty via a reserved Cap
    /// (Sage+Hellebore+Chalk / Hoarfrost+Ironbark+Farsight).
    Fortress,
    /// The political player: off-colour and wild denial, information and
    /// disruption (Ochre+Wisp+Mint / Farsight+Wormwood+Goldenseal).
    Kingmaker,
    /// The pure Loyalist: no toolkit bucket at all — ~100% own colour — with
    /// the Peek economy and cash-in (Sage+Mint+Nightshade /
    /// Eyebright+Goldenseal+Mandrake, Peek reserved).
    Loyalist,
}

impl DeckArchetype {
    /// Every archetype, in a stable order.
    pub const ALL: [DeckArchetype; 4] = [
        DeckArchetype::Warlord,
        DeckArchetype::Fortress,
        DeckArchetype::Kingmaker,
        DeckArchetype::Loyalist,
    ];

    /// The stable name sample specs and reports use.
    pub fn name(self) -> &'static str {
        match self {
            DeckArchetype::Warlord => "warlord",
            DeckArchetype::Fortress => "fortress",
            DeckArchetype::Kingmaker => "kingmaker",
            DeckArchetype::Loyalist => "loyalist",
        }
    }

    /// Parse a name back into an archetype.
    pub fn by_name(name: &str) -> Option<DeckArchetype> {
        DeckArchetype::ALL.into_iter().find(|a| a.name() == name)
    }

    /// The archetype's planned recipe (legalized per frame before submission).
    pub fn recipe(self) -> Recipe {
        match self {
            DeckArchetype::Warlord => Recipe {
                pantry: vec![
                    PantryBucket::Nightshade,
                    PantryBucket::Saffron,
                    PantryBucket::Bilberry,
                ],
                grimoire: vec![GrimoireBucket::Ironbark, GrimoireBucket::Brimstone],
                reserves: vec![SpellKind::Redirect],
            },
            DeckArchetype::Fortress => Recipe {
                pantry: vec![
                    PantryBucket::Sage,
                    PantryBucket::Hellebore,
                    PantryBucket::Chalk,
                ],
                grimoire: vec![
                    GrimoireBucket::Hoarfrost,
                    GrimoireBucket::Ironbark,
                    GrimoireBucket::Farsight,
                ],
                reserves: vec![SpellKind::Cap],
            },
            DeckArchetype::Kingmaker => Recipe {
                pantry: vec![PantryBucket::Ochre, PantryBucket::Wisp, PantryBucket::Mint],
                grimoire: vec![
                    GrimoireBucket::Farsight,
                    GrimoireBucket::Wormwood,
                    GrimoireBucket::Goldenseal,
                ],
                reserves: vec![SpellKind::Sour],
            },
            DeckArchetype::Loyalist => Recipe {
                pantry: vec![
                    PantryBucket::Sage,
                    PantryBucket::Mint,
                    PantryBucket::Nightshade,
                ],
                grimoire: vec![
                    GrimoireBucket::Eyebright,
                    GrimoireBucket::Goldenseal,
                    GrimoireBucket::Mandrake,
                ],
                reserves: vec![SpellKind::Peek],
            },
        }
    }
}

/// Adapt a planned recipe to a seat's draft frame: keep only offered buckets,
/// top short ledgers up from the frame's suggested quick-pick (then the
/// remaining options, in offer order), trim over-allowance ledgers, and keep
/// only the reserves the final grimoire justifies within the allowance.
/// Returns `None` for a non-draft frame.
pub fn legalize(plan: &Recipe, decision: &PendingDecision) -> Option<Recipe> {
    let PendingDecision::ApothecaryDraft {
        pantry_options,
        grimoire_options,
        picks_min,
        picks_max,
        reserves_max,
        suggested,
        ..
    } = decision
    else {
        return None;
    };
    let (min, max) = (*picks_min as usize, *picks_max as usize);

    fn fit<B: Copy + PartialEq>(
        planned: &[B],
        offered: &[B],
        preferred_fill: &[B],
        min: usize,
        max: usize,
    ) -> Vec<B> {
        let mut picked: Vec<B> = Vec::new();
        for b in planned {
            if picked.len() >= max {
                break;
            }
            if offered.contains(b) && !picked.contains(b) {
                picked.push(*b);
            }
        }
        // A short ledger tops up to the minimum only (stay close to the plan).
        for b in preferred_fill.iter().chain(offered) {
            if picked.len() >= min {
                break;
            }
            if offered.contains(b) && !picked.contains(b) {
                picked.push(*b);
            }
        }
        picked
    }

    let pantry = fit(&plan.pantry, pantry_options, &suggested.pantry, min, max);
    let grimoire = fit(
        &plan.grimoire,
        grimoire_options,
        &suggested.grimoire,
        min,
        max,
    );
    let reserves: Vec<SpellKind> = plan
        .reserves
        .iter()
        .copied()
        .filter(|k| grimoire.iter().any(|b| b.spells().contains(k)))
        .take(*reserves_max as usize)
        .collect();
    Some(Recipe {
        pantry,
        grimoire,
        reserves,
    })
}

/// A uniformly random legal recipe drawn from the frame (the Random baseline's
/// draft answer): a seeded shuffle of each roster, a uniform ledger size in
/// `min..=max`, and a coin-flip reserve from the picked role-groups.
pub fn random_recipe(decision: &PendingDecision, rng: &mut StdRng) -> Option<Recipe> {
    let PendingDecision::ApothecaryDraft {
        pantry_options,
        grimoire_options,
        picks_min,
        picks_max,
        reserves_max,
        ..
    } = decision
    else {
        return None;
    };
    let (min, max) = (*picks_min as usize, *picks_max as usize);
    let mut pantry = pantry_options.clone();
    pantry.shuffle(rng);
    pantry.truncate(rng.gen_range(min..=max.min(pantry.len())));
    let mut grimoire = grimoire_options.clone();
    grimoire.shuffle(rng);
    grimoire.truncate(rng.gen_range(min..=max.min(grimoire.len())));
    let mut reserves = Vec::new();
    if *reserves_max > 0 && rng.gen_bool(0.5) {
        let family: Vec<SpellKind> = grimoire
            .iter()
            .flat_map(|b| b.spells().iter().copied())
            .collect();
        if !family.is_empty() {
            reserves.push(family[rng.gen_range(0..family.len())]);
        }
    }
    Some(Recipe {
        pantry,
        grimoire,
        reserves,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft_frame(grimoire_options: Vec<GrimoireBucket>) -> PendingDecision {
        PendingDecision::ApothecaryDraft {
            pantry_options: PantryBucket::ALL.to_vec(),
            grimoire_options,
            picks_min: 2,
            picks_max: 3,
            bonus_buckets: 0,
            reserves_max: 1,
            suggested: Recipe {
                pantry: vec![PantryBucket::Sage, PantryBucket::Mint, PantryBucket::Honey],
                grimoire: vec![
                    GrimoireBucket::Farsight,
                    GrimoireBucket::Hoarfrost,
                    GrimoireBucket::Mandrake,
                ],
                reserves: vec![],
            },
        }
    }

    /// Archetype metadata is total, and every preset is legal as planned on a
    /// full-roster frame.
    #[test]
    fn presets_are_named_and_legal_on_a_full_roster() {
        let frame = draft_frame(GrimoireBucket::ALL.to_vec());
        for archetype in DeckArchetype::ALL {
            assert_eq!(DeckArchetype::by_name(archetype.name()), Some(archetype));
            let plan = archetype.recipe();
            let adapted = legalize(&plan, &frame).expect("a draft frame");
            assert_eq!(adapted, plan, "{archetype:?} should pass through intact");
            assert!(frame.permits_recipe(&adapted), "{archetype:?}");
        }
    }

    /// The Cinderwright adaptation: with Ironbark off the menu, Ironbark plans
    /// refit to a legal recipe and drop ward reserves.
    #[test]
    fn ironbark_plans_legalize_for_the_cinderwright() {
        let no_ironbark: Vec<GrimoireBucket> = GrimoireBucket::ALL
            .into_iter()
            .filter(|b| *b != GrimoireBucket::Ironbark)
            .collect();
        let frame = draft_frame(no_ironbark);
        for archetype in [DeckArchetype::Warlord, DeckArchetype::Fortress] {
            let adapted = legalize(&archetype.recipe(), &frame).expect("a draft frame");
            assert!(
                frame.permits_recipe(&adapted),
                "{archetype:?} must adapt to a legal recipe: {adapted:?}"
            );
            assert!(!adapted.grimoire.contains(&GrimoireBucket::Ironbark));
            assert!(
                adapted
                    .reserves
                    .iter()
                    .all(|k| !matches!(k, SpellKind::Cap | SpellKind::Halve | SpellKind::Redirect)),
                "ward reserves drop with the bucket"
            );
        }
    }

    /// Random recipes are always legal and vary across draws.
    #[test]
    fn random_recipes_are_legal() {
        use rand::SeedableRng;
        let frame = draft_frame(GrimoireBucket::ALL.to_vec());
        let mut rng = StdRng::seed_from_u64(11);
        let mut distinct = std::collections::HashSet::new();
        for _ in 0..100 {
            let recipe = random_recipe(&frame, &mut rng).expect("a draft frame");
            assert!(frame.permits_recipe(&recipe), "illegal: {recipe:?}");
            distinct.insert(format!("{recipe:?}"));
        }
        assert!(distinct.len() > 10, "the baseline must sample the space");
    }
}
