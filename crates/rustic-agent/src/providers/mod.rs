//! LLM provider implementations.
//!
//! Each sub-module wraps a specific provider's HTTP API and implements
//! [`LlmClient`](crate::client::llm::LlmClient) so the rest of the crate
//! can remain provider-agnostic.
//!
//! | Module       | Provider           |
//! |--------------|--------------------|
//! | [`anthropic`] | Anthropic (Claude) |
//! | [`openai`]    | OpenAI             |
//! | [`gemini`]    | Google Gemini      |
//! | [`local`]     | Local / Ollama     |
//! | [`groq`]      | Groq               |
//! | [`together`]  | Together           |
//! | [`fireworks`]  | Fireworks          |
//!
pub mod anthropic;
pub mod fireworks;
pub mod gemini;
pub mod groq;
pub mod local;
pub mod openai;
pub mod together;
pub mod mistral;