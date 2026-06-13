//! Spell resolution: the within-wave Instant resolver, the face-down Active
//! prime/fire lifecycle, and the resolution-time fires (wards and Hex on an
//! explosion, Harvest on a won pot).
//!
//! Visibility contract: an **Instant** activates at the wave reveal and is
//! visible to the table (the `casts` list, in resolution order); an **Active**
//! primes silently and is disclosed only when it fires (the [`SpellFire`]
//! narration). A volatility spell's *delta* is implied by its kind — absolute
//! cauldron totals are never disclosed (blind volatility holds).
//!
//! Within one wave, Instants resolve in a fixed category order (economy →
//! score → volatility → information), seat-arrival order within a category, so
//! resolution is deterministic and same-wave score spells read the same
//! pre-wave snapshot (order-independent, summing duplicates).

use std::collections::HashMap;

use boiling_point_protocol::PlayerId;
use boiling_point_protocol::server::SpellFire;
use boiling_point_protocol::vocab::{Color, IngredientView, SpellKind, SpellMode, SpellTarget};

use crate::content::spell::SpellValues;

use super::card::{Ingredient, Spell};
use super::pot::Pot;

/// A validated spell commit for one wave (the engine receives at most one per
/// player per wave; the session enforces the limit on the wire).
#[derive(Debug, Clone, Copy)]
pub struct CastCommit {
    /// Who cast it.
    pub player: PlayerId,
    /// The spell instance (already removed from the caster's hand).
    pub spell: Spell,
    /// The validated target, when the spell requires one.
    pub target: Option<SpellTarget>,
}

/// An Active spell primed face-down, waiting for its trigger. Hidden from the
/// table until it fires; spent unfired at round end (a wasted bet).
#[derive(Debug, Clone, Copy)]
pub struct PrimedSpell {
    /// Who primed it.
    pub player: PlayerId,
    /// The spell instance.
    pub spell: Spell,
    /// The player it is aimed at (Redirect, Hex).
    pub target: Option<PlayerId>,
    /// Whether it has fired (each primed spell fires at most once).
    pub fired: bool,
}

/// What resolving one wave's spell commits produced, for the presenter.
#[derive(Debug, Default)]
pub struct WaveSpellOutcome {
    /// Visible Instant activations, in resolution order (caster, spell, and the
    /// colour target for colour-aimed spells).
    pub casts: Vec<(PlayerId, SpellKind, Option<Color>)>,
    /// Casters who should privately receive the boiling point (Peek).
    pub peeked: Vec<PlayerId>,
    /// Private Assay reads: (caster, dominant colour, point lead).
    pub assays: Vec<(PlayerId, Option<Color>, u32)>,
    /// Ingredients revealed to the whole table by Expose: (owner, view, colorless).
    pub exposed: Vec<(PlayerId, IngredientView, bool)>,
    /// Casters owed a Forage draw (the caller draws from their grimoire).
    pub foragers: Vec<PlayerId>,
    /// Ingredients Skim pulled out of the pot (returned to pantries at settle).
    pub skimmed: Vec<(PlayerId, Ingredient)>,
    /// Whether a Quench fired: the next wave cannot explode.
    pub quenched: bool,
}

/// The fixed within-wave resolution rank for Instants. Lower resolves first;
/// information reads come last so they see the wave's settled state.
fn rank(kind: SpellKind) -> u8 {
    match kind {
        SpellKind::Forage => 0,
        SpellKind::Skim => 1,
        SpellKind::DoubleDown | SpellKind::Sour => 2,
        SpellKind::Dampen | SpellKind::Surge | SpellKind::Quench => 3,
        SpellKind::Peek | SpellKind::Assay | SpellKind::Expose => 4,
        // Actives prime after all Instants (order among primes is irrelevant).
        _ => 5,
    }
}

