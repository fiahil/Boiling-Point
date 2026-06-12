//! The Claude Messages API client (design D4): one decision = one tool-forced
//! `POST /v1/messages` call, made directly from Rust over raw HTTP (no official
//! Rust SDK exists; this is the documented raw-HTTP shape).
//!
//! The wire surface is deliberately tiny — exactly what a tool-forced,
//! non-streaming decision call needs — and sits behind the [`MessagesApi`]
//! trait so tests exercise the brain against a mock without network I/O.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The API version header value the Messages API requires.
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// The production API endpoint base.
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// A user-defined tool definition: name, description, and the JSON Schema of
/// its input (derived from the decision frame — see [`super::schema`]).
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    /// The tool's name (referenced by `tool_choice`).
    pub name: String,
    /// What the tool does and when to use it.
    pub description: String,
    /// JSON Schema for the tool input.
    pub input_schema: serde_json::Value,
}

/// `tool_choice`: force the named tool so the model can only answer by
/// expressing a decision.
#[derive(Debug, Clone, Serialize)]
pub struct ForcedTool {
    /// Always `"tool"`.
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// The tool the model must call.
    pub name: String,
    /// One decision per call — no parallel tool use.
    pub disable_parallel_tool_use: bool,
}

/// One conversation message (the brain sends a single user turn).
#[derive(Debug, Clone, Serialize)]
pub struct ApiMessage {
    /// `"user"` (the brain never continues an assistant turn).
    pub role: &'static str,
    /// The message text.
    pub content: String,
}

/// A `POST /v1/messages` request body, scoped to the decision-call shape.
#[derive(Debug, Clone, Serialize)]
pub struct MessagesRequest {
    /// Model id (e.g. `claude-opus-4-8`).
    pub model: String,
    /// Output ceiling — a tool-forced decision is tiny, so this stays small.
    pub max_tokens: u32,
    /// The persona/difficulty system prompt.
    pub system: String,
    /// The single user turn (game context + decision ask).
    pub messages: Vec<ApiMessage>,
    /// The one decision tool, schema derived from the frame.
    pub tools: Vec<ToolDefinition>,
    /// Forces the decision tool.
    pub tool_choice: ForcedTool,
}

/// Token usage from a response — the spend-cap accounting input.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Usage {
    /// Uncached input tokens billed at the full input price.
    #[serde(default)]
    pub input_tokens: u64,
    /// Output tokens.
    #[serde(default)]
    pub output_tokens: u64,
}

/// One response content block; only `tool_use` matters to the brain.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    /// The model's tool call: the expressed decision.
    #[serde(rename = "tool_use")]
    ToolUse {
        /// The tool input (the decision payload).
        input: serde_json::Value,
    },
    /// Narration (ignored).
    #[serde(rename = "text")]
    Text {
        /// The text content.
        text: String,
    },
    /// Anything else (thinking blocks etc.) — ignored.
    #[serde(other)]
    Other,
}

/// A `POST /v1/messages` response, scoped to what the brain consumes.
#[derive(Debug, Clone, Deserialize)]
pub struct MessagesResponse {
    /// The content blocks (the decision rides in a `tool_use`).
    pub content: Vec<ContentBlock>,
    /// Why generation stopped (`tool_use` expected).
    #[serde(default)]
    pub stop_reason: Option<String>,
    /// Token usage for spend accounting.
    #[serde(default)]
    pub usage: Usage,
}

impl MessagesResponse {
    /// The forced tool call's input, if the model produced one.
    pub fn tool_input(&self) -> Option<&serde_json::Value> {
        self.content.iter().find_map(|b| match b {
            ContentBlock::ToolUse { input } => Some(input),
            _ => None,
        })
    }
}

/// Errors from a Messages API call.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Transport/HTTP failure.
    #[error("messages api transport: {0}")]
    Transport(String),
    /// Non-2xx API response.
    #[error("messages api status {status}: {body}")]
    Status {
        /// HTTP status code.
        status: u16,
        /// Response body (the API's error envelope).
        body: String,
    },
    /// No API key configured.
    #[error("no API key: set ANTHROPIC_API_KEY")]
    NoApiKey,
}

/// The seam between the agent brain and the network: one decision call.
/// Mocked in tests (canned/slow/erroring responses); implemented for real by
/// [`HttpMessagesApi`].
#[async_trait]
pub trait MessagesApi: Send + Sync {
    /// Perform one tool-forced decision call.
    async fn call(&self, request: &MessagesRequest) -> Result<MessagesResponse, ApiError>;
}

/// The production client: reqwest over the first-party API, API-key auth.
pub struct HttpMessagesApi {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl HttpMessagesApi {
    /// A client with an explicit key (auth mode: API key billing — D4).
    pub fn new(api_key: String) -> Self {
        HttpMessagesApi {
            client: reqwest::Client::new(),
            api_key,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// A client reading `ANTHROPIC_API_KEY` from the environment.
    pub fn from_env() -> Result<Self, ApiError> {
        match std::env::var("ANTHROPIC_API_KEY") {
            Ok(key) if !key.is_empty() => Ok(Self::new(key)),
            _ => Err(ApiError::NoApiKey),
        }
    }

    /// Override the endpoint base URL (proxies, test servers).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

#[async_trait]
impl MessagesApi for HttpMessagesApi {
    async fn call(&self, request: &MessagesRequest) -> Result<MessagesResponse, ApiError> {
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| ApiError::Transport(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ApiError::Status {
                status: status.as_u16(),
                body,
            });
        }
        response
            .json::<MessagesResponse>()
            .await
            .map_err(|e| ApiError::Transport(format!("decode: {e}")))
    }
}

/// Per-MTok USD prices for spend-cap accounting (cached from the model
/// catalog; unknown models use the most expensive row so a cap can never be
/// silently exceeded by a price-table gap).
pub fn price_per_mtok(model: &str) -> (f64, f64) {
    match model {
        m if m.starts_with("claude-haiku-4-5") => (1.00, 5.00),
        m if m.starts_with("claude-sonnet-4-6") => (3.00, 15.00),
        m if m.starts_with("claude-opus-4") => (5.00, 25.00),
        // claude-fable-5 and anything newer/unknown: the top-tier price.
        _ => (10.00, 50.00),
    }
}

/// The USD cost of one call's usage under `model`'s pricing.
pub fn usage_cost_usd(model: &str, usage: Usage) -> f64 {
    let (input, output) = price_per_mtok(model);
    (usage.input_tokens as f64 * input + usage.output_tokens as f64 * output) / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Costs follow the per-model price table, and unknown models are priced
    /// at the top tier (caps can never be silently exceeded).
    #[test]
    fn usage_cost_follows_the_price_table() {
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
        };
        assert_eq!(usage_cost_usd("claude-opus-4-8", usage), 30.0);
        assert_eq!(usage_cost_usd("claude-haiku-4-5", usage), 6.0);
        assert_eq!(usage_cost_usd("claude-sonnet-4-6", usage), 18.0);
        assert_eq!(usage_cost_usd("some-future-model", usage), 60.0);
    }
}
