use crate::{client::rpc::JsonRpcRequest, tools::response::MCPToolCallResponseContent};
use anyhow::Result;
use reqwest::header::HeaderMap;
use serde_json::Value;
use std::fmt::Debug;

/// Adapts a specific MCP server's wire protocol into a uniform interface.
///
/// Implementors translate between the generic [`JsonRpcRequest`] / raw response
/// text format and the MCP server's actual message shapes. This lets the agent
/// core remain agnostic of individual server quirks.
///
/// All methods are synchronous because they only build or parse data structures —
/// network I/O is handled by the caller.
pub trait MCPServerAdapter: Send + Sync + Debug {
    /// Build the JSON-RPC request that asks the server for its full tool list.
    fn build_tool_list_request(&self) -> JsonRpcRequest;

    /// Parse a raw tool-list response body into a normalised string representation.
    fn parse_tool_list_response(&self, text: String) -> Result<String>;

    /// Build the JSON-RPC request to fetch a single tool's schema by `name`.
    fn build_tool_get_request(&self, name: &str) -> JsonRpcRequest;

    /// Parse a raw tool-get response body into a normalised string representation.
    fn parse_tool_get_response(&self, text: String) -> Result<String>;

    /// Build the JSON-RPC request to invoke `name` with the given `params`.
    fn build_tool_call_request(&self, name: &str, params: Value) -> JsonRpcRequest;

    /// Parse a raw tool-call response body into a [`Value`] result.
    fn parse_tool_call_response(&self, text: Vec<MCPToolCallResponseContent>) -> Value;

    /// Build the `initialize` request sent at the start of an MCP session.
    fn build_initialize_request(&self) -> JsonRpcRequest;

    /// Build the `notifications/initialized` notification sent after the server
    /// acknowledges the `initialize` request.
    fn build_initialized_notification(&self) -> JsonRpcRequest;

    /// Extract the session ID from the HTTP response headers, if present.
    ///
    /// MCP servers that use session-based transport embed the session token in a
    /// response header; this method abstracts over the specific header name.
    fn extract_session_id(&self, headers: &HeaderMap) -> Option<String>;
}
