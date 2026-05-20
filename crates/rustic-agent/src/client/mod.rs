//! Client-side abstractions for interacting with LLM backends and MCP tool servers.
//!
//! Each sub-module has a focused responsibility:
//! - [`llm`] — [`LlmClient`](llm::LlmClient) trait for completion calls (blocking and streaming)
//! - [`mcp`] — [`MCPServerAdapter`](mcp::MCPServerAdapter) trait for MCP tool-server communication
//! - [`message`] — [`Message`](message::Message) enum representing a turn in a conversation
//! - [`request`] — [`CompletionRequest`](request::CompletionRequest) and related input types
//! - [`response`] — response types for both blocking and streaming completions
//! - [`rpc`] — JSON-RPC 2.0 request/response envelope types used by the MCP adapter
//! - [`tools`] — [`Tool`](tools::Tool) trait, [`ToolDefinition`](tools::ToolDefinition), and call/result types

pub mod llm;
pub mod mcp;
pub mod message;
pub mod preset;
pub mod provider;
pub mod request;
pub mod response;
pub mod rpc;
pub mod tools;
