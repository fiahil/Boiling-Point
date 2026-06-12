//! The deterministic bot brain: instant, zero-cost, reproducible — the balance
//! harness's workhorse and every seat's timeliness floor.
//!
//! Heuristics choose **only among frame-enumerated options** (capability
//! `boom-bot-brain`): the playable list, the pass, and the castable spells with
//! their enumerated targets all come from the server's decision frame, so a bot
//! cannot desync from the rules — a balance patch changes what is enumerated,
//! never what the bot must re-derive. All randomness (tie-breaks, the Random
//! baseline, epsilon blunders) draws from the seat's seeded RNG ([`rng`]).

pub mod rng;

use async_trait::async_trait;
use rand::Rng;
use rand::rngs::StdRng;

use boiling_point_protocol::EmoteId;
use boiling_point_protocol::frame::{
    CastableSpell, PendingDecision, PlayableIngredient, TargetOptions,
};
use boiling_point_protocol::vocab::{Brewer, SpellKind, SpellTarget};

use crate::brain::{Answer, Brain, SpellCast, WaveAction};
use crate::view::{FrameContext, SeatView};

/// The bot's play posture. Each archetype expresses a distinct hypothesis
/// about v2 play (spell usage, fold timing, push thresholds); `Random` is the
/// uniform noise floor every heuristic must beat to justify itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archetype {
    /// Plays low, Peeks for certainty, wards up, bails early.
    Cautious,
    /// Pushes its biggest votes every wave; Doubles Down; almost never folds.
    Aggressive,
    /// Reads the table: information spells, colorless denial, pressure-aware
    /// folds, presses when behind.
    Political,
    /// Uniformly random over the frame's legal actions (the baseline).
    Random,
}

impl Archetype {
    /// Every archetype, in a stable order.
    pub const ALL: [Archetype; 4] = [
        Archetype::Cautious,
        Archetype::Aggressive,
        Archetype::Political,
        Archetype::Random,
    ];

    /// The stable name reports attribute outcomes to.
    pub fn name(self) -> &'static str {
        match self {
            Archetype::Cautious => "cautious",
            Archetype::Aggressive => "aggressive",
            Archetype::Political => "political",
            Archetype::Random => "random",
        }
    }

    /// Parse a name back into an archetype.
    pub fn by_name(name: &str) -> Option<Archetype> {
        Archetype::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// The bot brain's settings (distinct from the agent brain's — spec
/// `boom-bot-brain`): archetype, blunder epsilon, the optional Brewer
/// preference (the harness's persona × Brewer axis), and the seat RNG.
pub struct BotBrain {
    archetype: Archetype,
    /// Per-decision probability of substituting a uniformly random legal
    /// action for the heuristic choice (0 disables blunders).
    epsilon: f64,
    /// Preferred Brewer for the pre-game pick: taken when offered, else the
    /// first option (the deal is random, so a preference is best-effort —
    /// the harness matrix keys on the *actual* pick).
    brewer_preference: Option<Brewer>,
    rng: StdRng,
}

impl BotBrain {
    /// A bot with the given posture and seat RNG, blunder-free.
    pub fn new(archetype: Archetype, rng: StdRng) -> Self {
        BotBrain {
            archetype,
            epsilon: 0.0,
            brewer_preference: None,
            rng,
        }
    }

    /// Set the blunder epsilon (clamped to 0..=1).
    pub fn with_epsilon(mut self, epsilon: f64) -> Self {
        self.epsilon = epsilon.clamp(0.0, 1.0);
        self
    }

    /// Set the pre-game Brewer preference (the harness's brewer axis).
    pub fn with_brewer_preference(mut self, brewer: Option<Brewer>) -> Self {
        self.brewer_preference = brewer;
        self
    }

    /// The bot's archetype.
    pub fn archetype(&self) -> Archetype {
        self.archetype
    }
}

/// The safety margin (in estimated volatility) a careful bot keeps under a
/// *known* boiling point before bailing. `[needs playtesting]`.
const KNOWN_BP_MARGIN: f64 = 5.0;
/// Estimated volatility at or above which careful bots treat a *blind* pot as
/// explosion-risky. `[needs playtesting]`.
const BLIND_RISK_ESTIMATE: f64 = 18.0;

/// The frame's wave-commit parts (empty/false for a non-wave frame, which the
/// heuristics never see — [`BotBrain::decide`] answers Brewer picks first).
fn parts(decision: &PendingDecision) -> (&[PlayableIngredient], bool, &[CastableSpell]) {
    match decision {
        PendingDecision::WaveCommit {
            playable,
            can_pass,
            spells,
            ..
        } => (playable, *can_pass, spells),
        PendingDecision::BrewerPick { .. } => (&[], false, &[]),
    }
}

/// Whether the seat owns (scores for) a playable ingredient's colour.
fn is_mine(view: &SeatView, card: &PlayableIngredient) -> bool {
    card.ingredient.view.color == view.my_color
}

/// The seat's own-colour vote that scores best for the least risk.
fn best_scoring_vote(view: &SeatView, playable: &[PlayableIngredient]) -> Option<WaveAction> {
    playable
        .iter()
        .filter(|c| is_mine(view, c))
        .max_by(|a, b| {
            let (av, bv) = (a.ingredient.view, b.ingredient.view);
            av.points
                .cmp(&bv.points)
                .then(bv.volatility.cmp(&av.volatility)) // prefer lower volatility
                .then(b.ingredient.id.0.cmp(&a.ingredient.id.0)) // prefer lower id
        })
        .map(|c| WaveAction::Play {
            card: c.ingredient.id,
            colorless: false,
        })
}

/// The lowest-volatility playable, preferring own colour then points.
fn safest_card<'a>(
    view: &SeatView,
    playable: &'a [PlayableIngredient],
) -> Option<&'a PlayableIngredient> {
    playable.iter().min_by(|a, b| {
        let (av, bv) = (a.ingredient.view, b.ingredient.view);
        av.volatility
            .cmp(&bv.volatility)
            .then(is_mine(view, b).cmp(&is_mine(view, a))) // prefer own colour
            .then(bv.points.cmp(&av.points)) // prefer more points
            .then(a.ingredient.id.0.cmp(&b.ingredient.id.0))
    })
}

