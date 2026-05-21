use anyhow::Result;
use async_trait::async_trait;
use rustic_core::http::HttpClient;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::embeddings::client::{Embedding, EmbeddingClient};

pub const MODEL_GEMINI_EMBEDDING_001: &str = "gemini-embedding-001";
const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

/// Embedding client backed by the Google Gemini embeddings API
/// (`gemini-embedding-001`).
///
/// Requires a Gemini API key passed to [`GeminiEmbeddingClient::new`].
/// The key is sent via the `x-goog-api-key` request header.
///
/// Gemini does not expose a batch embedding endpoint, so
/// `embed_text_batch` falls back to the sequential default.
#[derive(Debug)]
pub struct GeminiEmbeddingClient {
    pub api_key: String,
    pub base_url: String,
    http_client: HttpClient,
}

impl GeminiEmbeddingClient {
    /// Create a client using the provided Gemini API key.
    pub fn new(api_key: &str) -> Result<Self> {
        Ok(GeminiEmbeddingClient {
            api_key: api_key.to_string(),
            base_url: GEMINI_BASE_URL.to_string(),
            http_client: HttpClient::new()?,
        })
    }
}

#[async_trait]
impl EmbeddingClient for GeminiEmbeddingClient {
    async fn embed_text(&self, text: &str) -> Result<Embedding> {
        let url = format!(
            "{}/v1beta/models/gemini-embedding-001:embedContent",
            self.base_url,
        );
        let request: GeminiEmbeddingsRequest = GeminiEmbeddingsRequest::new(vec![text]);

        let mut headers = reqwest::header::HeaderMap::new();
        // let bearer = format!("Bearer {}", self.api_key);
        // headers.insert("Authorization", bearer.parse()?);
        headers.insert("x-goog-api-key", self.api_key.parse()?);

        debug!("Request: {:#?}", request);

        let body = serde_json::json!(request);
        let response = self
            .http_client
            .post_request::<GeminiEmbeddingsResponse>(url, Some(headers), body)
            .await?;
        debug!("Response: {:#?}", response.embedding.values.len());

        let embedding = Embedding::new(response.embedding.clone().values);

        Ok(embedding)
    }

    // embed_text_batch uses the default sequential implementation from EmbeddingClient.
    // OpenAI overrides this with a real batch API call; Gemini does not have one.
}

#[derive(Serialize, Debug)]
pub(super) struct GeminiEmbeddingsRequest {
    pub model: String,
    content: GeminiEmbeddingsRequestContent,
}

#[derive(Serialize, Debug)]
pub(super) struct GeminiEmbeddingsRequestContent {
    parts: Vec<GeminiEmbeddingsRequestContentPart>,
}

#[derive(Serialize, Debug)]
pub(super) struct GeminiEmbeddingsRequestContentPart {
    text: String,
}

impl GeminiEmbeddingsRequest {
    pub fn new(texts: Vec<&str>) -> Self {
        let mut parts = Vec::new();
        for text in texts {
            let part = GeminiEmbeddingsRequestContentPart {
                text: text.to_string(),
            };
            parts.push(part);
        }

        Self {
            model: MODEL_GEMINI_EMBEDDING_001.to_string(),
            content: GeminiEmbeddingsRequestContent { parts },
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct GeminiEmbeddingsResponse {
    pub(super) embedding: GeminiEmbeddingsResponseEmbedding,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct GeminiEmbeddingsResponseEmbedding {
    pub(super) values: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::client::test_utils::run_embedding_client_test;

    #[tokio::test]
    async fn test_gemini_embedding_client() -> Result<()> {
        let Ok(api_key) = std::env::var("GEMINI_API_KEY") else {
            println!("Skipping: GEMINI_API_KEY not set");
            return Ok(());
        };
        let client = GeminiEmbeddingClient::new(&api_key)?;
        run_embedding_client_test(&client).await
    }
}
