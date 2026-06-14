//! The `Brain` trait and the budget-raced decision step.
//!
//! Every brain is a pure chooser over server-enumerated options:
//! `decide(view, frame) → answer`, where the frame carries the complete legal
//! action set (capability `boom-decision-frame`). The decision loop submits
//! only frame-enumerated actions: an answer outside the legal set — or one that
//! misses the latency budget — is replaced by the instant bot-brain fallback,
//! so a seat never stalls a wave (design D6). Brains are interchangeable
//! without changes to the client core; adding a brain means implementing this
//! trait.

use std::time::Duration;

use async_trait::async_trait;

use boiling_point_protocol::frame::PendingDecision;
use boiling_point_protocol::vocab::{Brewer, Recipe, SpellTarget};
use boiling_point_protocol::{CardId, ClientMessage, EmoteId};

use crate::view::{FrameContext, SeatView};

/// The mandatory ingredient-or-pass slot of a wave commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveAction {
    /// Commit a specific hand ingredient into the cauldron.
    Play {
        /// The hand ingredient.
        card: CardId,
        /// Play it colorless (volatility only, zero points).
        colorless: bool,
    },
    /// Pass — a permanent lockout for the rest of the round.
    Pass,
}

/// The optional (≤1 per wave) spell slot of a wave commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellCast {
    /// The grimoire spell to cast.
    pub spell: CardId,
    /// The target, when the spell requires one.
    pub target: Option<SpellTarget>,
}

/// A brain's complete answer to one decision frame. Grows a variant per
/// decision kind as the v2 surface lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The per-wave commit: ingredient-or-pass plus the optional spell.
    WaveCommit {
        /// The mandatory action slot.
        action: WaveAction,
        /// The optional spell slot.
        spell: Option<SpellCast>,
    },
    /// The pre-game Brewer pick (1 of the dealt pair).
    BrewerPick {
        /// The chosen Brewer.
        brewer: Brewer,
    },
    /// The pre-game Apothecary draft: the whole recipe in one submission
    /// (`boom2-apothecary`).
    ApothecaryDraft {
        /// The recipe to submit.
        recipe: Recipe,
    },
}

impl Answer {
    /// A bare pass with no spell — legal on every wave-commit frame, the
    /// fallback of last resort (for a wave commit; see [`Answer::failsafe`]).
    pub fn pass() -> Self {
        Answer::WaveCommit {
            action: WaveAction::Pass,
            spell: None,
        }
    }

    /// The always-legal answer of last resort for a frame: the bare pass for a
    /// wave commit, the first offered option for a Brewer pick, the suggested
    /// quick-pick for the Apothecary draft.
    pub fn failsafe(decision: &PendingDecision) -> Self {
        match decision {
            PendingDecision::WaveCommit { .. } => Answer::pass(),
            PendingDecision::BrewerPick { options } => Answer::BrewerPick {
                brewer: options
                    .first()
                    .copied()
                    .unwrap_or(boiling_point_protocol::Brewer::ALL[0]),
            },
            PendingDecision::ApothecaryDraft { suggested, .. } => Answer::ApothecaryDraft {
                recipe: suggested.clone(),
            },
        }
    }

    /// Whether every part of this answer lies inside the frame's enumerated
    /// legal set (the only gate between a brain and the wire).
    pub fn is_legal(&self, decision: &PendingDecision) -> bool {
        match self {
            Answer::WaveCommit { action, spell } => {
                let action_ok = match action {
                    WaveAction::Play { card, colorless } => {
                        decision.permits_play(*card, *colorless)
                    }
                    WaveAction::Pass => decision.permits_pass(),
                };
                let spell_ok = match spell {
                    Some(cast) => decision.permits_cast(cast.spell, cast.target),
                    None => true,
                };
                action_ok && spell_ok
            }
            Answer::BrewerPick { brewer } => decision.permits_pick(*brewer),
            Answer::ApothecaryDraft { recipe } => decision.permits_recipe(recipe),
        }
    }

