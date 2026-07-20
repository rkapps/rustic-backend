use serde::Deserialize;
use serde_json::Value;

/// Top-level response from `POST /v1/responses`.
#[derive(Deserialize, Debug)]
pub struct OpenAIResponse {
    /// Provider-assigned response ID (used as `previous_response_id` in follow-up turns).
    pub id: String,
    pub model: String,
    /// Ordered output items produced by the model.
    pub output: Vec<OpenAIResponseOutput>,
    pub usage: OpenAITokenUsage,
}

/// A single output item in an OpenAI response, discriminated by `type`.
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum OpenAIResponseOutput {
    /// A text message from the model.
    #[serde(rename = "message")]
    Message {
        id: String,
        status: String,
        content: Vec<OpenAIResponseContent>,
    },

    /// A function/tool call requested by the model.
    #[serde(rename = "function_call")]
    FunctionCall {
        status: String,
        /// JSON-encoded arguments string.
        arguments: String,
        call_id: String,
        name: String,
    },

    /// An internal reasoning item (not surfaced to the caller).
    #[serde(rename = "reasoning")]
    Reasoning { id: String, summary: Vec<String> },
}

/// A content block within a `Message` output item.
#[derive(Deserialize, Debug)]
pub struct OpenAIResponseContent {
    /// Content type; `"output_text"` carries the visible model reply.
    pub r#type: String,
    pub text: String,
}

/// Token accounting returned by OpenAI for both blocking and streaming calls.
#[derive(Deserialize, Debug)]
pub struct OpenAITokenUsage {
    pub total_tokens: i32,
    pub input_tokens: i32,
    pub input_tokens_details: OpenAIInputToken,
    pub output_tokens_details: OpenAIOutputToken,
    pub output_tokens: i32,
}

/// Breakdown of input token costs.
#[derive(Deserialize, Debug)]
pub struct OpenAIInputToken {
    /// Tokens served from the prompt cache.
    pub cached_tokens: i32,
}

/// Breakdown of output token costs.
#[derive(Deserialize, Debug)]
pub struct OpenAIOutputToken {
    /// Tokens consumed by internal chain-of-thought reasoning.
    pub reasoning_tokens: i32,
}

/// Outer SSE envelope (unused in the current streaming path, kept for completeness).
#[derive(Debug, Deserialize)]
pub struct OpentAIChunkResponse {
    pub event: String,
    pub data: Option<OpenAIChunkResponseData>,
}

/// The data payload of a single SSE event in an OpenAI streaming response.
#[derive(Debug, Deserialize)]
pub struct OpenAIChunkResponseData {
    /// SSE event type (e.g. `"response.output_text.delta"`, `"response.completed"`).
    pub r#type: String,
    /// Present on `response.completed` events; carries final usage stats.
    pub response: Option<OpenAIChunkResponseDataResponse>,
    /// Incremental text fragment for `response.output_text.delta`.
    pub delta: Option<String>,
    /// Present on `response.output_item.added` for new function-call items.
    pub item: Option<OpenAIChunkResponseDataItem>,
    /// Item ID used to correlate `delta` events to a specific output item.
    pub item_id: Option<String>,
}

/// A new output item announced during streaming (e.g. a function call being initiated).
#[derive(Debug, Deserialize)]
pub struct OpenAIChunkResponseDataItem {
    pub id: String,
    /// Item type: `"function_call"`, `"message"`, etc.
    pub r#type: String,
    pub status: Option<String>,
    pub arguments: Option<Value>,
    pub call_id: Option<String>,
    pub name: Option<String>,
}

/// Final response metadata delivered on the `response.completed` event.
#[derive(Debug, Deserialize)]
pub struct OpenAIChunkResponseDataResponse {
    pub id: String,
    pub model: String,
    /// Final token usage for the full request.
    pub usage: Option<OpenAITokenUsage>,
}

/// Top-level response from `POST /v1/chat/completions`.
#[derive(Deserialize, Debug)]
pub struct OpenAICompletionsResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<OpenAICompletionsChoice>,
    pub usage: OpenAICompletionsUsage,
}

#[derive(Deserialize, Debug)]
pub struct OpenAICompletionsChoice {
    pub index: i32,
    pub message: OpenAICompletionsMessage,
    pub finish_reason: String,
}

#[derive(Deserialize, Debug)]
pub struct OpenAICompletionsMessage {
    pub role: String,
    pub content: Option<String>,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<OpenAICompletionsTool>>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct OpenAICompletionsUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub prompt_tokens_details: Option<OpenAICompletionsPromptTokensDetails>,
    pub completion_tokens_details: Option<OpenAICompletionsCompletionTokensDetails>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct OpenAICompletionsPromptTokensDetails {
    pub cached_tokens: i32,
}

#[derive(Clone, Deserialize, Debug)]
pub struct OpenAICompletionsCompletionTokensDetails {
    pub reasoning_tokens: i32,
}

#[derive(Clone, Deserialize, Debug)]
pub struct OpenAICompletionsChunkResponse {
    pub id: String,
    pub choices: Vec<OpenAICompletionsChunkChoice>,
    pub usage: Option<OpenAICompletionsUsage>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct OpenAICompletionsChunkChoice {
    pub index: i32,
    pub finish_reason: Option<String>,
    pub delta: OpenAICompletionsChunkChoiceDelta,
}

#[derive(Clone, Deserialize, Debug)]
pub struct OpenAICompletionsChunkChoiceDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<OpenAICompletionsTool>>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct OpenAICompletionsTool {
    pub index: Option<i32>,
    pub id: Option<String>,
    pub r#type: Option<String>,
    pub function: OpenAICompletionsToolFunction,
}

#[derive(Clone, Deserialize, Debug)]
pub struct OpenAICompletionsToolFunction {
    pub name: Option<String>,
    pub arguments: String,
}
