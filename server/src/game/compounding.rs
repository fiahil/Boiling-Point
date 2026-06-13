//! In-pot ingredient interactions (change `boom2-compounding`): the legible
//! classes of compounding that give the Bramble/Honey buckets and the three
//! compounding Brewers their teeth.
//!
//! Two classes ship here:
//! - **Count-threshold (Honey).** A card scores `per_card` extra points for each
//!   card in the pot past the `past`-th. The trigger is the **public** card
//!   count, so it is plannable; a Distiller treats the pot as
//!   [`DISTILLER_POT_BONUS`] cards larger.
//! - **Named-combo (Bramble).** A pair pays a flat [`COMBO_BONUS_POINTS`] when
//!   **both** halves are in the pot — one half for a Herbalist. Combos are
//!   *bonuses, never requirements*: a lone half scores nothing extra and is
//!   penalised nothing (it is a plain ingredient). An Alchemist's fired combo
//!   also adds [`ALCHEMIST_COMBO_VOLATILITY`] to the completing card, flowing
//!   through the **same** effective volatility the detonator sort and the
//!   explosion check already use ([`super::pot::PotIngredient::effective_volatility`]).
//!
//! Knowability is the design axis (docs/06_boom2/02 §O3): both shipped classes
//! are knowable (a public count; a recipe you drafted). The hidden,
//! snowball-prone **colour-synergy** class is deliberately **not** authored in
//! this change — its cap is structural (no such ingredient exists). If it is
//! added later it MUST be capped or Peek-gated (spec `boom-compounding`).
//!
//! Every magnitude here is `[needs playtesting]` (Principle IV); the harness
//! measures snowball, lone-combo-half (dead-draw) rate, and threshold payoff.

use std::collections::HashMap;

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::server::CompoundingFire;
use boiling_point_protocol::vocab::{Brewer, ComboHalf, ComboPair, Compounding};

use super::brewers::{ALCHEMIST_COMBO_VOLATILITY, DISTILLER_POT_BONUS};
use super::pot::PotIngredient;

/// Points a fully-formed named combo pays its colour. `[needs playtesting]`
/// (docs/06_boom2/02: "+2 when paired").
pub const COMBO_BONUS_POINTS: u8 = 2;

/// The compounding bonus a single pot card carries, recomputed whenever the pot
/// changes. Default is "no bonus" — a plain card or a lone combo half.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CardCompounding {
    /// Bonus points this card adds to its colour at resolution.
    pub bonus_points: u8,
    /// Bonus volatility this card adds to its effective volatility (an
    /// Alchemist's fired combo).
    pub bonus_volatility: u8,
    /// What fired on this card, for the depile narration (`None` when nothing
    /// compounded — including a lone combo half, which is a plain card).
    pub fire: Option<CompoundingFire>,
}

/// The compounding-relevant Brewer seats — the three compounding Brewers, by
/// the rule each bends. Built once per round from the public Brewer map; empty
/// when no brewer phase ran (e.g. the sync test runner), in which case combos
/// still pay points and thresholds still fire — only the bends are absent.
#[derive(Debug, Default, Clone)]
pub struct CompoundingBrewers {
    /// Herbalists — their named combos fire from a single half.
    pub herbalists: Vec<PlayerId>,
    /// Distillers — their count-threshold cards see a larger pot.
    pub distillers: Vec<PlayerId>,
    /// Alchemists — their fired combo also adds volatility.
    pub alchemists: Vec<PlayerId>,
}

impl CompoundingBrewers {
    /// Collect the compounding Brewer seats from the table's public Brewer map.
    pub fn from_map(brewers: &HashMap<PlayerId, Brewer>) -> Self {
        let mut out = CompoundingBrewers::default();
        for (&player, &brewer) in brewers {
            match brewer {
                Brewer::Herbalist => out.herbalists.push(player),
                Brewer::Distiller => out.distillers.push(player),
                Brewer::Alchemist => out.alchemists.push(player),
                _ => {}
            }
        }
        out
    }

