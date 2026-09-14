use crate::{
    client::{
        llm::{CompletionStreamResponse, LlmClient},
        request::CompletionRequest,
        response::CompletionResponse,
    },
    providers::anthropic::completion::AnthropicClient,
};
use anyhow::Result;
use async_trait::async_trait;
use rustic_core::HttpResult;

/// [`LlmClient`] implementation that proxies to a locally-hosted model server.
///
/// Currently wraps an [`AnthropicClient`] configured with a custom base URL,
/// which is compatible with Ollama's OpenAI/Anthropic-style HTTP endpoint.
#[derive(Debug)]
pub struct LocalClient {
    inner: Box<dyn LlmClient>,
}

impl LocalClient {
    /// Create a `LocalClient` that speaks the Anthropic HTTP API against `base_url`.
    ///
    /// The API key is set to `"ollama"` as Ollama does not require authentication.
    pub fn anthropic_compat(base_url: String) -> Result<LocalClient> {
        Ok(Self {
            inner: Box::new(AnthropicClient::new_with_base_url(
                "ollama".to_string(),
                "ollama".to_string(),
                base_url,
            )?),
        })
    }
}

#[async_trait]
impl LlmClient for LocalClient {

    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {
        self.inner.complete_with_stream(request).await
    }
}
