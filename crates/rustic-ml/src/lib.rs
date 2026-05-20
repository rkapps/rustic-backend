//! Machine-learning utilities for the rustic-ai platform.
//!
//! # Modules
//!
//! - [`embeddings`] — [`embeddings::client::EmbeddingClient`] trait and concrete
//!   implementations (Candle/BERT, Gemini, OpenAI).
//! - [`search`] — vector similarity functions used by the storage layer for
//!   semantic search.

pub mod embeddings;
pub mod search;