# rustic-ml

Local machine learning utilities for the rustic-ai workspace. Provides an `EmbeddingClient` trait and local inference support via [Candle](https://github.com/huggingface/candle) and HuggingFace Hub.

## Contents

### `embeddings::EmbeddingClient`

A minimal async trait for text embedding:

```rust
#[async_trait]
pub trait EmbeddingClient: Send + Sync {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>>;
}
```

Implementations can wrap local Candle models or remote provider clients. This trait is the common interface used across the workspace wherever embeddings are needed.

## Dependencies

| Crate | Purpose |
|---|---|
| `candle-core` | Tensor operations and model execution |
| `candle-transformers` | Pre-built transformer architectures |
| `hf-hub` | Download and cache models from HuggingFace Hub |
| `tokenizers` | Fast tokenisation |

## Status

Early stage — the `EmbeddingClient` trait is stable; local model implementations are in progress.