/// Resolve one wave's spell commits against the pot. `snapshot_points` is the
/// per-colour point totals frozen *before* this wave's ingredients landed —
/// what Double Down / Sour read, so same-wave duplicates sum order-independently.
pub fn resolve_wave_spells(
    pot: &mut Pot,
    snapshot_points: &HashMap<Color, u32>,
    mut casts: Vec<CastCommit>,
    primed: &mut Vec<PrimedSpell>,
    values: &SpellValues,
) -> WaveSpellOutcome {
    let mut out = WaveSpellOutcome::default();
    casts.sort_by_key(|c| rank(c.spell.kind));

    for cast in casts {
        let kind = cast.spell.kind;
        if kind.mode() == SpellMode::Active {
            let target = match cast.target {
                Some(SpellTarget::Player { player }) => Some(player),
                _ => None,
            };
            primed.push(PrimedSpell {
                player: cast.player,
                spell: cast.spell,
                target,
                fired: false,
            });
            continue; // primes are silent — no visible activation
        }

        let color_target = match cast.target {
            Some(SpellTarget::Color { color }) => Some(color),
            _ => None,
        };
        out.casts.push((cast.player, kind, color_target));
        match kind {
            SpellKind::Peek => out.peeked.push(cast.player),
            SpellKind::Assay => {
                let (dominant, lead) = assay(pot);
                out.assays.push((cast.player, dominant, lead));
            }
            SpellKind::Expose => {
                if let Some(pc) = pot.cards.iter_mut().find(|p| !p.exposed) {
                    pc.exposed = true;
                    out.exposed
                        .push((pc.player, pc.ingredient.view(), pc.colorless));
                }
            }
            SpellKind::Dampen => pot.spell_volatility_delta -= values.dampen as i32,
            SpellKind::Surge => pot.spell_volatility_delta += values.surge as i32,
            SpellKind::Quench => out.quenched = true,
            SpellKind::DoubleDown => {
                if let Some(color) = color_target {
                    let snap = snapshot_points.get(&color).copied().unwrap_or(0);
                    *pot.color_adjust.entry(color).or_insert(0) += snap as i64;
                }
            }
            SpellKind::Sour => {
                if let Some(color) = color_target {
                    let snap = snapshot_points.get(&color).copied().unwrap_or(0);
                    // Post-Sour the colour reads floor(pre/2): subtract ceil(pre/2).
                    *pot.color_adjust.entry(color).or_insert(0) -= snap.div_ceil(2) as i64;
                }
            }
            SpellKind::Skim => {
                if let Some(removed) = pot.remove_last_of(cast.player) {
                    out.skimmed.push((removed.player, removed.ingredient));
                }
            }
            SpellKind::Forage => out.foragers.push(cast.player),
            // Actives were handled above.
            SpellKind::Cap
            | SpellKind::Halve
            | SpellKind::Redirect
            | SpellKind::Harvest
            | SpellKind::Hex => unreachable!("actives prime, never resolve as instants"),
        }
    }
    out
}

/// The Assay read: the dominant colour by current points and its lead over the
/// runner-up. `None` dominance when no colored Votes are in yet; ties read as
/// the first colour in the fixed colour order with a lead of zero.
fn assay(pot: &Pot) -> (Option<Color>, u32) {
    let mut totals: Vec<(Color, u32)> = Color::PLAYER_COLORS
        .into_iter()
        .filter(|c| pot.color_present(*c))
        .map(|c| (c, pot.color_points(c)))
        .collect();
    if totals.is_empty() {
        return (None, 0);
    }
    totals.sort_by_key(|t| std::cmp::Reverse(t.1));
    let lead = totals[0].1 - totals.get(1).map(|t| t.1).unwrap_or(0);
    (Some(totals[0].0), lead)
}

/// Apply detonation damage of `amount` to `player`, consulting their primed
/// wards: the earliest-primed unfired ward fires (and is consumed); Redirect
/// transfers the full amount to its target and **cascades** through the
/// target's own wards. Cycles terminate because each ward fires at most once.
fn apply_damage(
    player: PlayerId,
    amount: u32,
    values: &SpellValues,
    primed: &mut [PrimedSpell],
    deltas: &mut HashMap<PlayerId, i32>,
    fired: &mut Vec<SpellFire>,
) {
    let mut current = player;
    loop {
        let ward = primed.iter_mut().find(|p| {
            !p.fired
                && p.player == current
                && matches!(
                    p.spell.kind,
                    SpellKind::Cap | SpellKind::Halve | SpellKind::Redirect
                )
        });
        let Some(ward) = ward else {
            *deltas.entry(current).or_insert(0) -= amount as i32;
            return;
        };
        ward.fired = true;
        let kind = ward.spell.kind;
        let target = ward.target;
        fired.push(SpellFire {
            player: current,
            spell: kind,
            target,
        });
        match kind {
            SpellKind::Cap => {
                *deltas.entry(current).or_insert(0) -= amount.min(values.cap_max as u32) as i32;
                return;
            }
            SpellKind::Halve => {
                *deltas.entry(current).or_insert(0) -= (amount / 2) as i32;
                return;
            }
            SpellKind::Redirect => match target {
                // The full loss moves to the target, whose own wards now apply.
                Some(next) => current = next,
                // A target is validated at cast; a missing one degrades to no ward.
                None => {
                    *deltas.entry(current).or_insert(0) -= amount as i32;
                    return;
                }
            },
            _ => unreachable!("only wards are matched above"),
        }
    }
}

