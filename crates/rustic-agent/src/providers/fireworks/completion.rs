use crate::{
    client::{
        llm::{CompletionStreamResponse, LlmClient},
        request::CompletionRequest,
        response::CompletionResponse,
    }, providers::{fireworks::FIREWORKS_BASE_URL, openai::completion::OpenAIClient, }
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
pub struct FireworksClient {
    inner: Box<dyn LlmClient>,
}

impl FireworksClient {
    /// Create a `TogetherClient` that speaks the OpenAI HTTP API against `base_url`.
    ///
    /// The API key is set to `"ollama"` as Ollama does not require authentication.
    pub fn new(api_key: String) -> Result<FireworksClient> {

        info!(
            target: "agent-fireworks",
            "Fireworks request"
        );        
        
        Ok(Self {
            inner: Box::new(OpenAIClient::new_with_chat_completions(
                FIREWORKS_BASE_URL.to_string(),
                api_key,
            )?),
        })
    }
}

#[async_trait]
impl LlmClient for FireworksClient {
    async fn complete(&self, request: CompletionRequest) -> HttpResult<CompletionResponse> {

        info!(
            target: "agent-fireworks",
            "Fireworks request"
        );          
        self.inner.complete(request).await
    }

    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {

        self.inner.complete_with_stream(request).await
    }
}

