//! The Claude-driven agent brain (capability `boom-agent-brain`): one decision
//! = one tool-forced Messages API call, under a hard timeliness/cost contract.
//!
//! The brain builds its prompt exclusively from the seat's secret-free view
//! and transcript ([`prompt`]), derives the tool schema from the decision
//! frame's legal action set ([`schema`]), and maps the response back to an
//! enumerated action. Anything that goes wrong — caps reached, API errors,
//! malformed or illegal answers — **degrades to an internal bot brain** so the
//! seat always answers (the seat-level budget race in [`crate::brain`] is the
//! liveness guarantee on top). Personas and difficulty shape prompts only;
//! they can never widen the action space.

pub mod api;
pub mod prompt;
pub mod schema;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::bot::{Archetype, BotBrain};
use crate::brain::{Answer, Brain, BudgetConfig};
use crate::view::{FrameContext, SeatView};

use api::{ApiMessage, ForcedTool, MessagesApi, MessagesRequest, usage_cost_usd};
use prompt::Difficulty;

/// Hard spend caps (USD). When a cap is reached the seat degrades to the bot
/// brain for the remainder rather than exceed the cap or abandon the seat.
#[derive(Debug, Clone, Copy)]
pub struct SpendCaps {
    /// Ceiling for one game's API spend.
    pub per_game_usd: f64,
    /// Ceiling for the whole process (shared across every agent seat in it).
    pub per_process_usd: f64,
}

impl Default for SpendCaps {
    fn default() -> Self {
        SpendCaps {
            per_game_usd: 0.50,
            per_process_usd: 10.00,
        }
    }
}

/// The process-wide spend accumulator, shared by every agent brain in the
/// process so the per-process cap is global, not per-seat.
#[derive(Debug, Default)]
pub struct ProcessSpend {
    usd: Mutex<f64>,
}

impl ProcessSpend {
    /// A fresh accumulator (one per process; share the `Arc`).
    pub fn new() -> Arc<Self> {
        Arc::new(ProcessSpend::default())
    }

    /// Record a call's cost and return the new total.
    fn add(&self, cost: f64) -> f64 {
        let mut usd = self.usd.lock().expect("spend lock");
        *usd += cost;
        *usd
    }

    /// The total recorded so far.
    pub fn total_usd(&self) -> f64 {
        *self.usd.lock().expect("spend lock")
    }
}

/// The agent brain's settings — its own block, fully distinct from the bot
/// brain's (spec: "Settings are independent").
#[derive(Debug, Clone)]
pub struct AgentSettings {
    /// Model id (default the current Opus; `claude-haiku-4-5` suits tight
    /// wave budgets).
    pub model: String,
    /// Table persona, in prose (also the seat's display flavor).
    pub persona: String,
    /// Difficulty framing (prompt-only).
    pub difficulty: Difficulty,
    /// The seat-level latency budget derivation this brain is run under
    /// (consumed by the host when configuring the seat).
    pub latency: BudgetConfig,
    /// The bot posture this brain degrades/falls back to.
    pub fallback_archetype: Archetype,
    /// Hard spend caps.
    pub spend: SpendCaps,
    /// Response output ceiling (a tool-forced decision is tiny).
    pub max_tokens: u32,
    /// Transcript lines included per prompt (drop-oldest compaction beyond it).
    pub transcript_limit: usize,
}

impl Default for AgentSettings {
    fn default() -> Self {
        AgentSettings {
            model: "claude-opus-4-8".into(),
            persona: "an enigmatic potion brewer who plays with quiet confidence".into(),
            difficulty: Difficulty::Standard,
            latency: BudgetConfig::default(),
            fallback_archetype: Archetype::Cautious,
            spend: SpendCaps::default(),
            max_tokens: 1024,
            transcript_limit: 120,
        }
    }
}

/// Why an agent decision was answered by the internal degrade bot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DegradeReason {
    /// A spend cap was reached before the call.
    CapReached,
    /// The API call failed.
    ApiError(String),
    /// The response carried no usable tool call, or mapped outside the frame.
    BadAnswer,
}