    fn is_herbalist(&self, p: PlayerId) -> bool {
        self.herbalists.contains(&p)
    }
    fn is_distiller(&self, p: PlayerId) -> bool {
        self.distillers.contains(&p)
    }
    fn is_alchemist(&self, p: PlayerId) -> bool {
        self.alchemists.contains(&p)
    }
}

/// Recompute every card's compounding bonus from scratch for the current pot.
///
/// Idempotent and order-independent: it resets each card's bonus, then applies
/// count-thresholds (keyed off the public count) and named combos (each pair
/// fires at most once per owner, crediting the **completing** — latest-played —
/// half). Called whenever the pot changes so combo-added volatility feeds the
/// very next explosion check, and the final state drives scoring and the depile.
pub fn recompute(cards: &mut [PotIngredient], brewers: &CompoundingBrewers) {
    for c in cards.iter_mut() {
        c.compounding = CardCompounding::default();
    }

    apply_count_thresholds(cards, brewers);
    apply_combos(cards, brewers);
}

/// Count-threshold (Honey): each tagged card scores `per_card` per card past its
/// threshold, off the public pot count (a Distiller sees the pot larger).
fn apply_count_thresholds(cards: &mut [PotIngredient], brewers: &CompoundingBrewers) {
    let pot_count = cards.len() as u32;
    for c in cards.iter_mut() {
        let Some(Compounding::CountThreshold { past, per_card }) = c.ingredient.compounding else {
            continue;
        };
        let effective_count = pot_count
            + if brewers.is_distiller(c.player) {
                DISTILLER_POT_BONUS
            } else {
                0
            };
        let over = effective_count.saturating_sub(past as u32);
        let bonus = (per_card as u32 * over).min(u8::MAX as u32) as u8;
        if bonus > 0 {
            c.compounding.bonus_points = bonus;
            c.compounding.fire = Some(CompoundingFire::Threshold {
                bonus_points: bonus,
            });
        }
    }
}

