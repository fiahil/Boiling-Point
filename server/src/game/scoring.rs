//! Scoring and explosion: dominance by total colour points, winner-takes-all
//! with Alliance/Commune splits (round down), and shared-loss explosions — all
//! honouring the active modifiers (Bountiful additive, Double Stakes multiplier,
//! Reversal flip) and Shield (immunity / safe-resolution forfeit).

use std::collections::{HashMap, HashSet};

use boiling_point_protocol::vocab::Color;
use boiling_point_protocol::PlayerId;

use crate::content::ContentRegistry;

use super::modifiers::ActiveModifiers;
use super::pot::Pot;

/// Everything scoring needs that lives outside the pot.
pub struct ScoringContext<'a> {
    /// Active cauldron modifiers for the round.
    pub modifiers: &'a ActiveModifiers,
    /// Content registry (for modifier offsets/multipliers).
    pub registry: &'a ContentRegistry,
    /// Which player owns each player colour.
    pub color_owner: &'a HashMap<Color, PlayerId>,
    /// Players who played Shield this round.
    pub shielded: &'a HashSet<PlayerId>,
    /// All players at the table.
    pub all_players: &'a [PlayerId],
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
    /// Net points awarded to each player this round.
    pub awards: HashMap<PlayerId, i32>,
}

/// Result of an exploded pot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplosionResult {
    /// The pot value every non-shielded player loses.
    pub pot_value: u32,
    /// Per-player score delta (negative loss, or 0 if shielded).
    pub deltas: HashMap<PlayerId, i32>,
    /// Players who were shielded and took no loss.
    pub shielded: Vec<PlayerId>,
}

/// The pot's scored value: (sum of card points + Bountiful per-card bonus) ×
/// Double Stakes multiplier. Used identically for a win payout and a blast loss.
pub fn pot_value(pot: &Pot, ctx: &ScoringContext) -> u32 {
    let additive =
        pot.base_points() + ctx.modifiers.pot_bonus_per_card(ctx.registry) * pot.card_count();
    additive * ctx.modifiers.pot_multiplier(ctx.registry)
}

/// Score a safely-resolved pot: decide dominance on per-colour totals first
/// (Reversal flips to the lowest present colour; Bountiful's colourless bonus is
/// excluded from this step), then award the pot value, splitting on ties.
pub fn score_safe(pot: &Pot, ctx: &ScoringContext) -> SafeScore {
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

    // Split the pot equally among winning colours, rounding down.
    let share = (value / winners.len() as u32) as i32;
    for color in &winners {
        if let Some(owner) = ctx.color_owner.get(color) {
            let contributed = pot.cards.iter().any(|pc| pc.player == *owner);
            // Absent (0 cards) or shielded → forfeit the share.
            if contributed && !ctx.shielded.contains(owner) {
                *awards.entry(*owner).or_insert(0) += share;
            }
        }
    }

    SafeScore {
        color_points: present,
        winners,
        pot_value: value,
        awards,
    }
}

