//! Scoring: the P-symmetry. Every pot is worth `P = Σ colored Vote points`. A
//! safe brew pays the dominant colour **+P** (winner-takes-all, ties split
//! rounded down, integer-only); an explosion makes the **detonator(s)** split
//! **−P** (modified by wards, plus Hex extras) and awards no colour. A player
//! who contributed nothing scores 0 either way.
//!
//! Cauldron modifiers still compose onto the pot's value (Bountiful additive,
//! Double Stakes multiplier, Reversal flip); their magnitudes are v1-scaled and
//! flagged for a follow-up rescale.

use std::collections::{HashMap, HashSet};

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::server::SpellFire;
use boiling_point_protocol::vocab::{Brewer, Color};

use crate::content::registry::ContentRegistry;

use super::modifiers::ActiveModifiers;
use super::pot::Pot;
use super::spells::{PrimedSpell, resolve_explosion, resolve_harvests};

/// Everything scoring needs that lives outside the pot.
pub struct ScoringContext<'a> {
    /// Active cauldron modifiers for the round.
    pub modifiers: &'a ActiveModifiers,
    /// Content registry (for modifier offsets/multipliers and spell magnitudes).
    pub registry: &'a ContentRegistry,
    /// Which player owns each player colour.
    pub color_owner: &'a HashMap<Color, PlayerId>,
    /// Each player's colour (the Harvest check).
    pub player_color: &'a HashMap<PlayerId, Color>,
    /// All players at the table.
    pub all_players: &'a [PlayerId],
    /// Each player's public Brewer (`boom2-brewers`) — the scoring bends
    /// (Broker's round-up split, Cinderwright's half detonator damage) key off
    /// it. Empty when no brewer phase ran (e.g. the sync test runner).
    pub brewers: &'a HashMap<PlayerId, Brewer>,
}

/// Result of scoring a safely-resolved pot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeScore {
    /// Per-colour point totals (player colours present), for display.
    pub color_points: Vec<(Color, u32)>,
    /// The winning colour(s) — more than one means a split.
    pub winners: Vec<Color>,
    /// The pot's scored value.
    pub pot_value: u32,
    /// Net points awarded to each player this round (incl. Harvest bonuses).
    pub awards: HashMap<PlayerId, i32>,
    /// Harvests that fired, narrated in fire order.
    pub fired: Vec<SpellFire>,
}

/// Result of an exploded pot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplosionResult {
    /// The pot value the detonators split.
    pub pot_value: u32,
    /// The liable players, in fatal-wave sort order.
    pub detonators: Vec<PlayerId>,
    /// Per-player score delta (zero for the unaffected).
    pub deltas: HashMap<PlayerId, i32>,
    /// Wards and Hexes that fired, narrated in fire order.
    pub fired: Vec<SpellFire>,
}

/// The pot's scored value: (P + Bountiful per-card bonus) × Double Stakes
/// multiplier. Used identically for a win payout and a detonator loss.
pub fn pot_value(pot: &Pot, ctx: &ScoringContext) -> u32 {
    let additive =
        pot.vote_points() + ctx.modifiers.pot_bonus_per_card(ctx.registry) * pot.card_count();
    additive * ctx.modifiers.pot_multiplier(ctx.registry)
}

