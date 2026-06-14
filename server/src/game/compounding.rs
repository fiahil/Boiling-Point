//! In-pot ingredient interactions (change `boom2-compounding`): the legible
//! classes of compounding that give the Bramble/Honey buckets and the three
//! compounding Brewers their teeth.
//!
//! Two classes ship here:
//! - **Count-threshold (Honey).** A card scores `per_card` extra points for each
//!   card in the pot past the `past`-th. The trigger is the **public** card
//!   count, so it is plannable; a Distiller treats the pot as
//!   [`DISTILLER_POT_BONUS`] cards larger.
//! - **Named-combo (Bramble).** A named **2–5-member set** ([`ComboId`]) pays a
//!   size-scaling [`combo_bonus`] when **all** its members are in the pot
//!   (a Herbalist's fires twice, [`HERBALIST_COMBO_MULTIPLIER`]). Bigger combos
//!   are far rarer to assemble but pay massively. Combos are *bonuses, never
//!   requirements*: a lone member scores nothing extra and is penalised nothing
//!   (it is a plain ingredient), and a combo is detected by **owner** so it may
//!   span colours — the bonus credits the owner's colour. An Alchemist's fired
//!   combo also adds [`ALCHEMIST_COMBO_VOLATILITY`] to the completing card,
//!   flowing through the **same** effective volatility the detonator sort and
//!   the explosion check use ([`super::pot::PotIngredient::effective_volatility`]).
//!
//! Knowability is the design axis (docs/06_boom2/02 §O3): both shipped classes
//! are knowable (a public count; a recipe you drafted). The hidden,
//! snowball-prone **colour-synergy** class is deliberately **not** authored in
//! this change — its cap is structural (no such ingredient exists). If it is
//! added later it MUST be capped or Peek-gated (spec `boom-compounding`).
//!
//! Every magnitude here is `[needs playtesting]` (Principle IV); the harness
//! measures snowball, lone-combo-member (dead-draw) rate, and threshold payoff.

use std::collections::{HashMap, HashSet};

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::server::CompoundingFire;
use boiling_point_protocol::vocab::{Brewer, Color, ComboId, Compounding};

use super::brewers::{ALCHEMIST_COMBO_VOLATILITY, DISTILLER_POT_BONUS, HERBALIST_COMBO_MULTIPLIER};
use super::pot::PotIngredient;

/// The points a completed named combo pays, by its size (2–5 members). The
/// curve escalates so a hard-to-assemble 5-combo is a massive jackpot while a
/// 2-combo is a modest seasoning. All `[needs playtesting]`
/// (docs/06_boom2/02: "+2 when paired" anchors the size-2 value).
pub fn combo_bonus(size: u8) -> u8 {
    match size {
        0 | 1 => 0,
        2 => 2,
        3 => 5,
        4 => 9,
        _ => 15, // 5+ — the rare, massive payoff
    }
}

/// The compounding bonus a single pot card carries, recomputed whenever the pot
/// changes. Default is "no bonus" — a plain card or a lone combo member.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CardCompounding {
    /// Bonus points this card adds at resolution (to `credit_color`).
    pub bonus_points: u8,
    /// Bonus volatility this card adds to its effective volatility (an
    /// Alchemist's fired combo).
    pub bonus_volatility: u8,
    /// The colour the bonus points are credited to (`boom2-compounding`): a
    /// threshold credits its own card's colour; a combo credits its **owner's**
    /// colour, so a combo assembled across colours always pays the player who
    /// built it. `None` when nothing scores (no bonus, or a colourless card with
    /// no owner colour).
    pub credit_color: Option<Color>,
    /// What fired on this card, for the depile narration (`None` when nothing
    /// compounded — including a lone combo member, which is a plain card).
    pub fire: Option<CompoundingFire>,
}

