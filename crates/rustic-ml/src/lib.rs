//! Machine-learning utilities for the rustic-ai platform.
//!
//! # Modules
//!
//! - [`embeddings`] — [`EmbeddingClient`] trait and concrete implementations
//!   (Candle/BERT, Gemini, OpenAI).
//! - [`search`] — vector similarity functions used by the storage layer for
//!   semantic search.
//!
//! # Re-exports
//!
//! Commonly used types are available directly at the crate root:
//!
//! ```no_run
//! use rustic_ml::{EmbeddingClient, Embedding, BatchResult};
//! use rustic_ml::{cosine_similarity, search};
//! ```

pub mod embeddings;
pub mod ml;
pub mod search;
pub use embeddings::client::{BatchResult, Embedding, EmbeddingClient};
pub use search::similarity::{cosine_similarity, search};