    /// The wire messages that submit this answer, in send order (the action,
    /// then the optional cast, then the lock-in that closes the wave early; a
    /// brewer pick or recipe submission is a single message, final on receipt).
    pub fn to_messages(&self) -> Vec<ClientMessage> {
        match self {
            Answer::WaveCommit { action, spell } => {
                let mut msgs = Vec::with_capacity(3);
                msgs.push(match action {
                    WaveAction::Play { card, colorless } => ClientMessage::CommitIngredient {
                        card: *card,
                        colorless: *colorless,
                    },
                    WaveAction::Pass => ClientMessage::CommitPass,
                });
                if let Some(cast) = spell {
                    msgs.push(ClientMessage::CastSpell {
                        spell: cast.spell,
                        target: cast.target,
                    });
                }
                msgs.push(ClientMessage::LockIn);
                msgs
            }
            Answer::BrewerPick { brewer } => {
                vec![ClientMessage::PickBrewer { brewer: *brewer }]
            }
            Answer::ApothecaryDraft { recipe } => {
                vec![ClientMessage::SubmitRecipe {
                    recipe: recipe.clone(),
                }]
            }
        }
    }
}

/// A decision source: pure in `(view, frame)` plus whatever state the brain
/// itself owns (its RNG, its API client). Object-safe so hosts can mix brains
/// per seat at runtime.
#[async_trait]
pub trait Brain: Send {
    /// A stable, human-readable name (reports attribute outcomes to it).
    fn name(&self) -> String;

    /// Choose an answer to `frame` from its enumerated legal action set.
    async fn decide(&mut self, view: &SeatView, frame: &FrameContext) -> Answer;

    /// Optional persona table presence: a preset emote after acting, drawn
    /// from the host-configured palette. Cosmetic only — it never affects
    /// game state — so the default is silence.
    fn emote(&mut self, _view: &SeatView, _palette: &[EmoteId]) -> Option<EmoteId> {
        None
    }
}

/// How a decision was ultimately answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// The configured brain answered legally within budget.
    Brain,
    /// The fallback answered: the brain missed the budget.
    FallbackBudget,
    /// The fallback answered: the brain's answer was outside the legal set.
    FallbackIllegal,
}

/// Race `brain` against `budget`; on a miss — or an illegal answer — commit the
/// instant `fallback` instead (its answer is validated too; a doubly-bad answer
/// degrades to the always-legal bare pass). A late brain answer is dropped with
/// its future. Never blocks past the budget.
pub async fn decide_with_budget(
    brain: &mut dyn Brain,
    fallback: &mut dyn Brain,
    view: &SeatView,
    frame: &FrameContext,
    budget: Duration,
) -> (Answer, Resolution) {
    let raced = tokio::time::timeout(budget, brain.decide(view, frame)).await;
    let (answer, resolution) = match raced {
        Ok(answer) if answer.is_legal(&frame.decision) => (answer, Resolution::Brain),
        Ok(_) => (
            fallback.decide(view, frame).await,
            Resolution::FallbackIllegal,
        ),
        Err(_) => (
            fallback.decide(view, frame).await,
            Resolution::FallbackBudget,
        ),
    };
    if answer.is_legal(&frame.decision) {
        (answer, resolution)
    } else {
        // The fallback itself misbehaved (a brain bug): the frame's failsafe —
        // the bare pass for a wave commit, the first option for a brewer pick.
        (Answer::failsafe(&frame.decision), resolution)
    }
}

/// The per-decision latency budget configuration (design D6): the budget is the
/// frame's timer minus a safety margin, floored, with a default for untimed
/// frames.
#[derive(Debug, Clone, Copy)]
pub struct BudgetConfig {
    /// Subtracted from the frame timer to leave submission + network headroom.
    pub safety_margin_ms: u32,
    /// The budget never drops below this.
    pub min_ms: u32,
    /// Budget for frames that carry no timer.
    pub untimed_ms: u32,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        // Margins sized for the v2 pacing (~25s/15s waves) over a real wire.
        // `[needs playtesting]` like every latency number.
        BudgetConfig {
            safety_margin_ms: 2_500,
            min_ms: 50,
            untimed_ms: 30_000,
        }
    }
}