/// Named-combo (Bramble): a pair fires for an owner when both halves are in the
/// pot (one half for a Herbalist). The bonus is credited once per (owner, pair)
/// to the completing half; an Alchemist's fire also adds volatility there.
fn apply_combos(cards: &mut [PotIngredient], brewers: &CompoundingBrewers) {
    // Index the combo cards by (owner, pair), preserving pot order so the
    // "completing" card is deterministically the latest-played half.
    let mut groups: HashMap<(PlayerId, ComboPair), Vec<usize>> = HashMap::new();
    for (i, c) in cards.iter().enumerate() {
        if let Some(Compounding::Combo { pair, .. }) = c.ingredient.compounding {
            groups.entry((c.player, pair)).or_default().push(i);
        }
    }

    for ((owner, _pair), idxs) in groups {
        let (mut has_a, mut has_b) = (false, false);
        for &i in &idxs {
            if let Some(Compounding::Combo { half, .. }) = cards[i].ingredient.compounding {
                match half {
                    ComboHalf::A => has_a = true,
                    ComboHalf::B => has_b = true,
                }
            }
        }
        let fires = (has_a && has_b) || (brewers.is_herbalist(owner) && (has_a || has_b));
        if !fires {
            continue;
        }

        // Credit the latest-played half (max wave, then latest in pot order):
        // the card whose play completed the chemistry, so an Alchemist's added
        // volatility lands on a card that can sit in the fatal wave.
        let completing = *idxs
            .iter()
            .max_by_key(|&&i| (cards[i].wave_number, i))
            .expect("a fired combo has at least one half");
        let bonus_volatility = if brewers.is_alchemist(owner) {
            ALCHEMIST_COMBO_VOLATILITY
        } else {
            0
        };
        let c = &mut cards[completing];
        c.compounding.bonus_points = c
            .compounding
            .bonus_points
            .saturating_add(COMBO_BONUS_POINTS);
        c.compounding.bonus_volatility = c
            .compounding
            .bonus_volatility
            .saturating_add(bonus_volatility);
        c.compounding.fire = Some(CompoundingFire::Combo {
            bonus_points: COMBO_BONUS_POINTS,
            bonus_volatility,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::Ingredient;
    use crate::game::pot::PotIngredient;
    use boiling_point_protocol::CardId;
    use boiling_point_protocol::vocab::Color;
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn card(
        player: PlayerId,
        id: u32,
        wave: u8,
        compounding: Option<Compounding>,
    ) -> PotIngredient {
        PotIngredient {
            player,
            ingredient: Ingredient {
                id: CardId(id),
                color: Color::Ruby,
                volatility: 3,
                points: 1,
                compounding,
            },
            colorless: false,
            wave_number: wave,
            exposed: false,
            compounding: CardCompounding::default(),
        }
    }

    fn filler(player: PlayerId, id: u32) -> PotIngredient {
        card(player, id, 1, None)
    }

    /// A Honey card scores +1 per card past the 5th: an 8-card pot adds 3.
    #[test]
    fn count_threshold_scores_more_in_a_big_pot() {
        let p = pid(1);
        let honey = Compounding::CountThreshold {
            past: 5,
            per_card: 1,
        };
        let mut cards = vec![card(p, 1, 1, Some(honey))];
        for id in 2..=8 {
            cards.push(filler(p, id));
        }
        assert_eq!(cards.len(), 8);
        recompute(&mut cards, &CompoundingBrewers::default());
        assert_eq!(cards[0].compounding.bonus_points, 3, "cards 6, 7, 8");
        assert_eq!(
            cards[0].compounding.fire,
            Some(CompoundingFire::Threshold { bonus_points: 3 })
        );
    }

    /// Below the threshold the Honey card adds nothing (a small pot is no payoff).
    #[test]
    fn count_threshold_is_dormant_in_a_small_pot() {
        let p = pid(1);
        let honey = Compounding::CountThreshold {
            past: 5,
            per_card: 1,
        };
        let mut cards = vec![card(p, 1, 1, Some(honey)), filler(p, 2)];
        recompute(&mut cards, &CompoundingBrewers::default());
        assert_eq!(cards[0].compounding.bonus_points, 0);
        assert_eq!(cards[0].compounding.fire, None);
    }

    /// A Distiller treats the pot as 2 cards larger, so payoffs come online sooner.
    #[test]
    fn distiller_sees_a_larger_pot() {
        let p = pid(1);
        let honey = Compounding::CountThreshold {
            past: 5,
            per_card: 1,
        };
        let mut cards = vec![card(p, 1, 1, Some(honey))];
        for id in 2..=5 {
            cards.push(filler(p, id));
        }
        assert_eq!(cards.len(), 5); // at the threshold: a non-Distiller gets 0
        let mut plain = cards.clone();
        recompute(&mut plain, &CompoundingBrewers::default());
        assert_eq!(plain[0].compounding.bonus_points, 0);

        let distiller = CompoundingBrewers {
            distillers: vec![p],
            ..Default::default()
        };
        recompute(&mut cards, &distiller);
        // Effective count 5 + 2 = 7 → 2 past the 5th.
        assert_eq!(cards[0].compounding.bonus_points, 2);
    }

    /// A combo pays off when both halves are present; the completing (later) half
    /// carries the bonus.
    #[test]
    fn combo_pays_when_both_halves_present() {
        let p = pid(1);
        let half_a = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::A,
        };
        let half_b = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::B,
        };
        let mut cards = vec![card(p, 1, 1, Some(half_a)), card(p, 2, 2, Some(half_b))];
        recompute(&mut cards, &CompoundingBrewers::default());
        // The wave-2 half completes the pair and takes the bonus.
        assert_eq!(cards[0].compounding.bonus_points, 0);
        assert_eq!(cards[1].compounding.bonus_points, COMBO_BONUS_POINTS);
        assert_eq!(cards[1].compounding.bonus_volatility, 0, "no Alchemist");
        assert_eq!(
            cards[1].compounding.fire,
            Some(CompoundingFire::Combo {
                bonus_points: COMBO_BONUS_POINTS,
                bonus_volatility: 0,
            })
        );
    }

    /// A lone combo half is a plain card: no bonus, no penalty (bonus-not-requirement).
    #[test]
    fn lone_combo_half_is_a_plain_card() {
        let p = pid(1);
        let half_a = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::A,
        };
        let mut cards = vec![card(p, 1, 1, Some(half_a)), filler(p, 2)];
        recompute(&mut cards, &CompoundingBrewers::default());
        assert_eq!(cards[0].compounding, CardCompounding::default());
    }

    /// A Herbalist's combo fires from a single half.
    #[test]
    fn herbalist_combo_fires_from_one_half() {
        let p = pid(1);
        let half_a = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::A,
        };
        let mut cards = vec![card(p, 1, 1, Some(half_a)), filler(p, 2)];
        let herbalist = CompoundingBrewers {
            herbalists: vec![p],
            ..Default::default()
        };
        recompute(&mut cards, &herbalist);
        assert_eq!(cards[0].compounding.bonus_points, COMBO_BONUS_POINTS);
    }

    /// An Alchemist's fired combo also adds volatility, on the completing half.
    #[test]
    fn alchemist_combo_adds_volatility() {
        let p = pid(1);
        let half_a = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::A,
        };
        let half_b = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::B,
        };
        let mut cards = vec![card(p, 1, 1, Some(half_a)), card(p, 2, 1, Some(half_b))];
        let alchemist = CompoundingBrewers {
            alchemists: vec![p],
            ..Default::default()
        };
        recompute(&mut cards, &alchemist);
        // Both halves in wave 1 → completing is the later in pot order (index 1).
        assert_eq!(
            cards[1].compounding.bonus_volatility,
            ALCHEMIST_COMBO_VOLATILITY
        );
        assert_eq!(cards[1].compounding.bonus_points, COMBO_BONUS_POINTS);
    }

    /// A combo fires once per owner even with extra copies of a half (anti-snowball).
    #[test]
    fn combo_fires_at_most_once_per_owner() {
        let p = pid(1);
        let half_a = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::A,
        };
        let half_b = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::B,
        };
        let mut cards = vec![
            card(p, 1, 1, Some(half_a)),
            card(p, 2, 1, Some(half_a)),
            card(p, 3, 2, Some(half_b)),
        ];
        recompute(&mut cards, &CompoundingBrewers::default());
        let total: u8 = cards.iter().map(|c| c.compounding.bonus_points).sum();
        assert_eq!(total, COMBO_BONUS_POINTS, "one fire, not per copy");
    }

    /// Two players' combos are independent: each owner's pair is its own fire.
    #[test]
    fn combos_are_per_owner() {
        let (p1, p2) = (pid(1), pid(2));
        let half_a = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::A,
        };
        let half_b = Compounding::Combo {
            pair: ComboPair::SageMint,
            half: ComboHalf::B,
        };
        // p1 holds only half A; p2 holds both halves.
        let mut cards = vec![
            card(p1, 1, 1, Some(half_a)),
            card(p2, 2, 1, Some(half_a)),
            card(p2, 3, 2, Some(half_b)),
        ];
        recompute(&mut cards, &CompoundingBrewers::default());
        assert_eq!(cards[0].compounding.bonus_points, 0, "p1's lone half");
        let p2_bonus: u8 = cards
            .iter()
            .filter(|c| c.player == p2)
            .map(|c| c.compounding.bonus_points)
            .sum();
        assert_eq!(p2_bonus, COMBO_BONUS_POINTS);
    }
}