/// The highest-points playable regardless of colour (the aggressive fallback).
fn highest_points_card(playable: &[PlayableIngredient]) -> Option<WaveAction> {
    playable
        .iter()
        .max_by(|a, b| {
            let (av, bv) = (a.ingredient.view, b.ingredient.view);
            av.points
                .cmp(&bv.points)
                .then(av.volatility.cmp(&bv.volatility)) // aggression likes volatility
                .then(b.ingredient.id.0.cmp(&a.ingredient.id.0))
        })
        .map(|c| WaveAction::Play {
            card: c.ingredient.id,
            colorless: false,
        })
}

/// The first castable of a given kind with a deterministic legal target drawn
/// from the frame's enumeration (lowest card id; first enumerated target).
fn cast_of(spells: &[CastableSpell], kind: SpellKind) -> Option<SpellCast> {
    spells
        .iter()
        .filter(|s| s.kind == kind)
        .min_by_key(|s| s.spell.0)
        .and_then(|s| {
            let target = match &s.targets {
                TargetOptions::None => None,
                TargetOptions::Players { players } => Some(SpellTarget::Player {
                    player: *players.first()?,
                }),
                TargetOptions::Colors { colors } => Some(SpellTarget::Color {
                    color: *colors.first()?,
                }),
            };
            Some(SpellCast {
                spell: s.spell,
                target,
            })
        })
}

/// A castable of `kind` aimed at a specific colour, when the frame permits it.
fn cast_at_color(
    spells: &[CastableSpell],
    kind: SpellKind,
    color: boiling_point_protocol::vocab::Color,
) -> Option<SpellCast> {
    spells
        .iter()
        .filter(|s| s.kind == kind)
        .min_by_key(|s| s.spell.0)
        .and_then(|s| match &s.targets {
            TargetOptions::Colors { colors } if colors.contains(&color) => Some(SpellCast {
                spell: s.spell,
                target: Some(SpellTarget::Color { color }),
            }),
            _ => None,
        })
}

