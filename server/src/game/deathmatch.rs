//! The Deathmatch tiebreaker: pure elimination among players tied for the lead.
//!
//! Only volatility matters here — colour and points are dead. Every participant
//! is forced to commit one card per wave; when the pot explodes, the highest
//! total-volatility contributor is the Detonator and is eliminated (ties remove
//! all of them; Shield redirects the blast to the next-highest, cascading; all
//! shielded → no casualty and a fresh bout). Hands exhausting without an
//! explosion yields co-champions. Effects resolve through the same strategy
//! objects as normal play, via a volatility-only [`EffectCtx`].

use std::collections::{HashMap, HashSet};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use boiling_point_protocol::vocab::{Color, EffectKind};
use boiling_point_protocol::{CardId, PlayerId};

use crate::content::ContentRegistry;
use crate::content::effect::{EffectCategory, EffectCtx};

use super::card::Card;
use super::state::Hand;

/// The outcome of a Deathmatch: a single champion, or co-champions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeathmatchResult {
    /// One player won the tiebreak outright.
    Champion(PlayerId),
    /// Multiple players share the win (no boom, or simultaneous elimination).
    CoChampions(Vec<PlayerId>),
}

/// Chooses which card a participant is forced to commit this wave.
pub trait DeathmatchDecider {
    /// Pick a card id from `hand` (which is guaranteed non-empty) for `player`.
    fn pick(&mut self, player: PlayerId, hand: &Hand) -> CardId;
}

impl<F: FnMut(PlayerId, &Hand) -> CardId> DeathmatchDecider for F {
    fn pick(&mut self, player: PlayerId, hand: &Hand) -> CardId {
        self(player, hand)
    }
}

/// Volatility-only resolution state for one bout (one boiling point), reused
/// across its waves. Implements [`EffectCtx`] so the standard effect strategies
/// apply, with colour/points effects as no-ops.
struct Bout {
    pot_volatility: i32,
    contribution: HashMap<PlayerId, i32>,
    shielded: HashSet<PlayerId>,
    committed: Vec<(PlayerId, Card)>,
    recalled: Vec<(PlayerId, Card)>,
    peeked: Vec<PlayerId>,
    cur_player: PlayerId,
    cur_card: CardId,
}

impl Bout {
    fn new(participants: &[PlayerId]) -> Self {
        Bout {
            pot_volatility: 0,
            contribution: participants.iter().map(|p| (*p, 0)).collect(),
            shielded: HashSet::new(),
            committed: Vec::new(),
            recalled: Vec::new(),
            peeked: Vec::new(),
            cur_player: PlayerId(uuid::Uuid::nil()),
            cur_card: CardId(0),
        }
    }
}

impl EffectCtx for Bout {
    fn card_color(&self) -> Color {
        self.committed
            .iter()
            .find(|(_, c)| c.id == self.cur_card)
            .map(|(_, c)| c.color)
            .unwrap_or(Color::Wild)
    }

    fn adjust_volatility(&mut self, delta: i16) {
        self.pot_volatility = (self.pot_volatility + delta as i32).max(0);
        *self.contribution.entry(self.cur_player).or_insert(0) += delta as i32;
    }

    fn reveal_boiling_point_to_actor(&mut self) {
        self.peeked.push(self.cur_player);
    }

    fn shield_actor(&mut self) {
        self.shielded.insert(self.cur_player);
    }

    // Information/identity/points effects are dead in a Deathmatch.
    fn expose_random_card(&mut self) {}
    fn adopt_pre_wave_dominant_color(&mut self) {}
    fn double_pre_wave_color_points(&mut self, _color: Color) {}

    fn recall_own_card(&mut self) {
        // Pull back one of the actor's own previously-committed cards (not the
        // Recall card itself): drop its volatility from pot and contribution.
        let pos = self
            .committed
            .iter()
            .position(|(p, c)| *p == self.cur_player && c.id != self.cur_card);
        if let Some(pos) = pos {
            let (player, card) = self.committed.remove(pos);
            self.pot_volatility = (self.pot_volatility - card.volatility as i32).max(0);
            *self.contribution.entry(player).or_insert(0) -= card.volatility as i32;
            self.recalled.push((player, card));
        }
    }
}

