use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{agents::domain::LlmConfig, services::config::agent::HistoryMode::Full};

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
    pub llm_config: Option<LlmConfig>,
    // pub preset: Option<Preset>,
    /// When `true`, the agent appears in the public catalog and can be started directly by users.
    pub standalone: bool,
    pub execution: ExecutionType,
    /// System prompt injected before every conversation.
    pub system_prompt: String,
    /// IDs of tools from the global `ToolRegistry` this agent is permitted to use.
    pub tools: Vec<String>,
    #[serde(default)]
    pub mcp_tools: Vec<String>,
    // pub model_assignment: ModelAssignment,
    /// Conversation strategy overrides; `None` uses the server default.
    pub conversation: ConversationConfig,
    /// Pipeline-specific settings; `None` for `SingleAgent` execution types.
    pub pipeline: Option<PipelineConfig>,
    #[serde(default)]
    pub response_format_schema_path: String,
    #[serde(default)]
    pub response_format_schema: Option<Value>
}

impl AgentConfig {
    pub fn get_strategy(&self) -> CompletionStrategy {
        self.conversation.default_strategy.clone()
    }
    pub fn get_history_mode(&self) -> HistoryMode {
        self.conversation.history_mode.clone().unwrap_or(Full)
    }
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
    pub stages: Vec<PipelineStage>,
    /// Optional agent to invoke for follow-up turns after the pipeline completes.
    pub followup_agent: Option<String>,
    /// Blackboard keys that should survive across pipeline stages.
    pub persisted_blackboard_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStage {
    pub name: String,
    pub parallel: bool,
    pub relay: bool,
    pub sub_agents: Vec<AvailableAgent>
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableAgent {
    pub id: String,
    #[serde(default)]
    pub llm_config: Option<LlmConfig>,
}


#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConversationConfig {
    pub default_strategy: CompletionStrategy,
    pub allowed_strategies: Vec<CompletionStrategy>,
    pub history_mode: Option<HistoryMode>,
    pub max_turns: Option<u32>, // only valid for trimmed
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompletionStrategy {
    Stateless,
    #[default]
    Stateful,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryMode {
    #[default]
    Full,
    Trimmed,
}