/// Per-game accounting the host reads after a game.
#[derive(Debug, Clone, Default)]
pub struct AgentStats {
    /// Decisions answered via the API.
    pub api_decisions: u32,
    /// Decisions answered by the internal degrade bot, with reasons.
    pub degraded: Vec<DegradeReason>,
    /// This game's API spend.
    pub game_spend_usd: f64,
    /// Largest assembled user prompt, in characters (growth measurement).
    pub max_prompt_chars: usize,
}

/// The Claude-driven brain. Create one per seat per game (per-game spend
/// accounting resets with the instance; the process accumulator is shared).
pub struct AgentBrain {
    settings: AgentSettings,
    client: Arc<dyn MessagesApi>,
    process_spend: Arc<ProcessSpend>,
    degrade_bot: BotBrain,
    stats: AgentStats,
}

impl AgentBrain {
    /// A brain over `client` with `settings`; `degrade_rng` seeds the internal
    /// bot the brain degrades to.
    pub fn new(
        settings: AgentSettings,
        client: Arc<dyn MessagesApi>,
        process_spend: Arc<ProcessSpend>,
        degrade_rng: rand::rngs::StdRng,
    ) -> Self {
        let degrade_bot = BotBrain::new(settings.fallback_archetype, degrade_rng);
        AgentBrain {
            settings,
            client,
            process_spend,
            degrade_bot,
            stats: AgentStats::default(),
        }
    }

    /// The per-game accounting so far (fallback rate, spend, prompt growth).
    pub fn stats(&self) -> &AgentStats {
        &self.stats
    }

    /// The settings this brain runs under.
    pub fn settings(&self) -> &AgentSettings {
        &self.settings
    }

    /// Whether either spend cap is already exhausted.
    fn cap_reached(&self) -> bool {
        self.stats.game_spend_usd >= self.settings.spend.per_game_usd
            || self.process_spend.total_usd() >= self.settings.spend.per_process_usd
    }

    /// Answer via the degrade bot, recording why.
    async fn degrade(
        &mut self,
        reason: DegradeReason,
        view: &SeatView,
        frame: &FrameContext,
    ) -> Answer {
        tracing::warn!(?reason, "agent brain degraded to bot answer");
        self.stats.degraded.push(reason);
        self.degrade_bot.decide(view, frame).await
    }

    /// Build the one tool-forced request for this decision.
    fn request(&mut self, view: &SeatView, frame: &FrameContext) -> MessagesRequest {
        let tool = schema::tool_from_frame(&frame.decision);
        let (user, prompt_stats) = prompt::user_prompt(view, frame, self.settings.transcript_limit);
        self.stats.max_prompt_chars = self.stats.max_prompt_chars.max(prompt_stats.user_chars);
        MessagesRequest {
            model: self.settings.model.clone(),
            max_tokens: self.settings.max_tokens,
            system: prompt::system_prompt(&self.settings.persona, self.settings.difficulty),
            messages: vec![ApiMessage {
                role: "user",
                content: user,
            }],
            tool_choice: ForcedTool {
                kind: "tool",
                name: tool.name.clone(),
                disable_parallel_tool_use: true,
            },
            tools: vec![tool],
        }
    }
}

#[async_trait]
impl Brain for AgentBrain {
    fn name(&self) -> String {
        format!("agent:{}", self.settings.model)
    }

