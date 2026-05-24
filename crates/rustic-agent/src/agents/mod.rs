//! High-level agent orchestration: drives the LLM completion loop and dispatches tool calls.

pub mod agent;
pub mod helper;
pub mod pipeline_runner;

pub use agent::Agent;
pub use pipeline_runner::PipeLineRunner;
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageDecision {
    pub agents: Vec<String>, // agent_ids from available_agents pool
    pub execution: ExecutionMode,
    pub stop: bool,
    pub reasoning: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    Sequential,
    Parallel,
}

#[derive(Debug, Clone)]
pub struct SubAgentResponse {
    pub agent_id: String,
    pub content: String,  // text only, no JSON decisions, no fences
}