/// A held ward, preferring Cap then Halve then Redirect.
fn ward_cast(spells: &[CastableSpell]) -> Option<SpellCast> {
    [SpellKind::Cap, SpellKind::Halve, SpellKind::Redirect]
        .into_iter()
        .find_map(|kind| cast_of(spells, kind))
}

/// Whether the pot currently reads explosion-risky to a careful bot.
fn pot_is_risky(view: &SeatView, next_card_volatility: f64) -> bool {
    if view.quench_shielded_wave == Some(view.wave_number) {
        return false;
    }
    let projected = view.estimated_volatility() + next_card_volatility;
    match view.known_boiling_point() {
        Some(bp) => projected > bp as f64 - KNOWN_BP_MARGIN,
        None => projected > BLIND_RISK_ESTIMATE,
    }
}

/// A uniformly random legal answer drawn from the frame (the Random baseline
/// and the blunder substitute): pass-or-play uniform (colorless on a 25%
/// flip), plus a 30% chance of a uniformly chosen castable at a uniformly
/// chosen enumerated target.
fn uniform_answer(decision: &PendingDecision, rng: &mut StdRng) -> Answer {
    let (playable, can_pass, spells) = parts(decision);
    let options = playable.len() + usize::from(can_pass);
    let action = if options == 0 {
        WaveAction::Pass // unreachable under current rules; stay total
    } else {
        let choice = rng.gen_range(0..options);
        if can_pass && choice == 0 {
            WaveAction::Pass
        } else {
            let card = &playable[choice - usize::from(can_pass)];
            WaveAction::Play {
                card: card.ingredient.id,
                colorless: card.colorless_allowed && rng.gen_bool(0.25),
            }
        }
    };
    let spell = if !spells.is_empty() && rng.gen_bool(0.3) {
        let pick = &spells[rng.gen_range(0..spells.len())];
        let target = match &pick.targets {
            TargetOptions::None => Some(None),
            TargetOptions::Players { players } if !players.is_empty() => {
                Some(Some(SpellTarget::Player {
                    player: players[rng.gen_range(0..players.len())],
                }))
            }
            TargetOptions::Colors { colors } if !colors.is_empty() => {
                Some(Some(SpellTarget::Color {
                    color: colors[rng.gen_range(0..colors.len())],
                }))
            }
            _ => None, // a targeted spell with no legal target is skipped
        };
        target.map(|target| SpellCast {
            spell: pick.spell,
            target,
        })
    } else {
        None
    };
    Answer::WaveCommit { action, spell }
}

/// The cautious posture: Peek while blind, shed the safest card, ward a
/// fattening pot, bail once the pot reads dangerous (or it is ahead).
fn cautious(view: &SeatView, decision: &PendingDecision) -> Answer {
    let (playable, _, spells) = parts(decision);
    let spell = if view.known_boiling_point().is_none() {
        cast_of(spells, SpellKind::Peek)
    } else {
        None
    };
    let safest = safest_card(view, playable);
    let next_vol = safest
        .map(|c| c.ingredient.view.volatility as f64)
        .unwrap_or(0.0);
    let ahead = matches!(
        (view.my_score(), view.best_opponent_score()),
        (Some(me), Some(best)) if me > best
    );
    if pot_is_risky(view, next_vol) || (ahead && view.cauldron_card_count >= 6) {
        return Answer::WaveCommit {
            action: WaveAction::Pass,
            spell,
        };
    }
    let spell = spell.or_else(|| {
        if view.cauldron_card_count >= 4 {
            ward_cast(spells)
        } else {
            None
        }
    });
    Answer::WaveCommit {
        action: safest
            .map(|c| WaveAction::Play {
                card: c.ingredient.id,
                colorless: false,
            })
            .unwrap_or(WaveAction::Pass),
        spell,
    }
}

/// The aggressive posture: biggest vote every wave, Double Down once the pot
/// is worth it, effectively never pass.
fn aggressive(view: &SeatView, decision: &PendingDecision) -> Answer {
    let (playable, _, spells) = parts(decision);
    let spell = if view.cauldron_card_count >= 3 {
        cast_at_color(spells, SpellKind::DoubleDown, view.my_color)
    } else {
        None
    };
    let action = best_scoring_vote(view, playable)
        .or_else(|| highest_points_card(playable))
        .unwrap_or(WaveAction::Pass);
    Answer::WaveCommit { action, spell }
}