/// The compounding-relevant Brewer seats — the three compounding Brewers, by
/// the rule each bends. Built once per round from the public Brewer map; empty
/// when no brewer phase ran (e.g. the sync test runner), in which case combos
/// still pay points and thresholds still fire — only the bends are absent.
#[derive(Debug, Default, Clone)]
pub struct CompoundingBrewers {
    /// Herbalists — their completed combos fire twice (double payoff).
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
/// count-thresholds (keyed off the public count) and named combos (each combo
/// fires at most once per owner when **all** its members are present, crediting
/// the **owner's** colour). Called whenever the pot changes so combo-added
/// volatility feeds the very next explosion check, and the final state drives
/// scoring and the depile. `player_color` maps each seat to its anchor colour
/// (empty in brewerless sync tests — combos then credit the completing card's
/// own colour).
pub fn recompute(
    cards: &mut [PotIngredient],
    brewers: &CompoundingBrewers,
    player_color: &HashMap<PlayerId, Color>,
) {
    for c in cards.iter_mut() {
        c.compounding = CardCompounding::default();
    }

    apply_count_thresholds(cards, brewers);
    apply_combos(cards, brewers, player_color);
}

/// Count-threshold (Honey): each tagged card scores `per_card` per card past its
/// threshold, off the public pot count (a Distiller sees the pot larger). The
/// bonus is credited to the card's own colour (forfeited if played colourless).
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
            c.compounding.credit_color = c.effective_color();
            c.compounding.fire = Some(CompoundingFire::Threshold {
                bonus_points: bonus,
            });
        }
    }
}

