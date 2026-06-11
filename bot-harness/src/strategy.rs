//! Pluggable bot decision-making (D3).
//!
//! Every decision a bot makes in a wave — play which ingredient (as a Vote or
//! colorless), or pass, plus the optional ≤1 spell and the optional table-talk
//! emote — is a method on the [`Strategy`] trait, a pure function of the bot's
//! [`PlayerView`] and its seeded RNG. Purity under a seed is what keeps a whole
//! batch reproducible (D4): a strategy may consult only the player-visible view
//! and the RNG handed to it, never the wall clock or any ambient state.
//!
//! Four baselines ship for comparison — [`Cautious`], [`Aggressor`],
//! [`Diplomat`], and [`Random`]. They are deliberately simple starting
//! hypotheses that nonetheless exercise every v2 system (votes, colorless
//! pushes, Peek, wards, volatility and score spells), so harness statistics
//! cover the whole combat core. The trait exists so sharper strategies can be
//! slotted in as balance understanding grows.

use rand::Rng;
use rand::rngs::StdRng;

use boiling_point_protocol::vocab::{HandIngredient, SpellKind, SpellTarget, TargetKind};
use boiling_point_protocol::{CardId, EmoteId, PlayerId};

use crate::model::PlayerView;

/// A bot's mandatory ingredient-or-pass choice for a wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveAction {
    /// Commit a specific ingredient from hand into the cauldron.
    Play {
        /// The hand ingredient.
        card: CardId,
        /// Play it colorless (volatility only, zero points).
        colorless: bool,
    },
    /// Pass — a permanent lockout for the rest of the round.
    Pass,
}

/// A bot's optional spell cast for a wave (≤1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellPlay {
    /// The grimoire spell to cast.
    pub spell: CardId,
    /// The target, when the spell requires one.
    pub target: Option<SpellTarget>,
}

/// A bot's full decision for one wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveDecision {
    /// The mandatory ingredient-or-pass.
    pub action: WaveAction,
    /// The optional spell.
    pub spell: Option<SpellPlay>,
}

impl WaveDecision {
    /// A bare pass with no spell.
    pub fn pass() -> Self {
        WaveDecision {
            action: WaveAction::Pass,
            spell: None,
        }
    }
}

/// A play pattern: how a bot decides each wave. Pure in `(view, rng)`.
pub trait Strategy: Send + Sync {
    /// A stable, human-readable name (used to attribute wins in the report).
    fn name(&self) -> &'static str;

    /// Choose this wave's decision from the player-visible view and the seeded
    /// RNG.
    ///
    /// Called only when the bot is still active (has ingredients and has not
    /// passed), so implementations always have at least one ingredient to
    /// consider.
    fn decide(&self, view: &PlayerView, rng: &mut StdRng) -> WaveDecision;

    /// Optionally send a preset emote (the only comms channel). Cosmetic — it
    /// never affects game state — so the default is silence.
    fn emote(
        &self,
        _view: &PlayerView,
        _palette: &[EmoteId],
        _rng: &mut StdRng,
    ) -> Option<EmoteId> {
        None
    }
}

/// The safety margin (in estimated volatility) a careful bot keeps under a
/// *known* boiling point before bailing. `[needs playtesting]`.
const KNOWN_BP_MARGIN: f64 = 5.0;
/// Estimated volatility at or above which careful bots treat a *blind* pot as
/// explosion-risky — a conservative read of the boiling-point band, kept below
/// its low edge. `[needs playtesting]`.
const BLIND_RISK_ESTIMATE: f64 = 18.0;

/// Whether the bot owns (scores for) an ingredient's colour.
fn is_mine(view: &PlayerView, card: &HandIngredient) -> bool {
    card.view.color == view.my_color
}

/// The bot's own-colour Vote that scores best for the least risk: highest
/// points, then lowest volatility, then lowest id (a deterministic tiebreak).
fn best_scoring_vote(view: &PlayerView) -> Option<CardId> {
    view.ingredients
        .iter()
        .filter(|c| is_mine(view, c))
        .max_by(|a, b| {
            a.view
                .points
                .cmp(&b.view.points)
                .then(b.view.volatility.cmp(&a.view.volatility)) // prefer lower volatility
                .then(b.id.0.cmp(&a.id.0)) // prefer lower id
        })
        .map(|c| c.id)
}

