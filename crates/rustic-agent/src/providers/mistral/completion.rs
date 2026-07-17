use crate::{
    client::{
        llm::{CompletionStreamResponse, LlmClient},
        request::CompletionRequest,
        response::CompletionResponse,
    }, providers::{mistral::MISTRAL_BASE_URL, openai::completion::OpenAIClient, }
};
use anyhow::Result;
use async_trait::async_trait;
use rustic_core::HttpResult;
use tracing::info;

/// [`LlmClient`] implementation that proxies to a locally-hosted model server.
///
/// Currently wraps an [`OpenAIClient`] configured with a custom base URL,
/// which is compatible with Ollama's OpenAI/Anthropic-style HTTP endpoint.
#[derive(Debug)]
pub struct MistralClient {
    inner: Box<dyn LlmClient>,
}

impl MistralClient {
    /// Create a `TogetherClient` that speaks the OpenAI HTTP API against `base_url`.
    ///
    /// The API key is set to `"ollama"` as Ollama does not require authentication.
    pub fn new(api_key: String) -> Result<MistralClient> {

        info!(
            target: "agent-openai",
            "Mistral request"
        );        
        
        Ok(Self {
            inner: Box::new(OpenAIClient::new_with_chat_completions(
                MISTRAL_BASE_URL.to_string(),
                api_key,
            )?),
        })
    }
}

#[async_trait]
impl LlmClient for MistralClient {
    async fn complete(&self, request: CompletionRequest) -> HttpResult<CompletionResponse> {

        info!(
            target: "agent-openai",
            "Mistral request"
        );        
        self.inner.complete(mistral_request(request)).await
    }

    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {

        self.inner.complete_with_stream(mistral_request(request)).await
    }
}


pub fn mistral_request(request: CompletionRequest) -> CompletionRequest {

    CompletionRequest {
        id: request.id.clone(),
        provider: request.provider.clone(),
        model: request.model.clone(),
        system: request.system.clone(),
        messages: request.messages.clone(),
        iterations: request.iterations.clone(),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        reasoning_effort: crate::ReasoningEffort::None,
        enable_cache: request.enable_cache,
        stream: request.stream,
        store: false,
        definitions: request.definitions.clone(),
        last_response_id: None,
        response_format_schema: request.response_format_schema
    }

}