/// Score a safely-resolved pot: decide dominance on per-colour Vote totals
/// (Reversal flips to the lowest present colour), award the pot value to the
/// strictly-highest colour — splitting on ties, rounded down — then fire any
/// winning Harvests.
pub fn score_safe(pot: &Pot, ctx: &ScoringContext, primed: &mut [PrimedSpell]) -> SafeScore {
    // Per-colour totals for player colours actually present in the pot.
    let present: Vec<(Color, u32)> = Color::PLAYER_COLORS
        .into_iter()
        .filter(|c| pot.color_present(*c))
        .map(|c| (c, pot.color_points(c)))
        .collect();

    let value = pot_value(pot, ctx);

    let mut awards: HashMap<PlayerId, i32> = ctx.all_players.iter().map(|p| (*p, 0)).collect();

    // No player colour present → nobody wins; the pot is unawarded.
    if present.is_empty() {
        return SafeScore {
            color_points: present,
            winners: Vec::new(),
            pot_value: value,
            awards,
            fired: Vec::new(),
        };
    }

    let reversed = ctx.modifiers.reversed(ctx.registry);
    let extremum = if reversed {
        present.iter().map(|(_, p)| *p).min().unwrap()
    } else {
        present.iter().map(|(_, p)| *p).max().unwrap()
    };
    let winners: Vec<Color> = present
        .iter()
        .filter(|(_, p)| *p == extremum)
        .map(|(c, _)| *c)
        .collect();

    // Split the pot equally among winning colours, rounding down (integer-only)
    // — except for a Broker, whose share rounds up (`boom2-brewers`): everyone's
    // preferred ally. A sole winner's exact division is unaffected.
    let split = winners.len() as u32;
    for color in &winners {
        if let Some(owner) = ctx.color_owner.get(color) {
            // A player who contributed nothing scores 0 either way (P-symmetry).
            let contributed = pot.cards.iter().any(|pc| pc.player == *owner);
            if contributed {
                let share = if ctx.brewers.get(owner) == Some(&Brewer::Broker) {
                    value.div_ceil(split)
                } else {
                    value / split
                };
                *awards.entry(*owner).or_insert(0) += share as i32;
            }
        }
    }

    // Winning Harvests cash in on top of the take.
    let (bonuses, fired) = resolve_harvests(
        &winners,
        &awards,
        ctx.player_color,
        ctx.registry.spell_values(),
        primed,
    );
    for (player, bonus) in bonuses {
        *awards.entry(player).or_insert(0) += bonus;
    }

    SafeScore {
        color_points: present,
        winners,
        pot_value: value,
        awards,
        fired,
    }
}

