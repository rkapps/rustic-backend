use anyhow::Result;
use anyhow::Error;
use async_trait::async_trait;
use std::fmt::Debug;

/// Async interface for generating text embeddings.
///
/// Implement this trait to add a new embedding backend.  The only required
/// method is [`embed_text`](EmbeddingClient::embed_text); `embed_text_batch`
/// has a default that calls it sequentially and should be overridden when the
/// backend exposes a native batch endpoint (e.g. OpenAI).
#[async_trait]
pub trait EmbeddingClient: Send + Sync + Debug {
    /// Embed a single piece of text, returning a dense float vector.
    async fn embed_text(&self, text: &str) -> Result<Embedding>;

    /// Embed multiple texts.  Partial failures are captured in
    /// [`BatchResult::failed`] rather than propagating as an `Err`, so callers
    /// always receive all successful embeddings even when some inputs fail.
    ///
    /// The default implementation calls [`embed_text`](EmbeddingClient::embed_text)
    /// sequentially.  Override when the backend supports a native batch API.
    async fn embed_text_batch(&self, texts: &[&str]) -> Result<BatchResult> {
        let mut successful = Vec::new();
        let mut failed = Vec::new();
        for (index, text) in texts.iter().enumerate() {
            match self.embed_text(text).await {
                Ok(e) => successful.push((index, e)),
                Err(e) => failed.push((index, e)),
            }
        }
        Ok(BatchResult { successful, failed })
    }
}

/// A dense float vector produced by an embedding model.
#[derive(Debug, Clone)]
pub struct Embedding(Vec<f32>);

impl Embedding {
    /// Wrap a raw vector.
    pub fn new(vector: Vec<f32>) -> Self {
        Self(vector)
    }

    /// Borrow the underlying slice (zero-copy).
    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }

    /// Consume the embedding, returning the inner `Vec<f32>`.
    pub fn into_vec(self) -> Vec<f32> {
        self.0
    }

    /// Number of dimensions in the embedding.
    pub fn dimension(&self) -> usize {
        self.0.len()
    }
}

/// Result of a batch embedding call.
///
/// Both fields use the original input index so callers can correlate results
/// back to their inputs regardless of ordering.
#[derive(Debug)]
pub struct BatchResult {
    /// Embeddings that were generated successfully: `(input_index, embedding)`.
    pub successful: Vec<(usize, Embedding)>,
    /// Inputs that failed: `(input_index, error)`.
    pub failed: Vec<(usize, Error)>,
}

/// Shared test helper — call this from each client's test module.
#[cfg(test)]
pub mod test_utils {
    use super::*;
    use crate::search::similarity::cosine_similarity;

    pub async fn run_embedding_client_test(client: &impl EmbeddingClient) -> Result<()> {
        let start = std::time::Instant::now();
        let query = client.embed_text("find healthcare stocks").await?;
        println!(
            "time to embed query: {:?} vector: {}",
            start.elapsed(),
            query.clone().into_vec().len()
        );

        let short_article = "AbbVie biopharmaceutical HUMIRA autoimmune rheumatology oncology blood cancers HIV hepatitis Parkinson's";
        let start = std::time::Instant::now();
        let article = client.embed_text(short_article).await?;
        println!("time to embed shorter text: {:?}", start.elapsed());
        let start = std::time::Instant::now();
        println!(
            "Article similarity score: {:.4}",
            cosine_similarity(&query.clone().into_vec(), &article.into_vec())
        );
        println!("time for cosine_similarity: {:?}", start.elapsed());

        let long_article = "AbbVie Inc. (AbbVie) is a research-based biopharmaceutical company. The Company is engaged in the discovery development manufacture and sale of a range of pharmaceutical products. Its products are focused on treating conditions such as chronic autoimmune diseases in rheumatology gastroenterology and dermatology oncology including blood cancers virology including hepatitis C virus (HCV) and human immunodeficiency virus (HIV) neurological disorders such as Parkinson's disease and multiple sclerosis metabolic diseases including thyroid disease and complications associated with cystic fibrosis and other serious health conditions.";
        let start = std::time::Instant::now();
        let article = client.embed_text(long_article).await?;
        println!("time to embed longer text: {:?}", start.elapsed());
        let start = std::time::Instant::now();
        println!(
            "Article similarity score: {:.4}",
            cosine_similarity(&query.into_vec(), &article.into_vec())
        );
        println!("time for cosine_similarity: {:?}", start.elapsed());

        Ok(())
    }
}
