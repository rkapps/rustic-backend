use crate::{
    client::{
        llm::{CompletionStreamResponse, LlmClient},
        request::CompletionRequest,
    }, providers::{asione::ASIONE_BASE_URL, openai::completion::OpenAIClient},
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
pub struct AsiOneClient {
    inner: Box<dyn LlmClient>,
}

impl AsiOneClient {
    /// Create a `AsiOneClient` that speaks the OpenAI HTTP API against `base_url`.
    pub fn new(api_key: String) -> Result<AsiOneClient> {
        info!(
            target: "agent-asione",
            "AsiOne request"
        );

        Ok(Self {
            inner: Box::new(OpenAIClient::new_with_base_url(
                ASIONE_BASE_URL.to_string(),
                api_key,
            )?),
        })
    }
}

#[async_trait]
impl LlmClient for AsiOneClient {
    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {
        self.inner
            .complete_with_stream(asione_request(request))
            .await
    }
}

pub fn asione_request(request: CompletionRequest) -> CompletionRequest {
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
        store: request.store,
        definitions: request.definitions.clone(),
        last_response_id: request.last_response_id.clone(),  // was: None — needed for Responses continuity
        response_format_schema: request.response_format_schema,
    }
}