/// The lowest-volatility ingredient in hand, preferring own colour then higher
/// points, then lower id. The cautious "tread carefully" pick.
fn safest_card(view: &PlayerView) -> Option<&HandIngredient> {
    view.ingredients.iter().min_by(|a, b| {
        a.view
            .volatility
            .cmp(&b.view.volatility)
            .then(is_mine(view, b).cmp(&is_mine(view, a))) // prefer own colour
            .then(b.view.points.cmp(&a.view.points)) // prefer more points
            .then(a.id.0.cmp(&b.id.0))
    })
}

/// The highest-points ingredient in hand regardless of colour (the aggressor
/// fallback).
fn highest_points_card(view: &PlayerView) -> Option<CardId> {
    view.ingredients
        .iter()
        .max_by(|a, b| {
            a.view
                .points
                .cmp(&b.view.points)
                .then(a.view.volatility.cmp(&b.view.volatility)) // aggressor likes volatility
                .then(b.id.0.cmp(&a.id.0))
        })
        .map(|c| c.id)
}

/// The first held spell of a given kind, if any (lowest id, deterministic).
fn spell_of(view: &PlayerView, kind: SpellKind) -> Option<CardId> {
    view.spells
        .iter()
        .filter(|s| s.kind == kind)
        .min_by_key(|s| s.id.0)
        .map(|s| s.id)
}

/// A deterministic non-self player target (the next seat in table order).
fn next_opponent(view: &PlayerView) -> Option<PlayerId> {
    let seat = view.players.iter().position(|p| p.id == view.me)?;
    (1..view.players.len())
        .map(|i| view.players[(seat + i) % view.players.len()].id)
        .next()
}

/// Whether the pot currently reads as explosion-risky to a careful bot: against
/// the known boiling point when Peeked, against a blind estimate otherwise. A
/// quench-shielded wave is never risky.
fn pot_is_risky(view: &PlayerView, next_card_volatility: f64) -> bool {
    if view.quench_shielded_wave == Some(view.wave_number) {
        return false;
    }
    let projected = view.estimated_volatility() + next_card_volatility;
    match view.known_boiling_point() {
        Some(bp) => projected > bp as f64 - KNOWN_BP_MARGIN,
        None => projected > BLIND_RISK_ESTIMATE,
    }
}

/// A held ward, preferring Cap then Halve then Redirect, with a valid target.
fn ward_play(view: &PlayerView) -> Option<SpellPlay> {
    for kind in [SpellKind::Cap, SpellKind::Halve, SpellKind::Redirect] {
        if let Some(spell) = spell_of(view, kind) {
            let target = match kind.target_kind() {
                TargetKind::Player => Some(SpellTarget::Player {
                    player: next_opponent(view)?,
                }),
                _ => None,
            };
            return Some(SpellPlay { spell, target });
        }
    }
    None
}

/// Plays low and bails early: Peeks for certainty, sheds its safest card while
/// the pot is calm, primes a ward before risky pushes, and passes once the pot
/// reads dangerous (or it is comfortably ahead).
pub struct Cautious;

impl Strategy for Cautious {
    fn name(&self) -> &'static str {
        "cautious"
    }

    fn decide(&self, view: &PlayerView, _rng: &mut StdRng) -> WaveDecision {
        // Buy certainty early: Peek on the first wave it is held.
        let spell = if view.known_boiling_point().is_none() {
            spell_of(view, SpellKind::Peek).map(|spell| SpellPlay {
                spell,
                target: None,
            })
        } else {
            None
        };

        let safest = safest_card(view);
        let next_vol = safest.map(|c| c.view.volatility as f64).unwrap_or(0.0);
        let ahead = matches!(
            (view.my_score(), view.best_opponent_score()),
            (Some(me), Some(best)) if me > best
        );
        if pot_is_risky(view, next_vol) || (ahead && view.cauldron_card_count >= 6) {
            // Bail — a ward on the way out would be wasted (folded players are
            // exempt), so just pass. The Peek still rides if chosen.
            return WaveDecision {
                action: WaveAction::Pass,
                spell,
            };
        }
        // Staying in a fattening pot: prime a ward if no Peek rode this wave.
        let spell = spell.or_else(|| {
            if view.cauldron_card_count >= 4 {
                ward_play(view)
            } else {
                None
            }
        });
        match safest {
            Some(card) => WaveDecision {
                action: WaveAction::Play {
                    card: card.id,
                    colorless: false,
                },
                spell,
            },
            None => WaveDecision {
                action: WaveAction::Pass,
                spell,
            },
        }
    }
}

