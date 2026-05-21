//! LLM agent orchestration for the rustic-ai platform.
//!
//! # Re-exports
//!
//! Commonly used types are available directly at the crate root:
//!
//! ```no_run
//! // Agent and conversation
//! use rustic_agent::{Agent, Message};
//!
//! // Building agents
//! use rustic_agent::{AgentService, AgentBuilder};
//!
//! // Completion types
//! use rustic_agent::{CompletionRequest, CompletionResponse, CompletionChunkResponse};
//!
//! // Tools
//! use rustic_agent::{Tool, ToolRegistry, MCPRegistry};
//! ```

pub mod agents;
pub mod client;
pub mod providers;
pub mod services;
pub mod tools;

// Agent and conversation
pub use agents::Agent;
pub use client::message::Message;

// Building agents
pub use services::agent::AgentService;
pub use services::builder::AgentBuilder;

// LLM client trait and provider
pub use client::llm::{CompletionStreamResponse, LlmClient, LlmProvider};
pub use client::preset::Preset;
pub use client::provider::Provider;

// Completion request / response types
pub use client::request::{CompletionRequest, ReasoningEffort};
pub use client::response::{
    CompletionChunkResponse, CompletionResponse, CompletionResponseContent,
    CompletionResponseTokenUsage,
};

// Tool types
pub use client::tools::{Tool, ToolCallRequest, ToolDefinition};
pub use tools::mcp::MCPRegistry;
pub use tools::tool::ToolRegistry;