/// Named-combo (Bramble): a combo fires for an owner when **all** its distinct
/// members (2–5) are in the pot — regardless of their colours. The size-scaling
/// bonus is credited once per (owner, combo) to the completing (latest-played)
/// member but counts for the **owner's** colour, so a cross-colour combo still
/// pays its builder. A Herbalist's fires twice; an Alchemist's also adds
/// volatility (on the completing card, so it can sit in the fatal wave).
fn apply_combos(
    cards: &mut [PotIngredient],
    brewers: &CompoundingBrewers,
    player_color: &HashMap<PlayerId, Color>,
) {
    // Index combo cards by (owner, combo), preserving pot order so the
    // "completing" card is deterministically the latest-played member.
    let mut groups: HashMap<(PlayerId, ComboId), Vec<usize>> = HashMap::new();
    for (i, c) in cards.iter().enumerate() {
        if let Some(Compounding::Combo { combo, .. }) = c.ingredient.compounding {
            groups.entry((c.player, combo)).or_default().push(i);
        }
    }

    for ((owner, combo), idxs) in groups {
        // A combo fires when its distinct members are all present.
        let members: HashSet<u8> = idxs
            .iter()
            .filter_map(|&i| match cards[i].ingredient.compounding {
                Some(Compounding::Combo { member, .. }) => Some(member),
                _ => None,
            })
            .collect();
        if (members.len() as u8) < combo.size() {
            continue;
        }

        let size = combo.size();
        let mut bonus_points = combo_bonus(size);
        if brewers.is_herbalist(owner) {
            bonus_points = bonus_points.saturating_mul(HERBALIST_COMBO_MULTIPLIER);
        }
        let bonus_volatility = if brewers.is_alchemist(owner) {
            ALCHEMIST_COMBO_VOLATILITY
        } else {
            0
        };

        // Credit the latest-played member (max wave, then latest in pot order).
        let completing = *idxs
            .iter()
            .max_by_key(|&&i| (cards[i].wave_number, i))
            .expect("a fired combo has at least one member");
        // The combo pays the owner's colour (cross-colour safe); fall back to
        // the completing card's own colour when no colour map is supplied.
        let credit = player_color
            .get(&owner)
            .copied()
            .or_else(|| cards[completing].effective_color());
        let c = &mut cards[completing];
        c.compounding.bonus_points = c.compounding.bonus_points.saturating_add(bonus_points);
        c.compounding.bonus_volatility = c
            .compounding
            .bonus_volatility
            .saturating_add(bonus_volatility);
        c.compounding.credit_color = credit;
        c.compounding.fire = Some(CompoundingFire::Combo {
            size,
            bonus_points,
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

    fn no_colors() -> HashMap<PlayerId, Color> {
        HashMap::new()
    }

    /// A pot card of `color` owned by `player`, optionally tagged.
    fn card(
        player: PlayerId,
        id: u32,
        wave: u8,
        color: Color,
        compounding: Option<Compounding>,
    ) -> PotIngredient {
        PotIngredient {
            player,
            ingredient: Ingredient {
                id: CardId(id),
                color,
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

    fn ruby(
        player: PlayerId,
        id: u32,
        wave: u8,
        compounding: Option<Compounding>,
    ) -> PotIngredient {
        card(player, id, wave, Color::Ruby, compounding)
    }

    fn filler(player: PlayerId, id: u32) -> PotIngredient {
        ruby(player, id, 1, None)
    }

    fn combo(combo: ComboId, member: u8) -> Option<Compounding> {
        Some(Compounding::Combo { combo, member })
    }

    /// The payoff curve escalates: a 2-combo is modest, a 5-combo is massive.
    #[test]
    fn combo_bonus_escalates_with_size() {
        assert_eq!(combo_bonus(2), 2);
        assert!(combo_bonus(3) > combo_bonus(2));
        assert!(combo_bonus(4) > combo_bonus(3));
        assert!(combo_bonus(5) > combo_bonus(4));
        assert_eq!(combo_bonus(1), 0);
    }

    /// A Honey card scores +1 per card past the 5th: an 8-card pot adds 3.
    #[test]
    fn count_threshold_scores_more_in_a_big_pot() {
        let p = pid(1);
        let honey = Compounding::CountThreshold {
            past: 5,
            per_card: 1,
        };
        let mut cards = vec![ruby(p, 1, 1, Some(honey))];
        for id in 2..=8 {
            cards.push(filler(p, id));
        }
        assert_eq!(cards.len(), 8);
        recompute(&mut cards, &CompoundingBrewers::default(), &no_colors());
        assert_eq!(cards[0].compounding.bonus_points, 3, "cards 6, 7, 8");
        assert_eq!(cards[0].compounding.credit_color, Some(Color::Ruby));
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
        let mut cards = vec![ruby(p, 1, 1, Some(honey)), filler(p, 2)];
        recompute(&mut cards, &CompoundingBrewers::default(), &no_colors());
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
        let mut cards = vec![ruby(p, 1, 1, Some(honey))];
        for id in 2..=5 {
            cards.push(filler(p, id));
        }
        assert_eq!(cards.len(), 5); // at the threshold: a non-Distiller gets 0
        let mut plain = cards.clone();
        recompute(&mut plain, &CompoundingBrewers::default(), &no_colors());
        assert_eq!(plain[0].compounding.bonus_points, 0);

        let distiller = CompoundingBrewers {
            distillers: vec![p],
            ..Default::default()
        };
        recompute(&mut cards, &distiller, &no_colors());
        // Effective count 5 + 2 = 7 → 2 past the 5th.
        assert_eq!(cards[0].compounding.bonus_points, 2);
    }

    /// A 2-combo pays off when both members are present; the completing (later)
    /// member carries the bonus.
    #[test]
    fn combo_pays_when_all_members_present() {
        let p = pid(1);
        let mut cards = vec![
            ruby(p, 1, 1, combo(ComboId::SageMint, 0)),
            ruby(p, 2, 2, combo(ComboId::SageMint, 1)),
        ];
        recompute(&mut cards, &CompoundingBrewers::default(), &no_colors());
        assert_eq!(cards[0].compounding.bonus_points, 0);
        assert_eq!(cards[1].compounding.bonus_points, combo_bonus(2));
        assert_eq!(cards[1].compounding.bonus_volatility, 0, "no Alchemist");
        assert_eq!(
            cards[1].compounding.fire,
            Some(CompoundingFire::Combo {
                size: 2,
                bonus_points: combo_bonus(2),
                bonus_volatility: 0,
            })
        );
    }

    /// A larger combo pays its larger bonus only when ALL members are present;
    /// missing one member yields nothing.
    #[test]
    fn larger_combo_needs_every_member() {
        let p = pid(1);
        // GoldenElixir is size 4. Three of four members: no fire.
        let mut three = vec![
            ruby(p, 1, 1, combo(ComboId::GoldenElixir, 0)),
            ruby(p, 2, 1, combo(ComboId::GoldenElixir, 1)),
            ruby(p, 3, 1, combo(ComboId::GoldenElixir, 2)),
        ];
        recompute(&mut three, &CompoundingBrewers::default(), &no_colors());
        assert!(three.iter().all(|c| c.compounding.bonus_points == 0));

        // The fourth completes it for the size-4 payoff.
        three.push(ruby(p, 4, 2, combo(ComboId::GoldenElixir, 3)));
        recompute(&mut three, &CompoundingBrewers::default(), &no_colors());
        let total: u8 = three.iter().map(|c| c.compounding.bonus_points).sum();
        assert_eq!(total, combo_bonus(4));
    }

    /// A lone combo member is a plain card: no bonus, no penalty (bonus-not-requirement).
    #[test]
    fn lone_combo_member_is_a_plain_card() {
        let p = pid(1);
        let mut cards = vec![ruby(p, 1, 1, combo(ComboId::SageMint, 0)), filler(p, 2)];
        recompute(&mut cards, &CompoundingBrewers::default(), &no_colors());
        assert_eq!(cards[0].compounding, CardCompounding::default());
    }

    /// A Herbalist's completed combo fires twice — double the payoff.
    #[test]
    fn herbalist_combo_fires_twice() {
        let p = pid(1);
        let mut cards = vec![
            ruby(p, 1, 1, combo(ComboId::SageMint, 0)),
            ruby(p, 2, 2, combo(ComboId::SageMint, 1)),
        ];
        let herbalist = CompoundingBrewers {
            herbalists: vec![p],
            ..Default::default()
        };
        recompute(&mut cards, &herbalist, &no_colors());
        assert_eq!(cards[1].compounding.bonus_points, combo_bonus(2) * 2);
    }

    /// An Alchemist's fired combo also adds volatility, on the completing member.
    #[test]
    fn alchemist_combo_adds_volatility() {
        let p = pid(1);
        let mut cards = vec![
            ruby(p, 1, 1, combo(ComboId::SageMint, 0)),
            ruby(p, 2, 1, combo(ComboId::SageMint, 1)),
        ];
        let alchemist = CompoundingBrewers {
            alchemists: vec![p],
            ..Default::default()
        };
        recompute(&mut cards, &alchemist, &no_colors());
        // Both members in wave 1 → completing is the later in pot order (index 1).
        assert_eq!(
            cards[1].compounding.bonus_volatility,
            ALCHEMIST_COMBO_VOLATILITY
        );
        assert_eq!(cards[1].compounding.bonus_points, combo_bonus(2));
    }

    /// A combo fires once per owner even with extra copies of a member (anti-snowball).
    #[test]
    fn combo_fires_at_most_once_per_owner() {
        let p = pid(1);
        let mut cards = vec![
            ruby(p, 1, 1, combo(ComboId::SageMint, 0)),
            ruby(p, 2, 1, combo(ComboId::SageMint, 0)),
            ruby(p, 3, 2, combo(ComboId::SageMint, 1)),
        ];
        recompute(&mut cards, &CompoundingBrewers::default(), &no_colors());
        let total: u8 = cards.iter().map(|c| c.compounding.bonus_points).sum();
        assert_eq!(total, combo_bonus(2), "one fire, not per copy");
    }

    /// Two players' combos are independent: each owner's set is its own fire.
    #[test]
    fn combos_are_per_owner() {
        let (p1, p2) = (pid(1), pid(2));
        // p1 holds only member 0; p2 holds both members.
        let mut cards = vec![
            ruby(p1, 1, 1, combo(ComboId::SageMint, 0)),
            ruby(p2, 2, 1, combo(ComboId::SageMint, 0)),
            ruby(p2, 3, 2, combo(ComboId::SageMint, 1)),
        ];
        recompute(&mut cards, &CompoundingBrewers::default(), &no_colors());
        assert_eq!(cards[0].compounding.bonus_points, 0, "p1's lone member");
        let p2_bonus: u8 = cards
            .iter()
            .filter(|c| c.player == p2)
            .map(|c| c.compounding.bonus_points)
            .sum();
        assert_eq!(p2_bonus, combo_bonus(2));
    }

    /// A combo assembled across colours fires and credits its OWNER's colour —
    /// even when the completing member is a different colour or colourless.
    #[test]
    fn combo_applies_across_colors_and_credits_owner() {
        let p = pid(1);
        let player_color = HashMap::from([(p, Color::Ruby)]);
        // Member 0 is the owner's Ruby; member 1 (the completer) is off-colour
        // Emerald — without owner-credit it would feed the wrong colour.
        let mut cards = vec![
            card(p, 1, 1, Color::Ruby, combo(ComboId::SageMint, 0)),
            card(p, 2, 2, Color::Emerald, combo(ComboId::SageMint, 1)),
        ];
        recompute(&mut cards, &CompoundingBrewers::default(), &player_color);
        // The combo fired (cross-colour) and credits the owner's Ruby.
        assert_eq!(cards[1].compounding.bonus_points, combo_bonus(2));
        assert_eq!(cards[1].compounding.credit_color, Some(Color::Ruby));

        // Even a colourless completer still pays the owner's colour.
        let mut colorless = vec![
            card(p, 1, 1, Color::Ruby, combo(ComboId::SageMint, 0)),
            PotIngredient {
                colorless: true,
                ..card(p, 2, 2, Color::Ruby, combo(ComboId::SageMint, 1))
            },
        ];
        recompute(
            &mut colorless,
            &CompoundingBrewers::default(),
            &player_color,
        );
        assert_eq!(colorless[1].compounding.credit_color, Some(Color::Ruby));
        assert_eq!(colorless[1].compounding.bonus_points, combo_bonus(2));
    }
}