/// Pushes the pot: plays its biggest-scoring Vote every wave and effectively
/// never passes; Doubles Down on its own colour once the pot is worth it.
/// Stress-tests whether reckless play is over-rewarded under detonator-only
/// losses.
pub struct Aggressor;

impl Strategy for Aggressor {
    fn name(&self) -> &'static str {
        "aggressor"
    }

    fn decide(&self, view: &PlayerView, _rng: &mut StdRng) -> WaveDecision {
        let spell = if view.cauldron_card_count >= 3 {
            spell_of(view, SpellKind::DoubleDown).map(|spell| SpellPlay {
                spell,
                target: Some(SpellTarget::Color {
                    color: view.my_color,
                }),
            })
        } else {
            None
        };
        let action = best_scoring_vote(view)
            .or_else(|| highest_points_card(view))
            .map(|card| WaveAction::Play {
                card,
                colorless: false,
            })
            .unwrap_or(WaveAction::Pass);
        WaveDecision { action, spell }
    }
}

/// Adapts to the table: Peeks or Assays for information, wards up when the pot
/// turns dangerous, presses for points when behind, plays its toolkit colorless
/// rather than gift a rival points, and folds when exposed.
pub struct Diplomat;

impl Strategy for Diplomat {
    fn name(&self) -> &'static str {
        "diplomat"
    }

    fn decide(&self, view: &PlayerView, _rng: &mut StdRng) -> WaveDecision {
        // Information first: an early Peek, else an Assay read mid-round.
        let info_spell = if view.wave_number <= 1 && view.known_boiling_point().is_none() {
            spell_of(view, SpellKind::Peek).map(|spell| SpellPlay {
                spell,
                target: None,
            })
        } else if view.wave_number == 2 {
            spell_of(view, SpellKind::Assay).map(|spell| SpellPlay {
                spell,
                target: None,
            })
        } else {
            None
        };

        let safest = safest_card(view).copied();
        let next_vol = safest.map(|c| c.view.volatility as f64).unwrap_or(0.0);
        if pot_is_risky(view, next_vol) {
            // Danger: stay in only behind a ward (or a Dampen); otherwise fold.
            if let (Some(ward), Some(card)) = (ward_play(view), safest) {
                return WaveDecision {
                    action: WaveAction::Play {
                        card: card.id,
                        colorless: false,
                    },
                    spell: Some(ward),
                };
            }
            if let (Some(dampen), Some(card)) = (spell_of(view, SpellKind::Dampen), safest) {
                return WaveDecision {
                    action: WaveAction::Play {
                        card: card.id,
                        colorless: false,
                    },
                    spell: Some(SpellPlay {
                        spell: dampen,
                        target: None,
                    }),
                };
            }
            return WaveDecision {
                action: WaveAction::Pass,
                spell: info_spell,
            };
        }

        let behind = matches!(
            (view.my_score(), view.best_opponent_score()),
            (Some(me), Some(best)) if me < best
        );
        // Behind: press with the best Vote. Ahead/level: tread carefully, and
        // play an off-colour pick colorless rather than gift a rival its points.
        let action = if behind {
            best_scoring_vote(view)
                .or_else(|| safest.map(|c| c.id))
                .map(|card| WaveAction::Play {
                    card,
                    colorless: false,
                })
                .unwrap_or(WaveAction::Pass)
        } else {
            match safest {
                Some(card) => WaveAction::Play {
                    card: card.id,
                    colorless: !is_mine(view, &card),
                },
                None => WaveAction::Pass,
            }
        };
        WaveDecision {
            action,
            spell: info_spell,
        }
    }

    fn emote(&self, _view: &PlayerView, palette: &[EmoteId], rng: &mut StdRng) -> Option<EmoteId> {
        // An occasional, harmless table-talk emote — purely to exercise the channel.
        if palette.is_empty() || !rng.gen_bool(0.1) {
            return None;
        }
        Some(palette[rng.gen_range(0..palette.len())])
    }
}

