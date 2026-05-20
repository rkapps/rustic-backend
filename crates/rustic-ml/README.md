# rustic-ml

Machine-learning utilities for the rustic-ai platform. Provides a backend-agnostic `EmbeddingClient` trait with concrete implementations for local inference (Candle/BERT) and hosted APIs (Gemini, OpenAI), plus vector similarity search used by `rustic-storage`.

## Modules

### `embeddings`

#### `EmbeddingClient` trait

```rust
#[async_trait]
pub trait EmbeddingClient: Send + Sync + Debug {
    async fn embed_text(&self, text: &str) -> Result<Embedding>;

    // Default: sequential loop over embed_text.
    // Override when the backend has a native batch endpoint (e.g. OpenAI).
    async fn embed_text_batch(&self, texts: &[&str]) -> Result<BatchResult>;
}
```

`BatchResult` separates partial failures from successes so callers always receive all successful embeddings even when some inputs fail:

```rust
pub struct BatchResult {
    pub successful: Vec<(usize, Embedding)>,  // (input_index, embedding)
    pub failed:     Vec<(usize, Error)>,       // (input_index, error)
}
```

#### Implementations

| Client | Backend | Batch support |
|---|---|---|
| `CandleEmbeddingClient` | Local BERT via Candle (CPU) | Sequential default |
| `GeminiEmbeddingClient` | Google Gemini API (`gemini-embedding-001`) | Sequential default |
| `OpenAIEmbeddingClient` | OpenAI API (`text-embedding-3-small`) | Native batch API |

**Candle** — loads model files from a local directory (no network required at inference time):

```text
<cache_path>/
  model.safetensors
  config.json
  tokenizer.json
```

Embeddings are mean-pooled over token positions and L2-normalised.

**Gemini** — reads the API key from the `GEMINI_API_KEY` environment variable (or pass directly to `new`).

**OpenAI** — reads the API key from the `OPENAI_API_KEY` environment variable (or pass directly to `new`). Sends all texts in a single API call for `embed_text_batch`.

---

### `search`

Vector similarity utilities used by `rustic-storage` for semantic search.

```rust
// Score all candidates against a query vector; return top-k sorted by score.
pub fn search<K: Clone>(
    query: &[f32],
    candidates: &[(K, Vec<f32>)],
    top_k: usize,
) -> Vec<(K, f32)>;

// Cosine similarity in [-1.0, 1.0]. Returns 0.0 for zero vectors.
pub fn cosine_similarity(vec_a: &[f32], vec_b: &[f32]) -> f32;
```

---

## Dependencies

| Crate | Purpose |
|---|---|
| `candle-core` | Tensor operations and CPU inference |
| `candle-transformers` | BERT model architecture |
| `tokenizers` | HuggingFace fast tokenisation |
| `reqwest` | HTTP client for Gemini and OpenAI APIs |
| `async-trait` | Async methods in traits |
| `anyhow` | Error handling |

## Running tests

Candle test uses a locally cached model (path is hardcoded in the test).  
API tests skip automatically when the key env var is not set:

```sh
# Local inference (requires model files)
cargo test -p rustic-ml test_candle_embedding_client -- --nocapture

# Gemini
GEMINI_API_KEY=... cargo test -p rustic-ml test_gemini_embedding_client -- --nocapture

# OpenAI
OPENAI_API_KEY=... cargo test -p rustic-ml test_openai_embedding_client -- --nocapture
```
