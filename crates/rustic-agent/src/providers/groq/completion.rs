use crate::{
    client::{
        llm::{CompletionStreamResponse, LlmClient},
        request::CompletionRequest,
        response::CompletionResponse,
    },
    providers::{groq::GROQ_BASE_URL, openai::completion::OpenAIClient},
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

        info!(
            target: "agent-groq",
            model = %request.model,
            request= ?request.reasoning_effort,
            "Groq request"
        );             

        self.inner.complete(groq_request(request)).await
    }

    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {

        info!(
            target: "agent-groq",
            model = %request.model,
            request= ?request.reasoning_effort,
            "Groq request"
        );        

        self.inner.complete_with_stream(groq_request(request)).await
    }
}


pub fn groq_request(request: CompletionRequest) -> CompletionRequest {

    let max_tokens = if request.max_tokens > 32768 { 32768 } else {request.max_tokens};
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
        stream: true,
        store: false,
        definitions: request.definitions.clone(),
        last_response_id: None,
    }

}