use serde::{Deserialize, Serialize};

use crate::client::tools::ToolDefinition;

/// JSON-RPC result body for `tools/list`.
#[derive(Debug, Deserialize)]
pub(super) struct MCPToolListResponse {
    pub(super) tools: Vec<MCPToolDefinition>,
}

/// Schema for a single tool as returned by the MCP server.
#[derive(Debug, Deserialize, Clone, Serialize)]
pub(super) struct MCPToolDefinition {
    pub(super) name: String,
    pub(super) description: Option<String>,
    /// JSON Schema object describing the tool's accepted parameters.
    #[serde(rename = "inputSchema")]
    pub(super) input_schema: serde_json::Value,
}

impl From<MCPToolDefinition> for ToolDefinition {
    fn from(mcp: MCPToolDefinition) -> Self {
        ToolDefinition {
            r#type: "function".to_string(),
            name: mcp.name,
            description: mcp.description.unwrap_or_default(),
            parameters: mcp.input_schema,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct MCPToolCallResponse {
    pub(super) content: Vec<MCPToolCallResponseContent>,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct MCPToolCallResponseContent {
    #[allow(dead_code)]
    r#type: String,
    pub(super) text: String,
}