/// The result of a single bout (one boiling point).
enum BoutOutcome {
    /// The pot exploded; these participants are the Detonator(s) (possibly empty
    /// if everyone was shielded).
    Explosion { detonators: Vec<PlayerId> },
    /// A participant could not commit (hands exhausted) before any explosion.
    ExhaustedSafe,
}

/// Compute the Detonator(s): the highest-volatility contributor(s) among the
/// non-shielded participants. Empty if everyone is shielded (no casualty).
fn detonators(
    ids: &[PlayerId],
    contribution: &HashMap<PlayerId, i32>,
    shielded: &HashSet<PlayerId>,
) -> Vec<PlayerId> {
    let max = ids
        .iter()
        .filter(|p| !shielded.contains(p))
        .map(|p| contribution.get(p).copied().unwrap_or(0))
        .max();
    match max {
        None => Vec::new(), // all shielded
        Some(m) => ids
            .iter()
            .filter(|p| !shielded.contains(p) && contribution.get(p).copied().unwrap_or(0) == m)
            .copied()
            .collect(),
    }
}

/// Run one bout to its end (explosion or exhaustion), mutating participant hands.
fn run_bout(
    registry: &ContentRegistry,
    participants: &mut [(PlayerId, Hand)],
    boiling_point: i32,
    decider: &mut dyn DeathmatchDecider,
) -> BoutOutcome {
    let ids: Vec<PlayerId> = participants.iter().map(|(id, _)| *id).collect();
    let mut bout = Bout::new(&ids);

    loop {
        // Forced commit requires every participant to have a card.
        if participants.iter().any(|(_, h)| h.is_empty()) {
            return BoutOutcome::ExhaustedSafe;
        }

        // Each participant commits exactly one card.
        let mut effects: Vec<(CardId, PlayerId, EffectKind)> = Vec::new();
        for (id, hand) in participants.iter_mut() {
            let pick = decider.pick(*id, hand);
            let card = hand
                .take(pick)
                .or_else(|| {
                    hand.views()
                        .first()
                        .map(|h| h.id)
                        .and_then(|fid| hand.take(fid))
                })
                .expect("participant has a card");
            *bout.contribution.entry(*id).or_insert(0) += card.volatility as i32;
            bout.pot_volatility += card.volatility as i32;
            if let Some(e) = card.effect {
                effects.push((card.id, *id, e));
            }
            bout.committed.push((*id, card));
        }

        // Resolve effects in the fixed category order.
        effects.sort_by_key(|(_, _, k)| {
            registry
                .effect(*k)
                .map(|b| b.category())
                .unwrap_or(EffectCategory::Information)
        });
        for (card_id, player, kind) in effects {
            if let Some(behavior) = registry.effect(kind) {
                bout.cur_card = card_id;
                bout.cur_player = player;
                behavior.resolve(&mut bout);
            }
        }

        // Return recalled cards to their owners' hands.
        let recalled = std::mem::take(&mut bout.recalled);
        for (player, card) in recalled {
            if let Some((_, hand)) = participants.iter_mut().find(|(id, _)| *id == player) {
                hand.add([card]);
            }
        }

        if bout.pot_volatility > boiling_point {
            return BoutOutcome::Explosion {
                detonators: detonators(&ids, &bout.contribution, &bout.shielded),
            };
        }
    }
}