    async fn decide(&mut self, view: &SeatView, frame: &FrameContext) -> Answer {
        // The pre-game Brewer pick is answered deterministically (first
        // offered option) without an API call: a persona-shaped pick is a
        // possible later refinement, not worth a model round-trip today.
        if let boiling_point_protocol::frame::PendingDecision::BrewerPick { .. } = &frame.decision {
            return Answer::failsafe(&frame.decision);
        }
        if self.cap_reached() {
            return self.degrade(DegradeReason::CapReached, view, frame).await;
        }
        let request = self.request(view, frame);
        let response = match self.client.call(&request).await {
            Ok(response) => response,
            Err(e) => {
                return self
                    .degrade(DegradeReason::ApiError(e.to_string()), view, frame)
                    .await;
            }
        };
        // Spend is recorded for every completed call, even unusable ones.
        let cost = usage_cost_usd(&self.settings.model, response.usage);
        self.stats.game_spend_usd += cost;
        self.process_spend.add(cost);

        match response
            .tool_input()
            .and_then(|input| schema::answer_from_tool_input(&frame.decision, input))
        {
            Some(answer) => {
                self.stats.api_decisions += 1;
                answer
            }
            None => self.degrade(DegradeReason::BadAnswer, view, frame).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api::{ApiError, ContentBlock, MessagesResponse, Usage};
    use boiling_point_protocol::frame::{PendingDecision, PlayableIngredient};
    use boiling_point_protocol::vocab::{Color, HandIngredient, IngredientView};
    use boiling_point_protocol::{CardId, PlayerId};
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use serde_json::json;
    use std::sync::atomic::{AtomicU32, Ordering};
    use uuid::Uuid;

    fn frame() -> FrameContext {
        FrameContext {
            round_number: 1,
            wave_number: 1,
            timer_ms: Some(15_000),
            decision: PendingDecision::WaveCommit {
                playable: vec![PlayableIngredient {
                    ingredient: HandIngredient {
                        id: CardId(7),
                        view: IngredientView {
                            color: Color::Ruby,
                            volatility: 2,
                            points: 1,
                        },
                    },
                    colorless_allowed: true,
                }],
                can_pass: true,
                spells: vec![],
                can_defer: false,
            },
        }
    }

    fn view() -> SeatView {
        SeatView::new(PlayerId(Uuid::from_u128(1)), Color::Ruby).with_transcript()
    }

    /// A mock API: counts calls, returns a canned response or error.
    struct MockApi {
        calls: AtomicU32,
        response: Box<dyn Fn() -> Result<MessagesResponse, ApiError> + Send + Sync>,
    }

    #[async_trait]
    impl MessagesApi for MockApi {
        async fn call(&self, _req: &MessagesRequest) -> Result<MessagesResponse, ApiError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            (self.response)()
        }
    }

    fn tool_response(input: serde_json::Value, output_tokens: u64) -> MessagesResponse {
        MessagesResponse {
            content: vec![ContentBlock::ToolUse { input }],
            stop_reason: Some("tool_use".into()),
            usage: Usage {
                input_tokens: 1_000,
                output_tokens,
            },
        }
    }

    fn brain_with(api: Arc<MockApi>, caps: SpendCaps) -> AgentBrain {
        let settings = AgentSettings {
            spend: caps,
            ..AgentSettings::default()
        };
        AgentBrain::new(settings, api, ProcessSpend::new(), StdRng::seed_from_u64(1))
    }

    /// A good tool answer is used verbatim and the spend is accounted.
    #[tokio::test]
    async fn good_answers_flow_through_and_cost_is_tracked() {
        let api = Arc::new(MockApi {
            calls: AtomicU32::new(0),
            response: Box::new(|| Ok(tool_response(json!({"action": "play", "card": 7}), 50))),
        });
        let mut brain = brain_with(api.clone(), SpendCaps::default());
        let answer = brain.decide(&view(), &frame()).await;
        assert!(answer.is_legal(&frame().decision));
        assert!(matches!(
            answer,
            Answer::WaveCommit {
                action: crate::brain::WaveAction::Play {
                    card: CardId(7),
                    ..
                },
                ..
            }
        ));
        assert_eq!(brain.stats().api_decisions, 1);
        assert!(brain.stats().game_spend_usd > 0.0);
        assert!(brain.stats().max_prompt_chars > 0);
    }

    /// Cap reached ⇒ no further API calls; the seat still answers legally via
    /// the internal degrade bot (cap-degradation path).
    #[tokio::test]
    async fn cap_reached_degrades_without_calling_the_api() {
        let api = Arc::new(MockApi {
            calls: AtomicU32::new(0),
            response: Box::new(|| {
                // One expensive call: 100k output tokens ≈ $2.50 on Opus.
                Ok(tool_response(json!({"action": "pass"}), 100_000))
            }),
        });
        let caps = SpendCaps {
            per_game_usd: 0.01,
            per_process_usd: 10.0,
        };
        let mut brain = brain_with(api.clone(), caps);

        // First decision: under cap, calls the API, busts the per-game cap.
        let first = brain.decide(&view(), &frame()).await;
        assert!(first.is_legal(&frame().decision));
        assert_eq!(api.calls.load(Ordering::SeqCst), 1);
        assert!(brain.stats().game_spend_usd > 0.01);

        // Second decision: cap reached — degraded, NO new API call.
        let second = brain.decide(&view(), &frame()).await;
        assert!(second.is_legal(&frame().decision));
        assert_eq!(api.calls.load(Ordering::SeqCst), 1, "no call past the cap");
        assert_eq!(brain.stats().degraded, vec![DegradeReason::CapReached]);
    }

    /// The per-process cap is shared: another brain's spend exhausts it.
    #[tokio::test]
    async fn process_cap_is_shared_across_brains() {
        let api = Arc::new(MockApi {
            calls: AtomicU32::new(0),
            response: Box::new(|| Ok(tool_response(json!({"action": "pass"}), 10))),
        });
        let process = ProcessSpend::new();
        process.add(99.0); // someone else already spent it all
        let settings = AgentSettings {
            spend: SpendCaps {
                per_game_usd: 1.0,
                per_process_usd: 50.0,
            },
            ..AgentSettings::default()
        };
        let mut brain = AgentBrain::new(settings, api.clone(), process, StdRng::seed_from_u64(2));
        let answer = brain.decide(&view(), &frame()).await;
        assert!(answer.is_legal(&frame().decision));
        assert_eq!(api.calls.load(Ordering::SeqCst), 0);
    }

    /// API errors and unusable answers both degrade to a legal bot answer.
    #[tokio::test]
    async fn errors_and_bad_answers_degrade_legally() {
        let erroring = Arc::new(MockApi {
            calls: AtomicU32::new(0),
            response: Box::new(|| {
                Err(ApiError::Status {
                    status: 529,
                    body: "overloaded".into(),
                })
            }),
        });
        let mut brain = brain_with(erroring, SpendCaps::default());
        let answer = brain.decide(&view(), &frame()).await;
        assert!(answer.is_legal(&frame().decision));
        assert!(matches!(
            brain.stats().degraded.as_slice(),
            [DegradeReason::ApiError(_)]
        ));

        let malformed = Arc::new(MockApi {
            calls: AtomicU32::new(0),
            response: Box::new(|| Ok(tool_response(json!({"action": "play", "card": 999}), 5))),
        });
        let mut brain = brain_with(malformed, SpendCaps::default());
        let answer = brain.decide(&view(), &frame()).await;
        assert!(answer.is_legal(&frame().decision));
        assert!(matches!(
            brain.stats().degraded.as_slice(),
            [DegradeReason::BadAnswer]
        ));
    }

    /// The request the brain sends is tool-forced with the frame-derived schema.
    #[tokio::test]
    async fn requests_are_tool_forced_with_frame_schema() {
        let api = Arc::new(MockApi {
            calls: AtomicU32::new(0),
            response: Box::new(|| Ok(tool_response(json!({"action": "pass"}), 5))),
        });
        let mut brain = brain_with(api, SpendCaps::default());
        let request = brain.request(&view(), &frame());
        assert_eq!(request.tool_choice.kind, "tool");
        assert_eq!(request.tool_choice.name, schema::TOOL_NAME);
        assert_eq!(request.tools.len(), 1);
        assert_eq!(
            request.tools[0].input_schema["properties"]["card"]["enum"],
            json!([7])
        );
        // The no-secrets prompt audit at the request seam: nothing in the
        // request mentions the (undisclosed) boiling point value.
        assert!(
            request.messages[0]
                .content
                .contains("boiling point is unknown")
        );
    }
}
