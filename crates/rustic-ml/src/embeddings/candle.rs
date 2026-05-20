use anyhow::Result;
use async_trait::async_trait;
use candle_core::{DType, Device, Tensor};
use candle_transformers::models::{
    bert::{BertModel, Config},
    mimi::candle_nn::VarBuilder,
};
use std::{path::PathBuf, sync::Arc};
use tokenizers::Tokenizer;
use tracing::info;

use crate::embeddings::client::{Embedding, EmbeddingClient};

/// Local BERT embedding client powered by the [Candle](https://github.com/huggingface/candle)
/// inference framework.
///
/// Loads a pre-downloaded model from disk (safetensors + tokenizer JSON) and
/// runs inference on the CPU.  Inference is offloaded to a blocking thread pool
/// via `tokio::task::spawn_blocking` so it does not stall the async executor.
///
/// Embeddings are mean-pooled over token positions and L2-normalised, making
/// them suitable for cosine-similarity comparisons without further scaling.
pub struct CandleEmbeddingClient {
    pub model: Arc<BertModel>,
    pub tokenizer: Arc<Tokenizer>,
    pub device: Device,
}

// BertModel doesn't implement Debug, so we implement it manually.
impl std::fmt::Debug for CandleEmbeddingClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CandleEmbeddingClient")
            .field("device", &self.device)
            .finish_non_exhaustive()
    }
}

impl CandleEmbeddingClient {
    /// Load the BERT model from `cache_path`.
    ///
    /// Expects the directory to contain:
    /// - `model.safetensors` — model weights
    /// - `config.json` — model architecture config
    /// - `tokenizer.json` — HuggingFace tokenizer
    pub async fn new(cache_path: &str) -> Result<Self> {
        let cache = PathBuf::from(cache_path);

        let model_file = cache.join("model.safetensors");
        let config_file = cache.join("config.json");
        let tokenizer_file = cache.join("tokenizer.json");
        info!("model:     {:?}", model_file);
        info!("config:    {:?}", config_file);
        info!("tokenizer: {:?}", tokenizer_file);

        let device = Device::Cpu;
        let tokenizer = Tokenizer::from_file(tokenizer_file).map_err(anyhow::Error::msg)?;

        let config: Config = serde_json::from_str(&std::fs::read_to_string(config_file)?)?;

        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[model_file], DType::F32, &device)? };

        let model = BertModel::load(vb, &config)?;
        Ok(Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            device,
        })
    }
}

#[async_trait]
impl EmbeddingClient for CandleEmbeddingClient {
    async fn embed_text(&self, text: &str) -> Result<Embedding> {
        let model = self.model.clone();
        let tokenizer = self.tokenizer.clone();
        // Make sure you're using the prefix
        let text = format!("passage: {}", text);
        let device = self.device.clone();

        // Run on blocking thread pool — don't block async executor
        tokio::task::spawn_blocking(move || {
            let tokens = tokenizer.encode(text, true).map_err(anyhow::Error::msg)?;

            let token_ids = Tensor::new(tokens.get_ids(), &device)?.unsqueeze(0)?;

            let token_type_ids = token_ids.zeros_like()?;

            let output = model.forward(&token_ids, &token_type_ids, None)?;

            // Mean pooling
            let tensor = (output.sum(1)? / output.dim(1)? as f64)?;
            let tensor = tensor.squeeze(0)?;

            let norm = (tensor.sqr()?.sum(0)? + 1e-12)?.sqrt()?;
            let normalized = tensor.broadcast_div(&norm)?;
            let embedding = Embedding::new(normalized.to_vec1()?);

            Ok(embedding)
        })
        .await?
    }

    // embed_text_batch uses the default sequential implementation from EmbeddingClient.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::client::test_utils::run_embedding_client_test;

    #[tokio::test]
    async fn test_candle_embedding_client() -> Result<()> {
        let path = "/media/raghu/data2/Workspace/AppData/hf-home/minilm";
        let client = CandleEmbeddingClient::new(path).await?;
        run_embedding_client_test(&client).await
    }
}
