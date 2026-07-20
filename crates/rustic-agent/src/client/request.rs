use std::collections::HashMap;

use serde_json::Value;

use crate::client::{message::Message, tools::ToolDefinition};

/// All parameters needed to issue a completion request to an LLM backend.
///
/// Passed to [`LlmClient::complete`](crate::client::llm::LlmClient::complete) or
/// [`LlmClient::complete_with_stream`](crate::client::llm::LlmClient::complete_with_stream).
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub id: String,
    /// provider
    pub provider: String,
    /// Provider-specific model identifier (e.g. `"claude-opus-4-7"`).
    pub model: String,
    /// Optional system prompt prepended before the conversation history.
    pub system: Option<String>,
    /// Ordered conversation history including user turns, assistant replies, and tool exchanges.
    pub messages: Vec<Message>,
    /// order iterations messages
    pub iterations: HashMap<usize, Vec<Message>>,
    /// Sampling temperature; higher values increase output randomness.
    pub temperature: f32,
    /// Hard cap on the number of tokens the model may generate.
    pub max_tokens: i32,
    /// When `true`, the provider is asked to cache the prompt for cheaper repeated calls.
    pub enable_cache: bool,
    /// When `true`, the provider is stateful
    pub store: bool,
    /// Controls how much internal chain-of-thought reasoning the model performs.
    pub reasoning_effort: ReasoningEffort,
    /// When `true`, the response is delivered as a stream of chunks rather than a single payload.
    pub stream: bool,
    /// Tool schemas made available to the model during this request.
    pub definitions: Vec<ToolDefinition>,

    pub last_response_id: Option<String>,
    pub response_format_schema: Option<Value>,
}

/// Controls the depth of chain-of-thought reasoning the model applies before responding.
///
/// Not all providers support every level; unsupported levels are typically rounded
/// down to the nearest supported value.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ReasoningEffort {
    /// No extended reasoning; the model responds directly.
    None,
    /// Minimal reasoning — fast but shallower.
    Low,
    /// Balanced reasoning. This is the default.
    #[default]
    Medium,
    /// Maximum reasoning — slower but more thorough.
    High,
}