/// Resolve an explosion's damage: the detonators split −P equally (rounded
/// down, integer-only), modified per player by wards (Cap / Halve / Redirect
/// with cascade); then every primed Hex fires — its target takes the extra
/// loss, detonator or not, unwardable. A detonator in `cinderwrights` takes
/// half their share rounded up (`boom2-brewers` — the discipline's hard
/// ceiling; their ward-free grimoire means no ward then applies). Returns
/// per-player deltas and the fired narration in fire order.
pub fn resolve_explosion(
    detonators: &[PlayerId],
    pot_value: u32,
    values: &SpellValues,
    primed: &mut [PrimedSpell],
    cinderwrights: &std::collections::HashSet<PlayerId>,
) -> (HashMap<PlayerId, i32>, Vec<SpellFire>) {
    use boiling_point_protocol::vocab::Brewer;

    let mut deltas: HashMap<PlayerId, i32> = HashMap::new();
    let mut fired: Vec<SpellFire> = Vec::new();

    if !detonators.is_empty() {
        let share = pot_value / detonators.len() as u32;
        for d in detonators {
            let brewer = cinderwrights.contains(d).then_some(Brewer::Cinderwright);
            let amount = super::brewers::detonator_damage(brewer, share);
            apply_damage(*d, amount, values, primed, &mut deltas, &mut fired);
        }
    }

    // Hex: +extra on ANY explosion this round, aimed damage, not wardable.
    for hex in primed
        .iter_mut()
        .filter(|p| !p.fired && p.spell.kind == SpellKind::Hex)
    {
        hex.fired = true;
        fired.push(SpellFire {
            player: hex.player,
            spell: SpellKind::Hex,
            target: hex.target,
        });
        if let Some(target) = hex.target {
            *deltas.entry(target).or_insert(0) -= values.hex_bonus as i32;
        }
    }

    (deltas, fired)
}

