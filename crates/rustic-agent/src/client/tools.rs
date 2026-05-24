use rustic_core::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;

/// A tool invocation requested by the model.
///
/// Produced by deserializing the model's response and matched against registered
/// [`Tool`] implementations by `name` before `execute` is called.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    /// Name of the tool to invoke; must match a registered [`Tool::name`].
    pub name: String,
    /// Opaque call identifier assigned by the model; echoed back in the tool result.
    pub id: String,
    /// Arguments to pass to [`Tool::execute`], matching the tool's parameter schema.
    pub arguments: Value,
}

/// The schema for a single tool sent to the LLM in a completion request.
///
/// Built from a [`Tool`] implementation via [`ToolDefinition::new`] or
/// [`ToolDefinition::from_tool`], or constructed manually for MCP-sourced tools
/// via [`ToolDefinition::default_for_mcp`].
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolDefinition {
    /// Tool type; always `"function"` for current providers.
    pub r#type: String,
    pub name: String,
    pub description: String,
    /// JSON Schema object describing accepted parameters.
    pub parameters: serde_json::Value,
}

impl ToolDefinition {
    /// Build a [`ToolDefinition`] from a statically-typed [`Tool`] reference.
    pub fn new<T: Tool + 'static>(tool: &T) -> Self {
        Self {
            r#type: "function".to_string(),
            name: tool.name(),
            description: tool.description(),
            parameters: tool.parameters(),
        }
    }

    /// Build a [`ToolDefinition`] from a trait object, useful when the concrete
    /// type is not known at the call site.
    pub fn from_tool(tool: &dyn Tool) -> Self {
        Self {
            r#type: "function".to_string(),
            name: tool.name(),
            description: tool.description(),
            parameters: tool.parameters(),
        }
    }

    /// Build a [`ToolDefinition`] from raw parts, used when importing tools from
    /// an MCP server where the schema is already in serialized form.
    pub fn default_for_mcp(r#type: &str, name: &str, description: &str, parameters: Value) -> Self {
        Self {
            r#type: r#type.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        }
    }
}
