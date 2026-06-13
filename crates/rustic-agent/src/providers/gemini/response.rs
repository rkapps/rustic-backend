use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Top-level response from `POST /v1beta/interactions`.
#[derive(Debug, Deserialize)]
pub struct GeminiInteractionsResponse {
    /// Provider-assigned interaction ID (used as `previous_interaction_id` in follow-up turns).
    pub id: Option<String>,
    pub model: String,
    /// steps
    pub steps: Vec<GeminiStepsResponseOutput>,
    pub status: String,
    pub usage: GeminiInteractionsResponseTokenUsage,
}

/// A single output item in a Gemini response, discriminated by `type`.
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum GeminiStepsResponseOutput {
    /// input from the uesr
    #[serde(rename = "user_input")]
    UserInput { content: Vec<GeminiTextContent> },

    /// input from the model
    #[serde(rename = "model_output")]
    ModelOutput { content: Vec<GeminiTextContent> },

    /// Thought
    #[serde(rename = "thought")]
    Thought {
        // r#type: String,
        summary: Option<Vec<GeminiTextContent>>,
        signature: String,
    },

    /// A function/tool call requested by the model.
    #[serde(rename = "function_call")]
    FunctionCall {
        id: String,
        arguments: Value,
        name: String,
    },
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct GeminiTextContent {
    pub r#type: String,
    pub text: String,
}

/// Detailed token accounting returned by the Gemini Interactions API.
#[derive(Deserialize, Debug, Default)]
pub struct GeminiInteractionsResponseTokenUsage {
    pub total_tokens: i32,
    pub total_input_tokens: i32,
    /// Tokens served from the prompt cache.
    pub total_cached_tokens: i32,
    /// Tokens attributed to tool definitions and results.
    pub total_tool_use_tokens: i32,
    /// Tokens consumed by internal chain-of-thought reasoning.
    pub total_thought_tokens: i32,
    /// Visible output tokens (excluding thought tokens).
    pub total_output_tokens: i32,
}

impl std::ops::AddAssign for GeminiInteractionsResponseTokenUsage {
    fn add_assign(&mut self, rhs: Self) {
        self.total_cached_tokens += rhs.total_cached_tokens;
        self.total_input_tokens += rhs.total_input_tokens;
        self.total_output_tokens += rhs.total_output_tokens;
        self.total_thought_tokens += rhs.total_thought_tokens;
        self.total_tokens += rhs.total_tokens;
        self.total_tool_use_tokens += rhs.total_tool_use_tokens
    }
}