/// Compute the detonator-only explosion: the liable players split −P (rounded
/// down), each loss modified by their wards (Redirect cascading) — a
/// Cinderwright detonator takes half, rounded up (`boom2-brewers`); every
/// primed Hex then lands its extra on its target. Non-liable, un-hexed players
/// lose nothing.
pub fn explosion(
    pot: &Pot,
    ctx: &ScoringContext,
    detonators: Vec<PlayerId>,
    primed: &mut [PrimedSpell],
) -> ExplosionResult {
    let value = pot_value(pot, ctx);
    let cinderwrights: HashSet<PlayerId> = ctx
        .brewers
        .iter()
        .filter(|(_, b)| **b == Brewer::Cinderwright)
        .map(|(p, _)| *p)
        .collect();
    let (mut deltas, fired) = resolve_explosion(
        &detonators,
        value,
        ctx.registry.spell_values(),
        primed,
        &cinderwrights,
    );
    // Every player gets an explicit (possibly zero) delta for a stable wire shape.
    for player in ctx.all_players {
        deltas.entry(*player).or_insert(0);
    }
    ExplosionResult {
        pot_value: value,
        detonators,
        deltas,
        fired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::{Ingredient, Spell};
    use crate::game::pot::PotIngredient;
    use boiling_point_protocol::CardId;
    use boiling_point_protocol::vocab::{ModifierKind, SpellKind};
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn registry() -> ContentRegistry {
        crate::config::ContentConfig::from_toml(include_str!("../../content.toml"))
            .unwrap()
            .build_registry()
            .unwrap()
    }

    /// Four players, one per colour; returns (players, color_owner, player_color).
    fn players() -> (
        Vec<PlayerId>,
        HashMap<Color, PlayerId>,
        HashMap<PlayerId, Color>,
    ) {
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        let owner: HashMap<Color, PlayerId> = Color::PLAYER_COLORS
            .into_iter()
            .zip(ps.iter().copied())
            .collect();
        let player_color: HashMap<PlayerId, Color> = owner.iter().map(|(c, p)| (*p, *c)).collect();
        (ps, owner, player_color)
    }

    fn vote(player: PlayerId, color: Color, points: u8) -> PotIngredient {
        PotIngredient {
            player,
            ingredient: Ingredient {
                id: CardId(0),
                color,
                volatility: 1,
                points,
            },
            colorless: false,
            wave_number: 1,
            exposed: false,
        }
    }

    fn pot_with(cards: &[(PlayerId, Color, u8)]) -> Pot {
        let mut pot = Pot::new(0);
        for &(player, color, points) in cards {
            pot.cards.push(vote(player, color, points));
        }
        pot
    }

    /// The shared brewerless default for tests that don't exercise a bend.
    fn no_brewers() -> &'static HashMap<PlayerId, Brewer> {
        static EMPTY: std::sync::LazyLock<HashMap<PlayerId, Brewer>> =
            std::sync::LazyLock::new(HashMap::new);
        &EMPTY
    }

    fn ctx<'a>(
        mods: &'a ActiveModifiers,
        reg: &'a ContentRegistry,
        owner: &'a HashMap<Color, PlayerId>,
        player_color: &'a HashMap<PlayerId, Color>,
        all: &'a [PlayerId],
    ) -> ScoringContext<'a> {
        ScoringContext {
            modifiers: mods,
            registry: reg,
            color_owner: owner,
            player_color,
            brewers: no_brewers(),
            all_players: all,
        }
    }

    #[test]
    fn sole_dominant_takes_all() {
        let reg = registry();
        let (all, owner, pc) = players();
        let mods = ActiveModifiers::new();
        let pot = pot_with(&[
            (all[0], Color::Ruby, 3),
            (all[0], Color::Ruby, 3),
            (all[1], Color::Sapphire, 3),
        ]);
        let mut primed = Vec::new();
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &pc, &all), &mut primed);
        assert_eq!(s.winners, vec![Color::Ruby]);
        assert_eq!(s.pot_value, 9);
        assert_eq!(s.awards[&all[0]], 9);
        assert_eq!(s.awards[&all[1]], 0);
    }

    #[test]
    fn two_way_tie_splits_round_down() {
        let reg = registry();
        let (all, owner, pc) = players();
        let mods = ActiveModifiers::new();
        // Ruby 3, Sapphire 3, plus a third vote worth 1 → pot value 7, split 3 each.
        let pot = pot_with(&[
            (all[0], Color::Ruby, 3),
            (all[1], Color::Sapphire, 3),
            (all[2], Color::Emerald, 1),
        ]);
        let mut primed = Vec::new();
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &pc, &all), &mut primed);
        assert_eq!(s.pot_value, 7);
        assert_eq!(s.awards[&all[0]], 3);
        assert_eq!(s.awards[&all[1]], 3);
        // Leftover point evaporates; the third player isn't a winner.
        assert_eq!(s.awards[&all[2]], 0);
    }

    /// Wilds and colorless plays swell nothing: P counts colored Votes only.
    #[test]
    fn pot_value_counts_colored_votes_only() {
        let reg = registry();
        let (all, owner, pc) = players();
        let mods = ActiveModifiers::new();
        let mut pot = pot_with(&[(all[0], Color::Ruby, 2)]);
        // A wild with printed points and a colorless play with printed points.
        pot.cards.push(vote(all[1], Color::Wild, 3));
        pot.cards.push(PotIngredient {
            colorless: true,
            ..vote(all[2], Color::Emerald, 3)
        });
        let s_ctx = ctx(&mods, &reg, &owner, &pc, &all);
        assert_eq!(pot_value(&pot, &s_ctx), 2);
    }

    #[test]
    fn reversal_picks_lowest_present_colour() {
        let reg = registry();
        let (all, owner, pc) = players();
        let mut mods = ActiveModifiers::new();
        mods.push(ModifierKind::Reversal);
        let pot = pot_with(&[(all[0], Color::Ruby, 6), (all[1], Color::Sapphire, 3)]);
        let mut primed = Vec::new();
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &pc, &all), &mut primed);
        assert_eq!(s.winners, vec![Color::Sapphire]);
        assert_eq!(s.awards[&all[1]], 9);
    }

    #[test]
    fn bountiful_then_double_stakes_compose() {
        let reg = registry();
        let (all, owner, pc) = players();
        let mut mods = ActiveModifiers::new();
        mods.push(ModifierKind::BountifulBrew);
        mods.push(ModifierKind::DoubleStakes);
        // Two votes: Ruby 3, Sapphire 1. base=4, +1/card*2cards=+2 → 6, ×2 → 12.
        let pot = pot_with(&[(all[0], Color::Ruby, 3), (all[1], Color::Sapphire, 1)]);
        let mut primed = Vec::new();
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &pc, &all), &mut primed);
        assert_eq!(s.pot_value, 12);
        assert_eq!(s.winners, vec![Color::Ruby]);
        assert_eq!(s.awards[&all[0]], 12);
    }

    /// The Broker bend (`boom2-brewers`): on a split pot the Broker's share
    /// rounds up; the other winner still rounds down (everyone's preferred
    /// ally — they cost the pot, not their partner).
    #[test]
    fn broker_rounds_up_on_a_split() {
        let reg = registry();
        let (all, owner, pc) = players();
        let mods = ActiveModifiers::new();
        // Ruby 3, Sapphire 3, plus a third vote worth 1 → pot 7: a 3/3 split.
        let pot = pot_with(&[
            (all[0], Color::Ruby, 3),
            (all[1], Color::Sapphire, 3),
            (all[2], Color::Emerald, 1),
        ]);
        let brewers: HashMap<PlayerId, Brewer> = [(all[0], Brewer::Broker)].into();
        let ctx = ScoringContext {
            modifiers: &mods,
            registry: &reg,
            color_owner: &owner,
            player_color: &pc,
            all_players: &all,
            brewers: &brewers,
        };
        let mut primed = Vec::new();
        let s = score_safe(&pot, &ctx, &mut primed);
        assert_eq!(s.awards[&all[0]], 4, "the Broker rounds up");
        assert_eq!(s.awards[&all[1]], 3, "the partner still rounds down");
        // A sole win divides exactly: the bend changes nothing.
        let solo = pot_with(&[(all[0], Color::Ruby, 5)]);
        let s = score_safe(&solo, &ctx, &mut primed);
        assert_eq!(s.awards[&all[0]], 5);
    }

    /// The P-symmetry: an explosion costs only the detonators, who split −P.
    #[test]
    fn explosion_costs_only_the_detonators() {
        let reg = registry();
        let (all, owner, pc) = players();
        let mods = ActiveModifiers::new();
        let pot = pot_with(&[
            (all[0], Color::Ruby, 5),
            (all[1], Color::Sapphire, 4),
            (all[2], Color::Emerald, 0),
        ]);
        let mut primed = Vec::new();
        let e = explosion(
            &pot,
            &ctx(&mods, &reg, &owner, &pc, &all),
            vec![all[0], all[2]],
            &mut primed,
        );
        assert_eq!(e.pot_value, 9);
        assert_eq!(e.deltas[&all[0]], -4); // 9 / 2 rounded down
        assert_eq!(e.deltas[&all[2]], -4);
        assert_eq!(e.deltas[&all[1]], 0, "non-detonators lose nothing");
        assert_eq!(e.deltas[&all[3]], 0, "spectators lose nothing");
    }

    /// A winning Harvest cashes in on top of the take.
    #[test]
    fn winning_harvest_pays_its_bonus() {
        let reg = registry();
        let (all, owner, pc) = players();
        let mods = ActiveModifiers::new();
        let pot = pot_with(&[(all[0], Color::Ruby, 5)]);
        let mut primed = vec![PrimedSpell {
            player: all[0],
            spell: Spell {
                id: CardId(50),
                kind: SpellKind::Harvest,
            },
            target: None,
            fired: false,
        }];
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &pc, &all), &mut primed);
        assert_eq!(s.awards[&all[0]], 5 + 3);
        assert_eq!(s.fired.len(), 1);
    }
}