/// Uniformly random over {pass} ∪ {play each held ingredient} (colorless on a
/// coin flip), plus an occasional random legal spell — the noise floor every
/// other strategy must beat to justify its heuristics.
pub struct Random;

impl Strategy for Random {
    fn name(&self) -> &'static str {
        "random"
    }

    fn decide(&self, view: &PlayerView, rng: &mut StdRng) -> WaveDecision {
        // Options: pass (index 0) or play hand[i] (index i+1).
        let choice = rng.gen_range(0..=view.ingredients.len());
        let action = if choice == 0 {
            WaveAction::Pass
        } else {
            WaveAction::Play {
                card: view.ingredients[choice - 1].id,
                colorless: rng.gen_bool(0.25),
            }
        };
        // 30% of waves: cast a random held spell with a deterministic legal target.
        let spell = if !view.spells.is_empty() && rng.gen_bool(0.3) {
            let pick = view.spells[rng.gen_range(0..view.spells.len())];
            let target = match pick.kind.target_kind() {
                TargetKind::None => None,
                TargetKind::Color => Some(SpellTarget::Color {
                    color: boiling_point_protocol::vocab::Color::PLAYER_COLORS[rng.gen_range(0..4)],
                }),
                TargetKind::Player => {
                    next_opponent(view).map(|player| SpellTarget::Player { player })
                }
            };
            // A player-targeted spell with no available target is skipped.
            match (pick.kind.target_kind(), &target) {
                (TargetKind::Player, None) => None,
                _ => Some(SpellPlay {
                    spell: pick.id,
                    target,
                }),
            }
        } else {
            None
        };
        WaveDecision { action, spell }
    }
}

/// Construct a baseline strategy by name, or `None` if the name is unknown.
pub fn by_name(name: &str) -> Option<Box<dyn Strategy>> {
    match name {
        "cautious" => Some(Box::new(Cautious)),
        "aggressor" => Some(Box::new(Aggressor)),
        "diplomat" => Some(Box::new(Diplomat)),
        "random" => Some(Box::new(Random)),
        _ => None,
    }
}

/// The names of every baseline strategy, in a stable order.
pub const BASELINE_NAMES: [&str; 4] = ["cautious", "aggressor", "diplomat", "random"];

