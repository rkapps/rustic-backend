use std::pin::Pin;

use async_trait::async_trait;
use futures_util::Stream;
use rustic_core::http::HttpResult;
use serde::Serialize;

use crate::client::{
    request::CompletionRequest,
    response::{CompletionChunkResponse, CompletionResponse},
};

/// A pinned, heap-allocated stream of [`CompletionChunkResponse`] items yielded by a streaming
/// completion call. Each item is wrapped in [`HttpResult`] to propagate transport-level errors
/// inline with the stream.
pub type CompletionStreamResponse =
    Pin<Box<dyn Stream<Item = HttpResult<CompletionChunkResponse>> + Send>>;

/// Abstraction over an LLM backend that supports both blocking and streaming completion calls.
///
/// Implementors must be `Send + Sync` so that client instances can be shared across async tasks.
/// The `Debug` bound allows clients to appear in error messages and logs.
#[async_trait]
pub trait LlmClient: Send + Sync + std::fmt::Debug {
    /// Send a completion request and wait for the full response.
    ///
    /// Returns a [`CompletionResponse`] containing the model's reply, or an [`HttpResult`] error
    /// if the underlying request fails.
    async fn complete(&self, request: CompletionRequest) -> HttpResult<CompletionResponse>;

    /// Send a completion request and receive the response as a stream of chunks.
    ///
    /// Returns a [`CompletionStreamResponse`] that yields [`CompletionChunkResponse`] items as
    /// the model produces them, enabling low-latency token-by-token consumption.
    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse>;
}



#[derive(Debug, Serialize)]
pub struct LlmProvider {
    pub id: String,
    pub llm: String,
    pub models: Vec<String>,
    pub default_model: String,
}
