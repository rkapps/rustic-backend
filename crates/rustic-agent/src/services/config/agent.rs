use serde::{Deserialize, Serialize};

use crate::Preset;

/// Controls how an agent participates in request handling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionType {
    /// Handles a single request independently.
    SingleAgent,
    /// Orchestrates a sequence of sub-agents.
    Pipeline,
    /// Can act as both a standalone agent and a sub-agent inside a pipeline.
    PipelineAgent,
}

/// Full configuration for a single agent, deserialised from `agents.json`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentConfig {
    /// Unique agent identifier used for lookup and namespacing.
    pub id: String,
    pub name: String,
    pub description: String,
    pub preset: Option<Preset>,
    /// When `true`, the agent appears in the public catalog and can be started directly by users.
    pub standalone: bool,
    pub execution: ExecutionType,
    /// System prompt injected before every conversation.
    pub system_prompt: String,
    /// IDs of tools from the global `ToolRegistry` this agent is permitted to use.
    pub tools: Vec<String>,
    #[serde(default)]
    pub mcp_tools: Vec<String>,
    pub model_assignment: ModelAssignment,
    /// Conversation strategy overrides; `None` uses the server default.
    pub conversation: Option<ConversationConfig>,
    /// Pipeline-specific settings; `None` for `SingleAgent` execution types.
    pub pipeline: Option<PipelineConfig>,
}

/// Default and allowed provider/model combinations for an agent.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelAssignment {
    pub default: ModelProvider,
    pub allowed: Vec<ModelProvider>,
}

/// A provider + model pair used in model assignment config.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelProvider {
    /// Provider identifier (e.g. `"anthropic"`, `"openai"`).
    pub provider: String,
    pub model: String,
}

/// Overrides for conversation history and routing strategy.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConversationConfig {
    pub default_strategy: String,
    pub allowed_strategies: Vec<String>,
    pub default_history_mode: String,
}

/// Pipeline-specific configuration used when `execution` is `Pipeline` or `PipelineAgent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Pipeline variant (e.g. `"sequential"`, `"router"`).
    #[serde(rename = "type")]
    pub pipeline_type: String,
    /// Sub-agent IDs this pipeline can delegate to.
    pub available_agents: Vec<AvailableAgent>,
    /// Ordered stage names for sequential pipelines.
    pub stages: Vec<String>,
    /// Optional agent to invoke for follow-up turns after the pipeline completes.
    pub followup_agent: Option<String>,
    /// Blackboard keys that should survive across pipeline stages.
    pub persisted_blackboard_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableAgent {
    pub id: String,
    pub context: AgentContext,
    #[serde(default)]
    pub preset: Option<Preset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentContext {
    Goal, // original user messages
    Last, // last stage output
    All,  // full accumulated context
}