/// The political posture: information first, ward or Dampen to stay in a hot
/// pot, press when behind, colorless denial when level, fold when exposed.
fn political(view: &SeatView, decision: &PendingDecision) -> Answer {
    let (playable, _, spells) = parts(decision);
    let info_spell = if view.wave_number <= 1 && view.known_boiling_point().is_none() {
        cast_of(spells, SpellKind::Peek)
    } else if view.wave_number == 2 {
        cast_of(spells, SpellKind::Assay)
    } else {
        None
    };

    let safest = safest_card(view, playable).copied();
    let next_vol = safest
        .map(|c| c.ingredient.view.volatility as f64)
        .unwrap_or(0.0);
    if pot_is_risky(view, next_vol) {
        if let (Some(ward), Some(card)) = (ward_cast(spells), safest) {
            return Answer::WaveCommit {
                action: WaveAction::Play {
                    card: card.ingredient.id,
                    colorless: false,
                },
                spell: Some(ward),
            };
        }
        if let (Some(dampen), Some(card)) = (cast_of(spells, SpellKind::Dampen), safest) {
            return Answer::WaveCommit {
                action: WaveAction::Play {
                    card: card.ingredient.id,
                    colorless: false,
                },
                spell: Some(dampen),
            };
        }
        return Answer::WaveCommit {
            action: WaveAction::Pass,
            spell: info_spell,
        };
    }

    let behind = matches!(
        (view.my_score(), view.best_opponent_score()),
        (Some(me), Some(best)) if me < best
    );
    let action = if behind {
        best_scoring_vote(view, playable)
            .or(safest.map(|c| WaveAction::Play {
                card: c.ingredient.id,
                colorless: false,
            }))
            .unwrap_or(WaveAction::Pass)
    } else {
        match safest {
            Some(card) => WaveAction::Play {
                card: card.ingredient.id,
                colorless: card.colorless_allowed && !is_mine(view, &card),
            },
            None => WaveAction::Pass,
        }
    };
    Answer::WaveCommit {
        action,
        spell: info_spell,
    }
}

#[async_trait]
impl Brain for BotBrain {
    fn name(&self) -> String {
        format!("bot:{}", self.archetype.name())
    }

    async fn decide(&mut self, view: &SeatView, frame: &FrameContext) -> Answer {
        // The pre-game Brewer pick: the preference when offered, else the
        // first option — except Random, which picks uniformly (the baseline
        // covers the pick surface too).
        if let PendingDecision::BrewerPick { options } = &frame.decision {
            let brewer = match self.archetype {
                Archetype::Random if !options.is_empty() => {
                    options[self.rng.gen_range(0..options.len())]
                }
                _ => self
                    .brewer_preference
                    .filter(|b| options.contains(b))
                    .or_else(|| options.first().copied())
                    .unwrap_or(Brewer::ALL[0]),
            };
            return Answer::BrewerPick { brewer };
        }
        // Blunder injection: with probability epsilon, a uniformly random
        // legal action replaces the heuristic choice (Random is exactly the
        // always-uniform case).
        if matches!(self.archetype, Archetype::Random)
            || (self.epsilon > 0.0 && self.rng.gen_bool(self.epsilon))
        {
            return uniform_answer(&frame.decision, &mut self.rng);
        }
        match self.archetype {
            Archetype::Cautious => cautious(view, &frame.decision),
            Archetype::Aggressive => aggressive(view, &frame.decision),
            Archetype::Political => political(view, &frame.decision),
            Archetype::Random => unreachable!("handled above"),
        }
    }

