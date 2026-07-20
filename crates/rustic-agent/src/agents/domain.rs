use serde::{Deserialize, Serialize};

use crate::{Preset, services::config::agent::CompletionStrategy};

/// All runtime parameters needed to build a [`Runnable`](super::runner::Runnable) for an agent.
///
/// Passed to [`AgentService::build_runnable`](crate::services::agent::AgentService::build_runnable)
/// and threaded down through recursive pipeline construction. LLM and model are inherited by
/// sub-agents; strategy and system prompt are overridden per-agent from their config.
#[derive(Debug, Clone)]
pub struct AgentInput {
    pub agent_id: String,
    pub llm_config: LlmConfig,
    /// Optional system prompt override; `None` falls back to an empty string.
    pub system_prompt: Option<String>,
    pub strategy: CompletionStrategy,
    /// Nested sub-agent inputs; empty for leaf agents, unused by `new()`.
    pub subs: Vec<AgentInput>,
}

impl AgentInput {
    pub fn new(
        agent_id: String,
        llm_config: LlmConfig,
        system_prompt: Option<String>,
        strategy: CompletionStrategy,
    ) -> Self {
        Self {
            agent_id,
            llm_config,
            system_prompt,
            strategy,
            subs: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Deserialize, Clone, Serialize)]
pub struct LlmConfig {
    /// Provider ID (e.g. `"anthropic"`, `"openai"`).
    pub llm: Option<String>,
    /// Model identifier forwarded to the provider (e.g. `"claude-sonnet-4-6"`).
    pub model: Option<String>,
    pub preset: Option<Preset>,
}

impl LlmConfig {
    /// Merge two configs — self takes priority, other fills in missing fields
    pub fn merge(self, other: LlmConfig) -> LlmConfig {
        LlmConfig {
            llm: self.llm.or(other.llm),
            model: self.model.or(other.model),
            preset: self.preset.or(other.preset),
        }
    }
}

/// A single completed exchange: the user prompt sent to an agent and the assistant reply received.
///
/// [`PipeLineAgent`](super::runner::PipeLineAgent) accumulates these across stages to build the
/// growing conversation history that is replayed to the orchestrator on each decision turn.
#[derive(Debug, Clone)]
pub struct CompletionTurn {
    /// Position of this turn in the pipeline (1-based).
    pub sequence: u32,
    pub user_content: String,
    pub response_content: String,
    /// Provider-assigned ID for the assistant response; used for multi-turn context continuations.
    pub response_id: Option<String>,
}

impl CompletionTurn {}

/// The orchestrator's parsed decision for a single pipeline stage.
///
/// The orchestrator LLM returns this as JSON. [`PipeLineAgent`](super::runner::PipeLineAgent)
/// deserialises it, runs the chosen sub-agents, and loops back to ask for the next decision —
/// unless `stop` is `true`, in which case the single nominated agent produces the final response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageDecision {
    /// Sub-agents to run in this stage, each paired with a goal string.
    pub agents: Vec<AgentGoal>,
    pub execution: ExecutionMode,
    /// When `true` this is the synthesis stage; exactly one agent must be listed and
    /// execution must be sequential.
    pub stop: bool,
    /// Optional chain-of-thought from the orchestrator (useful for debugging).
    pub reasoning: Option<String>,
}

/// An agent nominated by the orchestrator for a stage, with an optional goal override.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGoal {
    /// Must match the ID of a sub-agent registered in the pipeline config.
    pub id: String,
    /// Goal string forwarded as the prompt to the sub-agent; required when resolving.
    pub goal: Option<String>,
}

/// Controls whether agents in a stage run one-after-another or concurrently.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Agents execute in order; each receives the previous agent's output as context.
    Sequential,
    /// Agents execute concurrently (bounded by a semaphore of 5); results are merged afterwards.
    Parallel,
}

impl<'de> Deserialize<'de> for ExecutionMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "sequential" => Ok(ExecutionMode::Sequential),
            "parallel" => Ok(ExecutionMode::Parallel),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &["sequential", "parallel"],
            )),
        }
    }
}