/// Compute the shared-loss explosion: every non-shielded player loses the full
/// pot value; shielded players lose nothing.
pub fn explosion(pot: &Pot, ctx: &ScoringContext) -> ExplosionResult {
    let value = pot_value(pot, ctx);
    let mut deltas = HashMap::new();
    let mut shielded = Vec::new();
    for &player in ctx.all_players {
        if ctx.shielded.contains(&player) {
            deltas.insert(player, 0);
            shielded.push(player);
        } else {
            deltas.insert(player, -(value as i32));
        }
    }
    ExplosionResult {
        pot_value: value,
        deltas,
        shielded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::Card;
    use crate::game::pot::PotCard;
    use boiling_point_protocol::vocab::ModifierKind;
    use boiling_point_protocol::CardId;
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

    /// Four players, one per colour; returns (players, color_owner).
    fn players() -> (Vec<PlayerId>, HashMap<Color, PlayerId>) {
        let ps: Vec<PlayerId> = (1..=4).map(pid).collect();
        let owner = Color::PLAYER_COLORS
            .into_iter()
            .zip(ps.iter().copied())
            .collect();
        (ps, owner)
    }

    fn card(color: Color, points: u8) -> Card {
        Card {
            id: CardId(0),
            color,
            volatility: 1,
            points,
            effect: None,
        }
    }

    fn pot_with(cards: &[(PlayerId, Color, u8)]) -> Pot {
        let mut pot = Pot::new(0);
        for &(player, color, points) in cards {
            pot.cards.push(PotCard {
                player,
                card: card(color, points),
                color,
                points,
            });
        }
        pot
    }

    fn ctx<'a>(
        mods: &'a ActiveModifiers,
        reg: &'a ContentRegistry,
        owner: &'a HashMap<Color, PlayerId>,
        shielded: &'a HashSet<PlayerId>,
        all: &'a [PlayerId],
    ) -> ScoringContext<'a> {
        ScoringContext {
            modifiers: mods,
            registry: reg,
            color_owner: owner,
            shielded,
            all_players: all,
        }
    }

    #[test]
    fn sole_dominant_takes_all() {
        let reg = registry();
        let (all, owner) = players();
        let mods = ActiveModifiers::new();
        let shielded = HashSet::new();
        // Ruby 6 (two 3s), Sapphire 3.
        let pot = pot_with(&[
            (all[0], Color::Ruby, 3),
            (all[0], Color::Ruby, 3),
            (all[1], Color::Sapphire, 3),
        ]);
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &shielded, &all));
        assert_eq!(s.winners, vec![Color::Ruby]);
        assert_eq!(s.pot_value, 9);
        assert_eq!(s.awards[&all[0]], 9);
        assert_eq!(s.awards[&all[1]], 0);
    }

    #[test]
    fn two_way_tie_splits_round_down() {
        let reg = registry();
        let (all, owner) = players();
        let mods = ActiveModifiers::new();
        let shielded = HashSet::new();
        // Ruby 3, Sapphire 3, plus a wild worth 1 → pot value 7, split 3 each.
        let pot = pot_with(&[
            (all[0], Color::Ruby, 3),
            (all[1], Color::Sapphire, 3),
            (all[2], Color::Wild, 1),
        ]);
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &shielded, &all));
        assert_eq!(s.pot_value, 7);
        assert_eq!(s.awards[&all[0]], 3);
        assert_eq!(s.awards[&all[1]], 3);
        // Leftover point evaporates; wild owner contributed but isn't a winner.
        assert_eq!(s.awards[&all[2]], 0);
    }

    #[test]
    fn reversal_picks_lowest_present_colour() {
        let reg = registry();
        let (all, owner) = players();
        let mut mods = ActiveModifiers::new();
        mods.push(ModifierKind::Reversal);
        let shielded = HashSet::new();
        let pot = pot_with(&[(all[0], Color::Ruby, 6), (all[1], Color::Sapphire, 3)]);
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &shielded, &all));
        assert_eq!(s.winners, vec![Color::Sapphire]);
        assert_eq!(s.awards[&all[1]], 9);
    }

    #[test]
    fn bountiful_then_double_stakes_compose() {
        let reg = registry();
        let (all, owner) = players();
        let mut mods = ActiveModifiers::new();
        mods.push(ModifierKind::BountifulBrew);
        mods.push(ModifierKind::DoubleStakes);
        let shielded = HashSet::new();
        // Two cards: Ruby 3, Sapphire 1. base=4, +1/card*2cards=+2 → 6, ×2 → 12.
        let pot = pot_with(&[(all[0], Color::Ruby, 3), (all[1], Color::Sapphire, 1)]);
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &shielded, &all));
        assert_eq!(s.pot_value, 12);
        assert_eq!(s.winners, vec![Color::Ruby]);
        assert_eq!(s.awards[&all[0]], 12);
    }

    #[test]
    fn explosion_hits_everyone_but_shielded() {
        let reg = registry();
        let (all, owner) = players();
        let mods = ActiveModifiers::new();
        let mut shielded = HashSet::new();
        shielded.insert(all[2]);
        let pot = pot_with(&[(all[0], Color::Ruby, 5), (all[1], Color::Sapphire, 4)]);
        let e = explosion(&pot, &ctx(&mods, &reg, &owner, &shielded, &all));
        assert_eq!(e.pot_value, 9);
        assert_eq!(e.deltas[&all[0]], -9);
        assert_eq!(e.deltas[&all[1]], -9);
        assert_eq!(e.deltas[&all[2]], 0); // shielded
        assert_eq!(e.deltas[&all[3]], -9); // spectator still loses
    }

    #[test]
    fn shielded_winner_forfeits_on_safe_brew() {
        let reg = registry();
        let (all, owner) = players();
        let mods = ActiveModifiers::new();
        let mut shielded = HashSet::new();
        shielded.insert(all[0]); // Ruby owner shielded
        let pot = pot_with(&[(all[0], Color::Ruby, 6), (all[1], Color::Sapphire, 3)]);
        let s = score_safe(&pot, &ctx(&mods, &reg, &owner, &shielded, &all));
        assert_eq!(s.winners, vec![Color::Ruby]);
        assert_eq!(s.awards[&all[0]], 0); // forfeited despite winning
    }
}
