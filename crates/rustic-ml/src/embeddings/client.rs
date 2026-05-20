use anyhow::Result;
use async_trait::async_trait;

// Your existing trait
#[async_trait]
pub trait EmbeddingClient: Send + Sync {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>>;
}