/// Run a full Deathmatch among the tied players, returning the champion(s).
/// Participants bring their remaining hands; an empty hand at the start is
/// eliminated immediately (placed last). Bouts repeat with a fresh boiling point
/// until one survivor remains, hands exhaust, or everyone is eliminated at once.
pub fn run_deathmatch(
    registry: &ContentRegistry,
    tied: Vec<(PlayerId, Hand)>,
    bp_min: u8,
    bp_max: u8,
    decider: &mut dyn DeathmatchDecider,
    seed: u64,
) -> DeathmatchResult {
    let mut rng = StdRng::seed_from_u64(seed);
    // Empty-handed players are eliminated immediately.
    let mut participants: Vec<(PlayerId, Hand)> =
        tied.into_iter().filter(|(_, h)| !h.is_empty()).collect();

    if participants.is_empty() {
        return DeathmatchResult::CoChampions(Vec::new());
    }

    loop {
        if participants.len() == 1 {
            return DeathmatchResult::Champion(participants[0].0);
        }
        let bp = rng.gen_range(bp_min..=bp_max) as i32;
        match run_bout(registry, &mut participants, bp, decider) {
            BoutOutcome::ExhaustedSafe => {
                return DeathmatchResult::CoChampions(
                    participants.iter().map(|(id, _)| *id).collect(),
                );
            }
            BoutOutcome::Explosion { detonators } => {
                if detonators.is_empty() {
                    continue; // all shielded — no casualty, fresh bout
                }
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

    fn card(id: u32, vol: u8, effect: Option<EffectKind>) -> Card {
        Card {
            id: CardId(id),
            color: Color::Ruby,
            volatility: vol,
            points: 0,
            effect,
        }
    }

    fn hand(cards: Vec<Card>) -> Hand {
        let mut h = Hand::new();
        h.add(cards);
        h
    }

    /// Always play the first card in hand.
    fn first() -> impl FnMut(PlayerId, &Hand) -> CardId {
        |_p, h| h.views().first().unwrap().id
    }

    #[test]
    fn highest_volatility_contributor_is_eliminated() {
        let reg = registry();
        let tied = vec![
            (pid(1), hand(vec![card(1, 5, None)])),
            (pid(2), hand(vec![card(2, 1, None)])),
        ];
        // pot 6 > bp 3 → p1 (vol 5) is the Detonator → p2 champion.
        let mut d = first();
        let result = run_deathmatch(&reg, tied, 3, 3, &mut d, 1);
        assert_eq!(result, DeathmatchResult::Champion(pid(2)));
    }

    #[test]
    fn shield_redirects_to_next_highest() {
        let reg = registry();
        // p1 contributes the most (9) but is shielded → blast hits p2 (5).
        let mut participants = vec![
            (pid(1), hand(vec![card(1, 9, Some(EffectKind::Shield))])),
            (pid(2), hand(vec![card(2, 5, None)])),
            (pid(3), hand(vec![card(3, 1, None)])),
        ];
        let mut d = first();
        match run_bout(&reg, &mut participants, 10, &mut d) {
            BoutOutcome::Explosion { detonators } => assert_eq!(detonators, vec![pid(2)]),
            _ => panic!("expected explosion"),
        }
    }

    #[test]
    fn all_shielded_yields_no_casualty() {
        let reg = registry();
        let mut participants = vec![
            (pid(1), hand(vec![card(1, 9, Some(EffectKind::Shield))])),
            (pid(2), hand(vec![card(2, 9, Some(EffectKind::Shield))])),
        ];
        let mut d = first();
        match run_bout(&reg, &mut participants, 10, &mut d) {
            BoutOutcome::Explosion { detonators } => assert!(detonators.is_empty()),
            _ => panic!("expected explosion"),
        }
    }

    #[test]
    fn tie_for_most_eliminates_all_of_them() {
        let ids = [pid(1), pid(2), pid(3)];
        let contribution: HashMap<PlayerId, i32> = [(ids[0], 5), (ids[1], 5), (ids[2], 2)]
            .into_iter()
            .collect();
        let dets = detonators(&ids, &contribution, &HashSet::new());
        assert_eq!(dets.len(), 2);
        assert!(dets.contains(&ids[0]) && dets.contains(&ids[1]));
    }

    #[test]
    fn exhausted_hands_yield_co_champions() {
        let reg = registry();
        // One low-volatility card each, huge boiling point → never explodes;
        // hands exhaust after one wave → co-champions.
        let tied = vec![
            (pid(1), hand(vec![card(1, 1, None)])),
            (pid(2), hand(vec![card(2, 1, None)])),
        ];
        let mut d = first();
        let result = run_deathmatch(&reg, tied, 50, 50, &mut d, 1);
        match result {
            DeathmatchResult::CoChampions(mut w) => {
                w.sort_by_key(|p| p.0);
                assert_eq!(w, vec![pid(1), pid(2)]);
            }
            _ => panic!("expected co-champions"),
        }
    }
}
