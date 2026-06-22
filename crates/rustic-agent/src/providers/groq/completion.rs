use crate::{
    ReasoningEffort, client::{
        llm::{CompletionStreamResponse, LlmClient},
        request::CompletionRequest,
        response::CompletionResponse,
    }, providers::{groq::GROQ_BASE_URL, openai::completion::OpenAIClient}
};
use anyhow::Result;
use async_trait::async_trait;
use rustic_core::HttpResult;
use tracing::info;

/// [`LlmClient`] implementation that proxies to a locally-hosted model server.
///
/// Currently wraps an [`AnthropicClient`] configured with a custom base URL,
/// which is compatible with Ollama's OpenAI/Anthropic-style HTTP endpoint.
#[derive(Debug)]
pub struct GroqClient {
    inner: Box<dyn LlmClient>,
}

impl GroqClient {
    /// Create a `GroqClient` that speaks the OpenAI HTTP API against `base_url`.
    ///
    /// The API key is set to `"ollama"` as Ollama does not require authentication.
    pub fn new(api_key: String) -> Result<GroqClient> {
        Ok(Self {
            inner: Box::new(OpenAIClient::new_with_base_url(
                GROQ_BASE_URL.to_string(),
                api_key,
            )?),
        })
    }
}

#[async_trait]
impl LlmClient for GroqClient {
    async fn complete(&self, request: CompletionRequest) -> HttpResult<CompletionResponse> {

        self.inner.complete(groq_request(request)).await
    }

    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {

        self.inner.complete_with_stream(groq_request(request)).await
    }
}


pub fn groq_request(request: CompletionRequest) -> CompletionRequest {

    let max_tokens = get_max_tokens(&request.model, &request.reasoning_effort);
    info!(
        target: "agent-groq",
        model = %request.model,
        request= ?request.reasoning_effort,
        max_tokens= ?max_tokens,
        "Groq request"
    );        


    CompletionRequest {
        id: request.id.clone(),
        model: request.model.clone(),
        system: request.system.clone(),
        messages: request.messages.clone(),
        iterations: request.iterations.clone(),
        temperature: request.temperature,
        max_tokens: max_tokens,
        reasoning_effort: crate::ReasoningEffort::None,
        enable_cache: request.enable_cache,
        stream: request.stream,
        store: false,
        definitions: request.definitions.clone(),
        last_response_id: None,
    }

}


fn get_max_tokens(model: &str, effort: &ReasoningEffort) -> i32 {
    let tpm_budget = match model {
        m if m.contains("qwen3.6") => 8_000,
        m if m.contains("llama-3.1-8b") => 30_000,
        m if m.contains("llama-3.3") => 12_000,
        m if m.contains("llama-4") => 30_000,
        _ => 8_000, // conservative default
    };

    let effort_ratio = match effort {
        ReasoningEffort::None => 0.15,
        ReasoningEffort::Low => 0.25,
        ReasoningEffort::Medium => 0.40,
        ReasoningEffort::High => 0.60,
    };

    // Leave room for the input tokens (system prompt etc.)
    ((tpm_budget as f32 * effort_ratio) as u32).min(8000) as i32
}