    fn emote(&mut self, _view: &SeatView, palette: &[EmoteId]) -> Option<EmoteId> {
        // Occasional, harmless table-talk — the political bot plays the room.
        if palette.is_empty()
            || !matches!(self.archetype, Archetype::Political | Archetype::Random)
            || !self.rng.gen_bool(0.1)
        {
            return None;
        }
        Some(palette[self.rng.gen_range(0..palette.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::server::PlayerPublic;
    use boiling_point_protocol::vocab::{Color, HandIngredient, IngredientView};
    use boiling_point_protocol::{CardId, PlayerId};
    use rand::SeedableRng;
    use uuid::Uuid;

    fn pid(n: u128) -> PlayerId {
        PlayerId(Uuid::from_u128(n))
    }

    fn playable(id: u32, color: Color, vol: u8, pts: u8) -> PlayableIngredient {
        PlayableIngredient {
            ingredient: HandIngredient {
                id: CardId(id),
                view: IngredientView {
                    color,
                    volatility: vol,
                    points: pts,
                },
            },
            colorless_allowed: true,
        }
    }

    fn castable(id: u32, kind: SpellKind) -> CastableSpell {
        CastableSpell {
            spell: CardId(id),
            kind,
            targets: match kind.target_kind() {
                boiling_point_protocol::vocab::TargetKind::None => TargetOptions::None,
                boiling_point_protocol::vocab::TargetKind::Player => TargetOptions::Players {
                    players: vec![pid(2), pid(3), pid(4)],
                },
                boiling_point_protocol::vocab::TargetKind::Color => TargetOptions::Colors {
                    colors: Color::PLAYER_COLORS.to_vec(),
                },
            },
        }
    }

    fn frame(playable_cards: Vec<PlayableIngredient>, spells: Vec<CastableSpell>) -> FrameContext {
        FrameContext {
            round_number: 1,
            wave_number: 2,
            timer_ms: Some(15_000),
            decision: PendingDecision::WaveCommit {
                playable: playable_cards,
                can_pass: true,
                spells,
                can_defer: false,
            },
        }
    }

    fn view_with(cauldron: u8) -> SeatView {
        let mut v = SeatView::new(pid(1), Color::Ruby);
        v.players = (1..=4u128)
            .map(|n| PlayerPublic {
                id: pid(n),
                display_name: format!("p{n}"),
                color: Color::PLAYER_COLORS[(n - 1) as usize],
                connected: true,
                guest: false,
            })
            .collect();
        v.cauldron_card_count = cauldron;
        v.wave_number = 2;
        v
    }

    fn rng(seed: u64) -> StdRng {
        StdRng::seed_from_u64(seed)
    }

    /// Every archetype, across many seeded frames, answers strictly within the
    /// frame's legal set (the legal-set adherence property).
    #[tokio::test]
    async fn all_archetypes_answer_inside_the_legal_set() {
        for archetype in Archetype::ALL {
            let mut brain = BotBrain::new(archetype, rng(9)).with_epsilon(0.3);
            for i in 0..200u32 {
                let f = frame(
                    vec![
                        playable(1, Color::Ruby, (i % 7) as u8, 2),
                        playable(2, Color::Sapphire, 1, (i % 3) as u8),
                    ],
                    vec![
                        castable(10, SpellKind::Peek),
                        castable(11, SpellKind::Hex),
                        castable(12, SpellKind::Sour),
                        castable(13, SpellKind::Cap),
                    ],
                );
                let v = view_with((i % 9) as u8);
                let answer = brain.decide(&v, &f).await;
                assert!(
                    answer.is_legal(&f.decision),
                    "{archetype:?} produced an illegal answer: {answer:?}"
                );
            }
        }
    }

    /// Same seed, same view/frame sequence ⇒ identical action sequences.
    #[tokio::test]
    async fn same_seed_same_decisions() {
        let run = || async {
            let mut brain = BotBrain::new(Archetype::Random, rng(77)).with_epsilon(0.5);
            let mut answers = Vec::new();
            for i in 0..50u32 {
                let f = frame(
                    vec![playable(1, Color::Ruby, (i % 8) as u8, 1)],
                    vec![castable(10, SpellKind::Peek)],
                );
                let v = view_with((i % 10) as u8);
                answers.push(brain.decide(&v, &f).await);
            }
            answers
        };
        assert_eq!(run().await, run().await);
    }

    /// Epsilon 0 is pure heuristic: across many decisions the cautious bot
    /// never deviates from its deterministic choice (no RNG is consumed on the
    /// heuristic path, so two different seeds agree everywhere).
    #[tokio::test]
    async fn epsilon_zero_is_pure_heuristic() {
        let mut a = BotBrain::new(Archetype::Cautious, rng(1));
        let mut b = BotBrain::new(Archetype::Cautious, rng(2));
        for i in 0..100u32 {
            let f = frame(
                vec![
                    playable(1, Color::Ruby, (i % 7) as u8, 2),
                    playable(2, Color::Emerald, 3, 1),
                ],
                vec![castable(10, SpellKind::Peek), castable(13, SpellKind::Cap)],
            );
            let v = view_with((i % 9) as u8);
            assert_eq!(a.decide(&v, &f).await, b.decide(&v, &f).await);
        }
    }

    /// Archetypes diverge: on the same decisions, the postures pick different
    /// actions (the measurable-difference floor for 4.4; aggregate divergence
    /// is asserted again at batch level in the harness tests).
    #[tokio::test]
    async fn archetypes_diverge_on_the_same_frames() {
        let mut cautious = BotBrain::new(Archetype::Cautious, rng(5));
        let mut aggressive = BotBrain::new(Archetype::Aggressive, rng(5));
        let mut differences = 0u32;
        for i in 0..50u32 {
            // A fattening pot with a spicy high-value card on offer.
            let f = frame(
                vec![
                    playable(1, Color::Ruby, 6, 3),
                    playable(2, Color::Ruby, 1, 0),
                ],
                vec![castable(10, SpellKind::Peek)],
            );
            let v = view_with(4 + (i % 5) as u8);
            if cautious.decide(&v, &f).await != aggressive.decide(&v, &f).await {
                differences += 1;
            }
        }
        assert!(
            differences > 25,
            "cautious and aggressive should disagree on most hot-pot decisions ({differences}/50)"
        );
    }

    /// Every archetype answers a Brewer-pick frame legally: the preference
    /// when offered, the first option otherwise (Random picks within the pair).
    #[tokio::test]
    async fn brewer_picks_are_legal_and_preference_aware() {
        use boiling_point_protocol::vocab::Brewer;
        let pick_frame = FrameContext {
            round_number: 0,
            wave_number: 0,
            timer_ms: Some(20_000),
            decision: PendingDecision::BrewerPick {
                options: vec![Brewer::Featherhand, Brewer::Lurker],
            },
        };
        for archetype in Archetype::ALL {
            let mut plain = BotBrain::new(archetype, rng(8));
            let answer = plain.decide(&view_with(0), &pick_frame).await;
            assert!(answer.is_legal(&pick_frame.decision), "{archetype:?}");
        }
        // The preference is taken when offered…
        let mut wants_lurker =
            BotBrain::new(Archetype::Cautious, rng(8)).with_brewer_preference(Some(Brewer::Lurker));
        assert_eq!(
            wants_lurker.decide(&view_with(0), &pick_frame).await,
            Answer::BrewerPick {
                brewer: Brewer::Lurker
            }
        );
        // …and falls back to the first option when the deal omits it.
        let mut wants_broker =
            BotBrain::new(Archetype::Cautious, rng(8)).with_brewer_preference(Some(Brewer::Broker));
        assert_eq!(
            wants_broker.decide(&view_with(0), &pick_frame).await,
            Answer::BrewerPick {
                brewer: Brewer::Featherhand
            }
        );
    }

    /// The cautious bot bails out of a pot it estimates as risky, and trusts a
    /// Peeked boiling point over the blind estimate.
    #[tokio::test]
    async fn cautious_risk_logic_matches_the_v1_baseline() {
        let mut brain = BotBrain::new(Archetype::Cautious, rng(3));
        // 9 cards ≈ 21.6 estimated volatility > the blind risk line ⇒ pass.
        let f = frame(vec![playable(1, Color::Ruby, 1, 2)], vec![]);
        let v = view_with(9);
        assert!(matches!(
            brain.decide(&v, &f).await,
            Answer::WaveCommit {
                action: WaveAction::Pass,
                ..
            }
        ));
        // A known high boiling point overrides the blind estimate ⇒ play.
        let mut informed = view_with(9);
        informed.disclose_boiling_point(32);
        assert!(matches!(
            brain.decide(&informed, &f).await,
            Answer::WaveCommit {
                action: WaveAction::Play { .. },
                ..
            }
        ));
    }
}