/// Resolve Harvests on a safe brew: each primed Harvest whose caster's colour
/// won the pot **and** who actually took a share fires for the bonus.
pub fn resolve_harvests(
    winners: &[Color],
    awards: &HashMap<PlayerId, i32>,
    player_color: &HashMap<PlayerId, Color>,
    values: &SpellValues,
    primed: &mut [PrimedSpell],
) -> (HashMap<PlayerId, i32>, Vec<SpellFire>) {
    let mut bonuses: HashMap<PlayerId, i32> = HashMap::new();
    let mut fired: Vec<SpellFire> = Vec::new();
    for harvest in primed
        .iter_mut()
        .filter(|p| !p.fired && p.spell.kind == SpellKind::Harvest)
    {
        let won = player_color
            .get(&harvest.player)
            .is_some_and(|c| winners.contains(c));
        let took_share = awards.get(&harvest.player).copied().unwrap_or(0) > 0;
        if won && took_share {
            harvest.fired = true;
            fired.push(SpellFire {
                player: harvest.player,
                spell: SpellKind::Harvest,
                target: None,
            });
            *bonuses.entry(harvest.player).or_insert(0) += values.harvest_bonus as i32;
        }
    }
    (bonuses, fired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::CardId;
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn values() -> SpellValues {
        SpellValues {
            dampen: 3,
            surge: 3,
            cap_max: 3,
            hex_bonus: 5,
            harvest_bonus: 3,
            forage_draws: 2,
        }
    }

    fn spell(id: u32, kind: SpellKind) -> Spell {
        Spell {
            id: CardId(id),
            kind,
        }
    }

    fn primed(player: PlayerId, id: u32, kind: SpellKind, target: Option<PlayerId>) -> PrimedSpell {
        PrimedSpell {
            player,
            spell: spell(id, kind),
            target,
            fired: false,
        }
    }

    fn pot_with_ruby(points: u32) -> (Pot, HashMap<Color, u32>) {
        let mut pot = Pot::new(0);
        pot.cards.push(super::super::pot::PotIngredient {
            player: pid(1),
            ingredient: Ingredient {
                id: CardId(900),
                color: Color::Ruby,
                volatility: 2,
                points: points as u8,
                compounding: None,
            },
            colorless: false,
            wave_number: 1,
            exposed: false,
            compounding: super::super::compounding::CardCompounding::default(),
        });
        let snapshot: HashMap<Color, u32> = Color::PLAYER_COLORS
            .into_iter()
            .map(|c| (c, pot.color_points(c)))
            .collect();
        (pot, snapshot)
    }

    /// Dampen and Surge adjust the cauldron-level delta, never a card.
    #[test]
    fn volatility_spells_adjust_the_running_total() {
        let (mut pot, snap) = pot_with_ruby(1);
        let mut pr = Vec::new();
        let out = resolve_wave_spells(
            &mut pot,
            &snap,
            vec![
                CastCommit {
                    player: pid(1),
                    spell: spell(1, SpellKind::Surge),
                    target: None,
                },
                CastCommit {
                    player: pid(2),
                    spell: spell(2, SpellKind::Dampen),
                    target: None,
                },
            ],
            &mut pr,
            &values(),
        );
        assert_eq!(pot.spell_volatility_delta, 0); // +3 −3
        assert_eq!(pot.cards[0].effective_volatility(), 2); // card untouched
        assert_eq!(out.casts.len(), 2);
    }

    /// Two same-colour Double Downs both read the pre-wave snapshot and sum
    /// (3 → 9, a triple), order-independent.
    #[test]
    fn two_double_downs_sum_to_triple() {
        let (mut pot, snap) = pot_with_ruby(3);
        let mut pr = Vec::new();
        let target = Some(SpellTarget::Color { color: Color::Ruby });
        resolve_wave_spells(
            &mut pot,
            &snap,
            vec![
                CastCommit {
                    player: pid(1),
                    spell: spell(1, SpellKind::DoubleDown),
                    target,
                },
                CastCommit {
                    player: pid(2),
                    spell: spell(2, SpellKind::DoubleDown),
                    target,
                },
            ],
            &mut pr,
            &values(),
        );
        assert_eq!(pot.color_points(Color::Ruby), 9);
    }

    /// Sour halves a colour: post = floor(pre/2).
    #[test]
    fn sour_halves_a_color() {
        let (mut pot, snap) = pot_with_ruby(3);
        let mut pr = Vec::new();
        resolve_wave_spells(
            &mut pot,
            &snap,
            vec![CastCommit {
                player: pid(2),
                spell: spell(1, SpellKind::Sour),
                target: Some(SpellTarget::Color { color: Color::Ruby }),
            }],
            &mut pr,
            &values(),
        );
        assert_eq!(pot.color_points(Color::Ruby), 1); // floor(3/2)
    }

    /// An Active primes silently: no visible cast, present in the primed list.
    #[test]
    fn actives_prime_silently() {
        let (mut pot, snap) = pot_with_ruby(1);
        let mut pr = Vec::new();
        let out = resolve_wave_spells(
            &mut pot,
            &snap,
            vec![CastCommit {
                player: pid(1),
                spell: spell(1, SpellKind::Cap),
                target: None,
            }],
            &mut pr,
            &values(),
        );
        assert!(out.casts.is_empty(), "a prime must not be visible");
        assert_eq!(pr.len(), 1);
        assert!(!pr[0].fired);
    }

    /// Wards modify detonation damage: Cap eats at most 3, Halve halves,
    /// Redirect shoves the full loss onto its target and cascades.
    #[test]
    fn wards_cap_halve_and_redirect_with_cascade() {
        let v = values();
        // p1 redirects to p2; p2 halves; p3 caps; p4 is bare.
        let mut pr = vec![
            primed(pid(1), 1, SpellKind::Redirect, Some(pid(2))),
            primed(pid(2), 2, SpellKind::Halve, None),
            primed(pid(3), 3, SpellKind::Cap, None),
        ];
        let (deltas, fired) = resolve_explosion(
            &[pid(1), pid(3), pid(4)],
            12,
            &v,
            &mut pr,
            &Default::default(),
        );
        // Share = 12 / 3 = 4 each. p1 → redirect → p2's halve → p2 loses 2.
        assert_eq!(deltas.get(&pid(1)).copied().unwrap_or(0), 0);
        assert_eq!(deltas[&pid(2)], -2);
        assert_eq!(deltas[&pid(3)], -3); // capped at 3 (share was 4)
        assert_eq!(deltas[&pid(4)], -4); // bare
        assert_eq!(fired.len(), 3); // redirect, halve, cap all narrated
    }

    /// A Redirect cycle terminates: each ward fires once, so the loss lands.
    #[test]
    fn redirect_cycle_terminates() {
        let v = values();
        let mut pr = vec![
            primed(pid(1), 1, SpellKind::Redirect, Some(pid(2))),
            primed(pid(2), 2, SpellKind::Redirect, Some(pid(1))),
        ];
        let (deltas, fired) = resolve_explosion(&[pid(1)], 10, &v, &mut pr, &Default::default());
        // p1 → p2 → back to p1 (both redirects spent) → p1 eats it.
        assert_eq!(deltas[&pid(1)], -10);
        assert_eq!(fired.len(), 2);
    }

    /// Hex fires on any explosion — extra aimed damage, detonator or not, and
    /// not wardable.
    #[test]
    fn hex_adds_aimed_damage() {
        let v = values();
        let mut pr = vec![
            primed(pid(1), 1, SpellKind::Hex, Some(pid(3))),
            primed(pid(3), 2, SpellKind::Cap, None), // must NOT absorb the hex
        ];
        let (deltas, fired) = resolve_explosion(&[pid(2)], 8, &v, &mut pr, &Default::default());
        assert_eq!(deltas[&pid(2)], -8);
        assert_eq!(deltas[&pid(3)], -5);
        assert!(fired.iter().any(|f| f.spell == SpellKind::Hex));
        assert!(
            !fired.iter().any(|f| f.spell == SpellKind::Cap),
            "hex damage is not wardable"
        );
    }

    /// The Cinderwright bend (`boom2-brewers`): a detonator in the set takes
    /// half their share rounded UP — the discipline's ceiling (never more than
    /// half off, never immunity); others split normally.
    #[test]
    fn cinderwright_detonator_takes_ceil_half() {
        let v = values();
        let mut pr = Vec::new();
        let cinderwrights = std::collections::HashSet::from([pid(1)]);
        // Share = 9 / 2 = 4 each; the Cinderwright takes ceil(4/2) = 2.
        let (deltas, _) = resolve_explosion(&[pid(1), pid(2)], 9, &v, &mut pr, &cinderwrights);
        assert_eq!(deltas[&pid(1)], -2);
        assert_eq!(deltas[&pid(2)], -4);
        // A share of 1 still costs 1 — never a free explosion.
        let (deltas, _) = resolve_explosion(&[pid(1)], 1, &v, &mut Vec::new(), &cinderwrights);
        assert_eq!(deltas[&pid(1)], -1);
    }

    /// An explosion with no fatal-wave cards (e.g. a parting Surge) has no
    /// detonator: nobody pays the pot, but Hexes still fire.
    #[test]
    fn empty_detonator_set_costs_nobody_the_pot() {
        let v = values();
        let mut pr = vec![primed(pid(1), 1, SpellKind::Hex, Some(pid(2)))];
        let (deltas, _) = resolve_explosion(&[], 10, &v, &mut pr, &Default::default());
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[&pid(2)], -5);
    }

    /// Harvest pays its bonus only to a winner who actually took a share.
    #[test]
    fn harvest_fires_only_on_a_taken_win() {
        let v = values();
        let player_color: HashMap<PlayerId, Color> =
            [(pid(1), Color::Ruby), (pid(2), Color::Sapphire)].into();
        let mut pr = vec![
            primed(pid(1), 1, SpellKind::Harvest, None),
            primed(pid(2), 2, SpellKind::Harvest, None),
        ];
        let awards: HashMap<PlayerId, i32> = [(pid(1), 7), (pid(2), 0)].into();
        let (bonuses, fired) =
            resolve_harvests(&[Color::Ruby], &awards, &player_color, &v, &mut pr);
        assert_eq!(bonuses.get(&pid(1)), Some(&3));
        assert!(
            !bonuses.contains_key(&pid(2)),
            "a loser's harvest stays primed"
        );
        assert_eq!(fired.len(), 1);
        assert!(!pr[1].fired);
    }
}
