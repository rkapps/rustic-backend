use anyhow::Result;
use async_trait::async_trait;
use rustic_core::http::HttpClient;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::embeddings::client::{BatchResult, Embedding, EmbeddingClient};

/// Default model used when creating an [`OpenAIEmbeddingClient`].
pub const MODEL_TEXT_EMBEDDING_3_SMALL: &str = "text-embedding-3-small";
const OPENAI_BASE_URL: &str = "https://api.openai.com";

#[derive(Debug, Deserialize)]
pub(super) struct OpenAIEmbeddingsResponse {
    pub data: Vec<OpenAIEmbeddingsResponseData>,
    // pub model: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct OpenAIEmbeddingsResponseData {
    pub index: usize,
    pub embedding: Vec<f32>,
}

#[async_trait]
impl EmbeddingClient for OpenAIEmbeddingClient {
    async fn embed_text(&self, text: &str) -> Result<Embedding> {
        let url = format!("{}/v1/embeddings", self.base_url,);
        let request = OpenAIEmbeddingsRequest::new(&[text]);

        let mut headers = reqwest::header::HeaderMap::new();
        let bearer = format!("Bearer {}", self.api_key);
        headers.insert("Authorization", bearer.parse()?);

        let body = serde_json::json!(request);
        debug!("Request Body: {:#?}", request);

        let response = self
            .http_client
            .post_request::<OpenAIEmbeddingsResponse>(url, Some(headers), body)
            .await?;
        debug!("Response: {:#?}", response.data.len());
        let embedding = Embedding::new(response.data[0].embedding.clone());

        Ok(embedding)
    }

    async fn embed_text_batch(&self, texts: &[&str]) -> Result<BatchResult> {
        let url = format!("{}/v1/embeddings", self.base_url,);
        let request = OpenAIEmbeddingsRequest::new(texts);
        let mut headers = reqwest::header::HeaderMap::new();
        let bearer = format!("Bearer {}", self.api_key);
        headers.insert("Authorization", bearer.parse()?);

        let body = serde_json::json!(request);
        debug!("Request Body: {:#?}", request);

        let response = self
            .http_client
            .post_request::<OpenAIEmbeddingsResponse>(url, Some(headers), body)
            .await?;

        // consume the response, assuming that OpenAI returns the vectors in the same order.
        // NEED TO VERIFY!!!
        let successful = response
            .data
            .into_iter()
            .map(|data| (data.index, Embedding::new(data.embedding)))
            .collect();

        Ok(BatchResult {
            successful,
            failed: Vec::new(),
        })
    }
}

#[derive(Serialize, Debug)]
pub struct OpenAIEmbeddingsRequest {
    pub model: String,
    pub input: Vec<String>,
}

impl OpenAIEmbeddingsRequest {
    pub fn new(texts: &[&str]) -> Self {
        Self {
            model: MODEL_TEXT_EMBEDDING_3_SMALL.to_string(),
            input: texts.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Embedding client backed by the OpenAI embeddings API
/// (`text-embedding-3-small` by default).
///
/// Requires an OpenAI API key passed to [`OpenAIEmbeddingClient::new`].
/// The key is sent as a `Bearer` token in the `Authorization` header.
///
/// Unlike Gemini and Candle, `embed_text_batch` sends all texts in a single
/// API call and maps the response back to input indices using the `index`
/// field returned by OpenAI — no sequential loop needed.
#[derive(Debug)]
pub struct OpenAIEmbeddingClient {
    pub api_key: String,
    pub base_url: String,
    http_client: HttpClient,
}

impl OpenAIEmbeddingClient {
    /// Create a client using the provided OpenAI API key.
    pub fn new(api_key: &str) -> Result<Self> {
        Ok(OpenAIEmbeddingClient {
            api_key: api_key.to_string(),
            base_url: OPENAI_BASE_URL.to_string(),
            http_client: HttpClient::new()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::client::test_utils::run_embedding_client_test;

    #[tokio::test]
    async fn test_openai_embedding_client() -> Result<()> {
        let Ok(api_key) = std::env::var("OPENAI_API_KEY") else {
            println!("Skipping: OPENAI_API_KEY not set");
            return Ok(());
        };
        let client = OpenAIEmbeddingClient::new(&api_key)?;
        run_embedding_client_test(&client).await
    }
}