impl BudgetConfig {
    /// The budget for a frame.
    pub fn budget_for(&self, frame: &FrameContext) -> Duration {
        let ms = match frame.timer_ms {
            Some(t) => t.saturating_sub(self.safety_margin_ms).max(self.min_ms),
            None => self.untimed_ms,
        };
        Duration::from_millis(ms as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::PlayerId;
    use boiling_point_protocol::frame::{CastableSpell, PlayableIngredient, TargetOptions};
    use boiling_point_protocol::vocab::{Color, HandIngredient, IngredientView, SpellKind};
    use uuid::Uuid;

    fn frame() -> FrameContext {
        FrameContext {
            round_number: 1,
            wave_number: 1,
            timer_ms: Some(10_000),
            decision: PendingDecision::WaveCommit {
                playable: vec![PlayableIngredient {
                    ingredient: HandIngredient {
                        id: CardId(1),
                        view: IngredientView {
                            color: Color::Ruby,
                            volatility: 2,
                            points: 1,
                            compounding: None,
                        },
                    },
                    colorless_allowed: true,
                }],
                can_pass: true,
                spells: vec![CastableSpell {
                    spell: CardId(2),
                    kind: SpellKind::Peek,
                    targets: TargetOptions::None,
                }],
                can_defer: false,
            },
        }
    }

    fn view() -> SeatView {
        SeatView::new(PlayerId(Uuid::from_u128(1)), Color::Ruby)
    }

    /// A brain that answers a fixed value instantly.
    struct Fixed(Answer);
    #[async_trait]
    impl Brain for Fixed {
        fn name(&self) -> String {
            "fixed".into()
        }
        async fn decide(&mut self, _v: &SeatView, _f: &FrameContext) -> Answer {
            self.0.clone()
        }
    }

    /// A brain that never answers in time.
    struct Stalled;
    #[async_trait]
    impl Brain for Stalled {
        fn name(&self) -> String {
            "stalled".into()
        }
        async fn decide(&mut self, _v: &SeatView, _f: &FrameContext) -> Answer {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            Answer::pass()
        }
    }

    fn legal_play() -> Answer {
        Answer::WaveCommit {
            action: WaveAction::Play {
                card: CardId(1),
                colorless: false,
            },
            spell: None,
        }
    }

    /// In-budget legal answers go through untouched.
    #[tokio::test(start_paused = true)]
    async fn fast_legal_brain_is_used() {
        let f = frame();
        let v = view();
        let (answer, res) = decide_with_budget(
            &mut Fixed(legal_play()),
            &mut Fixed(Answer::pass()),
            &v,
            &f,
            Duration::from_millis(100),
        )
        .await;
        assert_eq!(answer, legal_play());
        assert_eq!(res, Resolution::Brain);
    }

    /// A budget miss commits the fallback and the late answer is discarded.
    #[tokio::test(start_paused = true)]
    async fn budget_miss_commits_the_fallback() {
        let f = frame();
        let v = view();
        let (answer, res) = decide_with_budget(
            &mut Stalled,
            &mut Fixed(legal_play()),
            &v,
            &f,
            Duration::from_millis(100),
        )
        .await;
        assert_eq!(answer, legal_play());
        assert_eq!(res, Resolution::FallbackBudget);
    }

    /// An out-of-frame answer is never submitted: the fallback replaces it.
    #[tokio::test(start_paused = true)]
    async fn illegal_answer_falls_back() {
        let f = frame();
        let v = view();
        let illegal = Answer::WaveCommit {
            action: WaveAction::Play {
                card: CardId(999),
                colorless: false,
            },
            spell: None,
        };
        let (answer, res) = decide_with_budget(
            &mut Fixed(illegal),
            &mut Fixed(Answer::pass()),
            &v,
            &f,
            Duration::from_millis(100),
        )
        .await;
        assert_eq!(answer, Answer::pass());
        assert_eq!(res, Resolution::FallbackIllegal);
    }

    /// Even a misbehaving fallback degrades to the always-legal pass.
    #[tokio::test(start_paused = true)]
    async fn broken_fallback_degrades_to_pass() {
        let f = frame();
        let v = view();
        let illegal = Answer::WaveCommit {
            action: WaveAction::Play {
                card: CardId(999),
                colorless: false,
            },
            spell: None,
        };
        let (answer, _res) = decide_with_budget(
            &mut Stalled,
            &mut Fixed(illegal),
            &v,
            &f,
            Duration::from_millis(100),
        )
        .await;
        assert_eq!(answer, Answer::pass());
    }

    /// Budgets derive from the frame timer minus the margin, floored.
    #[test]
    fn budget_derivation() {
        let cfg = BudgetConfig {
            safety_margin_ms: 2_000,
            min_ms: 50,
            untimed_ms: 9_000,
        };
        let mut f = frame();
        assert_eq!(cfg.budget_for(&f), Duration::from_millis(8_000));
        f.timer_ms = Some(1_000);
        assert_eq!(cfg.budget_for(&f), Duration::from_millis(50));
        f.timer_ms = None;
        assert_eq!(cfg.budget_for(&f), Duration::from_millis(9_000));
    }
}
