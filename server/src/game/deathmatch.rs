//! The Deathmatch tiebreaker: pure elimination among players tied for the lead.
//!
//! Only volatility matters here — colour and points are dead, and the grimoire
//! stays shut (spells are a round-economy device; the tiebreak is raw nerve).
//! Every participant is forced to commit one ingredient per wave; when the pot
//! explodes, the highest total-volatility contributor is the Detonator and is
//! eliminated (ties remove all of them) — aligned with the main game's
//! "heaviest pays" model. Hands exhausting without an explosion yields
//! co-champions.

use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use boiling_point_protocol::{CardId, PlayerId};

use super::state::Hand;

/// The outcome of a Deathmatch: a single champion, or co-champions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeathmatchResult {
    /// One player won the tiebreak outright.
    Champion(PlayerId),
    /// Multiple players share the win (no boom, or simultaneous elimination).
    CoChampions(Vec<PlayerId>),
}

/// Chooses which ingredient a participant is forced to commit this wave.
pub trait DeathmatchDecider {
    /// Pick an ingredient id from `hand` (guaranteed non-empty) for `player`.
    fn pick(&mut self, player: PlayerId, hand: &Hand) -> CardId;
}

impl<F: FnMut(PlayerId, &Hand) -> CardId> DeathmatchDecider for F {
    fn pick(&mut self, player: PlayerId, hand: &Hand) -> CardId {
        self(player, hand)
    }
}

/// The result of a single bout (one boiling point).
enum BoutOutcome {
    /// The pot exploded; these participants are the Detonator(s).
    Explosion { detonators: Vec<PlayerId> },
    /// A participant could not commit (hands exhausted) before any explosion.
    ExhaustedSafe,
}

/// Compute the Detonator(s): the highest-volatility contributor(s).
fn detonators(ids: &[PlayerId], contribution: &HashMap<PlayerId, i32>) -> Vec<PlayerId> {
    let max = ids
        .iter()
        .map(|p| contribution.get(p).copied().unwrap_or(0))
        .max();
    match max {
        None => Vec::new(),
        Some(m) => ids
            .iter()
            .filter(|p| contribution.get(p).copied().unwrap_or(0) == m)
            .copied()
            .collect(),
    }
}

/// Run one bout to its end (explosion or exhaustion), mutating participant hands.
fn run_bout(
    participants: &mut [(PlayerId, Hand)],
    boiling_point: i32,
    decider: &mut dyn DeathmatchDecider,
) -> BoutOutcome {
    let ids: Vec<PlayerId> = participants.iter().map(|(id, _)| *id).collect();
    let mut contribution: HashMap<PlayerId, i32> = ids.iter().map(|p| (*p, 0)).collect();
    let mut pot_volatility = 0i32;

    loop {
        // Forced commit requires every participant to have an ingredient.
        if participants.iter().any(|(_, h)| h.no_ingredients()) {
            return BoutOutcome::ExhaustedSafe;
        }

        // Each participant commits exactly one ingredient.
        for (id, hand) in participants.iter_mut() {
            let pick = decider.pick(*id, hand);
            let card = hand
                .take_ingredient(pick)
                .or_else(|| {
                    let first = hand.ingredients().first().map(|c| c.id)?;
                    hand.take_ingredient(first)
                })
                .expect("participant has an ingredient");
            *contribution.entry(*id).or_insert(0) += card.volatility as i32;
            pot_volatility += card.volatility as i32;
        }

        if pot_volatility > boiling_point {
            return BoutOutcome::Explosion {
                detonators: detonators(&ids, &contribution),
            };
        }
    }
}

/// Run a full Deathmatch among the tied players, returning the champion(s).
/// Participants bring their remaining hands; an empty hand at the start is
/// eliminated immediately (placed last). Bouts repeat with a fresh boiling point
/// until one survivor remains, hands exhaust, or everyone is eliminated at once.
pub fn run_deathmatch(
    tied: Vec<(PlayerId, Hand)>,
    bp_min: u8,
    bp_max: u8,
    decider: &mut dyn DeathmatchDecider,
    seed: u64,
) -> DeathmatchResult {
    let mut rng = StdRng::seed_from_u64(seed);
    // Empty-handed players are eliminated immediately.
    let mut participants: Vec<(PlayerId, Hand)> = tied
        .into_iter()
        .filter(|(_, h)| !h.no_ingredients())
        .collect();

    if participants.is_empty() {
        return DeathmatchResult::CoChampions(Vec::new());
    }

    loop {
        if participants.len() == 1 {
            return DeathmatchResult::Champion(participants[0].0);
        }
        let bp = rng.gen_range(bp_min..=bp_max) as i32;
        match run_bout(&mut participants, bp, decider) {
            BoutOutcome::ExhaustedSafe => {
                return DeathmatchResult::CoChampions(
                    participants.iter().map(|(id, _)| *id).collect(),
                );
            }
            BoutOutcome::Explosion { detonators } => {
                participants.retain(|(id, _)| !detonators.contains(id));
                match participants.len() {
                    1 => return DeathmatchResult::Champion(participants[0].0),
                    0 => return DeathmatchResult::CoChampions(detonators), // everyone out at once
                    _ => continue, // fresh bout among survivors
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::Ingredient;
    use boiling_point_protocol::vocab::Color;
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn ing(id: u32, vol: u8) -> Ingredient {
        Ingredient {
            id: CardId(id),
            color: Color::Ruby,
            volatility: vol,
            points: 0,
            compounding: None,
        }
    }

    fn hand(cards: Vec<Ingredient>) -> Hand {
        let mut h = Hand::new();
        h.add_ingredients(cards);
        h
    }

    /// Always play the first ingredient in hand.
    fn first() -> impl FnMut(PlayerId, &Hand) -> CardId {
        |_p, h| h.ingredients().first().unwrap().id
    }

    #[test]
    fn highest_volatility_contributor_is_eliminated() {
        let tied = vec![
            (pid(1), hand(vec![ing(1, 5)])),
            (pid(2), hand(vec![ing(2, 1)])),
        ];
        // pot 6 > bp 3 → p1 (vol 5) is the Detonator → p2 champion.
        let mut d = first();
        let result = run_deathmatch(tied, 3, 3, &mut d, 1);
        assert_eq!(result, DeathmatchResult::Champion(pid(2)));
    }

    #[test]
    fn tie_for_most_eliminates_all_of_them() {
        let ids = [pid(1), pid(2), pid(3)];
        let contribution: HashMap<PlayerId, i32> = [(ids[0], 5), (ids[1], 5), (ids[2], 2)]
            .into_iter()
            .collect();
        let dets = detonators(&ids, &contribution);
        assert_eq!(dets.len(), 2);
        assert!(dets.contains(&ids[0]) && dets.contains(&ids[1]));
    }

    #[test]
    fn exhausted_hands_yield_co_champions() {
        // One low-volatility ingredient each, huge boiling point → never
        // explodes; hands exhaust after one wave → co-champions.
        let tied = vec![
            (pid(1), hand(vec![ing(1, 1)])),
            (pid(2), hand(vec![ing(2, 1)])),
        ];
        let mut d = first();
        let result = run_deathmatch(tied, 50, 50, &mut d, 1);
        match result {
            DeathmatchResult::CoChampions(mut w) => {
                w.sort_by_key(|p| p.0);
                assert_eq!(w, vec![pid(1), pid(2)]);
            }
            _ => panic!("expected co-champions"),
        }
    }
}