/// Build a per-seat strategy assignment from four names (one per seat). Returns
/// the first unknown name as an error.
pub fn assignment_from_names(names: &[String]) -> Result<Vec<Box<dyn Strategy>>, String> {
    names
        .iter()
        .map(|n| by_name(n).ok_or_else(|| format!("unknown strategy: {n}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::server::PlayerPublic;
    use boiling_point_protocol::vocab::{Color, HandSpell, IngredientView};
    use uuid::Uuid;

    fn view_with(hand: Vec<HandIngredient>, cauldron: u8) -> PlayerView {
        let mut v = PlayerView::new(PlayerId(Uuid::from_u128(1)), Color::Ruby);
        v.players = (1..=4u128)
            .map(|n| PlayerPublic {
                id: PlayerId(Uuid::from_u128(n)),
                display_name: format!("p{n}"),
                color: Color::PLAYER_COLORS[(n - 1) as usize],
                connected: true,
                guest: false,
            })
            .collect();
        v.ingredients = hand;
        v.cauldron_card_count = cauldron;
        v.wave_number = 2;
        v
    }

    fn card(id: u32, color: Color, vol: u8, pts: u8) -> HandIngredient {
        HandIngredient {
            id: CardId(id),
            view: IngredientView {
                color,
                volatility: vol,
                points: pts,
            },
        }
    }

    fn spell(id: u32, kind: SpellKind) -> HandSpell {
        HandSpell {
            id: CardId(id),
            kind,
        }
    }

    fn rng() -> StdRng {
        use rand::SeedableRng;
        StdRng::seed_from_u64(1)
    }

    /// The cautious bot bails out of a pot it estimates as risky.
    #[test]
    fn cautious_passes_a_risky_pot() {
        // 9 cards ≈ 21.6 estimated volatility > the blind risk line.
        let v = view_with(vec![card(1, Color::Ruby, 1, 2)], 9);
        let d = Cautious.decide(&v, &mut rng());
        assert_eq!(d.action, WaveAction::Pass);
    }

    /// The cautious bot sheds its lowest-volatility card in a calm pot.
    #[test]
    fn cautious_plays_safest_card() {
        let v = view_with(
            vec![card(1, Color::Ruby, 5, 3), card(2, Color::Ruby, 1, 1)],
            1,
        );
        let d = Cautious.decide(&v, &mut rng());
        assert_eq!(
            d.action,
            WaveAction::Play {
                card: CardId(2),
                colorless: false
            }
        );
    }

    /// The cautious bot trusts a Peeked boiling point over the blind estimate.
    #[test]
    fn cautious_uses_known_boiling_point() {
        // 9 cards reads risky blind — but a known high BP says it is fine.
        let mut v = view_with(vec![card(1, Color::Ruby, 1, 2)], 9);
        v.disclose_boiling_point(32);
        let d = Cautious.decide(&v, &mut rng());
        assert!(matches!(d.action, WaveAction::Play { .. }));
    }

    /// The cautious bot buys certainty: it casts a held Peek while blind.
    #[test]
    fn cautious_casts_peek_when_blind() {
        let mut v = view_with(vec![card(1, Color::Ruby, 1, 2)], 1);
        v.spells = vec![spell(10, SpellKind::Peek)];
        let d = Cautious.decide(&v, &mut rng());
        assert_eq!(
            d.spell,
            Some(SpellPlay {
                spell: CardId(10),
                target: None
            })
        );
    }

    /// The aggressor maximises its own-colour points.
    #[test]
    fn aggressor_plays_best_scoring_vote() {
        let v = view_with(
            vec![card(1, Color::Ruby, 1, 1), card(2, Color::Ruby, 3, 3)],
            1,
        );
        let d = Aggressor.decide(&v, &mut rng());
        assert_eq!(
            d.action,
            WaveAction::Play {
                card: CardId(2),
                colorless: false
            }
        );
    }

    /// The diplomat wards up to stay in a dangerous pot.
    #[test]
    fn diplomat_wards_when_risky() {
        let mut v = view_with(vec![card(1, Color::Ruby, 1, 2)], 9);
        v.spells = vec![spell(11, SpellKind::Cap)];
        let d = Diplomat.decide(&v, &mut rng());
        assert!(matches!(d.action, WaveAction::Play { .. }));
        assert_eq!(
            d.spell,
            Some(SpellPlay {
                spell: CardId(11),
                target: None
            })
        );
    }

    /// The diplomat plays an off-colour pick colorless rather than gift points.
    #[test]
    fn diplomat_goes_colorless_on_off_color() {
        let v = view_with(vec![card(1, Color::Sapphire, 1, 2)], 1);
        let d = Diplomat.decide(&v, &mut rng());
        assert_eq!(
            d.action,
            WaveAction::Play {
                card: CardId(1),
                colorless: true
            }
        );
    }

    /// Random only ever returns options that exist (a held card or a pass), and
    /// only legal spell targets.
    #[test]
    fn random_stays_in_bounds() {
        let mut v = view_with(vec![card(5, Color::Ruby, 1, 1)], 0);
        v.spells = vec![spell(20, SpellKind::Hex), spell(21, SpellKind::Sour)];
        for seed in 0..50u64 {
            use rand::SeedableRng;
            let mut r = StdRng::seed_from_u64(seed);
            let d = Random.decide(&v, &mut r);
            match d.action {
                WaveAction::Pass => {}
                WaveAction::Play { card, .. } => assert_eq!(card, CardId(5)),
            }
            if let Some(sp) = d.spell {
                match sp.target {
                    Some(SpellTarget::Player { player }) => assert_ne!(player, v.me),
                    Some(SpellTarget::Color { color }) => assert_ne!(color, Color::Wild),
                    None => {}
                }
            }
        }
    }

    /// Names resolve to strategies; unknown names are rejected.
    #[test]
    fn assignment_resolves_known_names() {
        let names: Vec<String> = BASELINE_NAMES.iter().map(|s| s.to_string()).collect();
        let assigned = assignment_from_names(&names).expect("all known");
        assert_eq!(assigned.len(), 4);
        assert!(assignment_from_names(&["nope".to_string()]).is_err());
    }
}
