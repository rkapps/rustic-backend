use std::{fmt, pin::Pin};

use futures::Stream;
use rustic_core::HttpResult;
use serde::{Deserialize, Serialize};

use crate::{
    TokenUsage,
    agents::domain::{AgentResponse, truncate},
};

pub type TurnStreamResponse = Pin<Box<dyn Stream<Item = HttpResult<TurnChunkResponse>> + Send>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnChunkResponse {
    /// Incremental visible text chunk — forwarded to UI
    Content { agent_id: String, content: String },
    /// Incremental thought/reasoning — not forwarded to UI
    Thought { agent_id: String, thought: String },
    /// Pipeline status update — UI only
    Status { agent_id: String, status: String },
    /// Agent execution complete — carries full agent response for storage
    Final { response: Box<TurnResponse> },
}

impl TurnChunkResponse {
    pub fn content(agent_id: String, content: String) -> Self {
        Self::Content { agent_id, content }
    }

    pub fn thought(agent_id: String, thought: String) -> Self {
        Self::Thought { agent_id, thought }
    }

    pub fn status(agent_id: String, status: String) -> Self {
        Self::Status { agent_id, status }
    }

    pub fn final_response(response: TurnResponse) -> Self {
        Self::Final { response: Box::new(response) }
    }

    pub fn is_final(&self) -> bool {
        matches!(self, Self::Final { .. })
    }

    pub fn turn_response(&self) -> Option<&TurnResponse> {
        match self {
            Self::Final { response } => Some(response),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnResponse {
    pub agent_id: String,
    pub prompt: String,
    pub content: String,             // final response content
    pub response_id: Option<String>, // LLM response id
    pub usage: TokenUsage,
    pub execution: TurnExecution, // pub stages: Vec<TurnStage>,
}

impl TurnResponse {
    pub fn for_single_agent(prompt: String, agent_response: AgentResponse) -> Self {
        let response = agent_response.clone();
        Self {
            agent_id: agent_response.agent_id,
            prompt,
            content: agent_response.content,
            response_id: Some(agent_response.response_id),
            usage: agent_response.usage,
            execution: TurnExecution::SingleAgent { response },
        }
    }
    pub fn for_deterministic(prompt: &str, agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            prompt: prompt.to_string(),
            content: String::new(),
            response_id: None,
            usage: TokenUsage::default(),
            execution: TurnExecution::Deterministic {
                stages: Vec::new(),
                synthesizer: None, // what actually ran (from config)
            },
        }
    }

    pub fn for_dynamic(agent_id: String, prompt: String) -> Self {
        Self {
            agent_id,
            prompt,
            content: String::new(),
            response_id: None,
            usage: TokenUsage::default(),
            execution: TurnExecution::Dynamic {
                decisions: Vec::new(),
                synthesizer: None,
            },
        }
    }

    pub fn add_stage(&mut self, stage: StageResponse) {
        if let TurnExecution::Deterministic { stages, .. } = &mut self.execution {
            // accumulate usage from all turn responses in the stage
            for turn in &stage.responses {
                self.usage += turn.usage.clone();
            }
            stages.push(stage);
        }
    }

    pub fn add_decision(&mut self, decision: DecisionResponse) {
        if let TurnExecution::Dynamic { decisions, .. } = &mut self.execution {
            // accumulate usage from all turn responses in the stage
            for turn in &decision.responses {
                self.usage += turn.usage.clone();
            }
            decisions.push(decision);
        }
    }

    pub fn set_synthesizer(&mut self, final_response: Box<TurnResponse>) {
        self.content = final_response.content.clone();
        self.usage += final_response.usage.clone();

        match &mut self.execution {
            TurnExecution::Deterministic { synthesizer, .. } => {
                *synthesizer = Some(final_response);
            }
            TurnExecution::Dynamic { synthesizer, .. } => {
                *synthesizer = Some(final_response);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnExecution {
    SingleAgent {
        response: AgentResponse,
    },
    Deterministic {
        stages: Vec<StageResponse>,
        synthesizer: Option<Box<TurnResponse>>, // ← Box for recursion
    },
    Dynamic {
        decisions: Vec<DecisionResponse>,
        synthesizer: Option<Box<TurnResponse>>, // ← Box for recursion
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResponse {
    pub name: String,
    pub responses: Vec<TurnResponse>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResponse {
    pub iteration: u32,
    pub decision: StageDecision,
    pub responses: Vec<TurnResponse>,
    pub duration_ms: u64,
}

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

impl fmt::Display for TurnResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::json!({
            "agent_id": self.agent_id,
            "prompt": self.prompt,
            "content": truncate(&self.content, 300),
            // "duration_ms": self.duration_ms,
            "usage": {
                "input_tokens": self.usage.input_tokens,
                "output_tokens": self.usage.output_tokens,
                "cached_read_tokens": self.usage.cached_read_tokens,
                "reasoning_tokens": self.usage.reasoning_tokens,
                "total_tokens": self.usage.total_tokens,
            },
            "execution": self.execution_json(),
        });
        write!(
            f,
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        )
    }
}

impl TurnResponse {
    fn execution_json(&self) -> serde_json::Value {
        match &self.execution {
            TurnExecution::SingleAgent { response } => serde_json::json!({
                "type": "single_agent",
                "agent": agent_json(response),
            }),
            TurnExecution::Deterministic {
                stages,
                synthesizer,
            } => serde_json::json!({
                "type": "deterministic",
                "stages": stages.iter().map(|s| serde_json::json!({
                    "name": s.name,
                    "duration_ms": s.duration_ms,
                    "responses": s.responses.iter().map(|r| serde_json::json!({
                        "agent_id": r.agent_id,
                        "prompt": truncate(&r.prompt, 100),
                        "content": truncate(&r.content, 300),
                        // "duration_ms": r.duration_ms,
                        "usage": {
                            "input_tokens": r.usage.input_tokens,
                            "output_tokens": r.usage.output_tokens,
                            "cached_read_tokens": self.usage.cached_read_tokens,
                            "reasoning_tokens": r.usage.reasoning_tokens,
                            "total_tokens": r.usage.total_tokens,
                        },
                        "execution": r.execution_json(),
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "synthesizer": synthesizer.as_ref().map(|s| serde_json::json!({
                    "agent_id": s.agent_id,
                    "prompt": truncate(&s.prompt, 100),
                    "content": truncate(&s.content, 300),
                    // "duration_ms": s.duration_ms,
                    "usage": {
                        "input_tokens": s.usage.input_tokens,
                        "output_tokens": s.usage.output_tokens,
                        "cached_read_tokens": self.usage.cached_read_tokens,
                        "reasoning_tokens": s.usage.reasoning_tokens,
                        "total_tokens": s.usage.total_tokens,
                    },
                    "execution": s.execution_json(),
                })),
            }),
            TurnExecution::Dynamic {
                decisions,
                synthesizer,
            } => serde_json::json!({
                "type": "dynamic",
                "decisions": decisions.iter().map(|d| serde_json::json!({
                    "iteration": d.iteration,
                    "duration_ms": d.duration_ms,
                    "responses": d.responses.iter().map(|r| serde_json::json!({
                        "agent_id": r.agent_id,
                        "prompt": truncate(&r.prompt, 100),
                        "content": truncate(&r.content, 300),
                        // "duration_ms": r.duration_ms,
                        "usage": {
                            "input_tokens": r.usage.input_tokens,
                            "output_tokens": r.usage.output_tokens,
                            "cached_read_tokens": self.usage.cached_read_tokens,
                            "reasoning_tokens": r.usage.reasoning_tokens,
                            "total_tokens": r.usage.total_tokens,
                        },
                        "execution": r.execution_json(),
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "synthesizer": synthesizer.as_ref().map(|s| serde_json::json!({
                    "agent_id": s.agent_id,
                    "content": truncate(&s.content, 300),
                    // "duration_ms": s.duration_ms,
                    "usage": {
                        "input_tokens": s.usage.input_tokens,
                        "output_tokens": s.usage.output_tokens,
                        "cached_read_tokens": self.usage.cached_read_tokens,
                        "reasoning_tokens": s.usage.reasoning_tokens,
                        "total_tokens": s.usage.total_tokens,
                    },
                    "execution": s.execution_json(),
                })),
            }),
        }
    }
}

fn agent_json(response: &AgentResponse) -> serde_json::Value {
    serde_json::json!({
        "agent_id": response.agent_id,
        "model": response.model,
        "duration_ms": response.duration_ms,
        "usage": {
            "input_tokens": response.usage.input_tokens,
            "output_tokens": response.usage.output_tokens,
            "cached_read_tokens": response.usage.cached_read_tokens,
            "reasoning_tokens": response.usage.reasoning_tokens,
            "total_tokens": response.usage.total_tokens,
        },
        "iterations": response.iterations.iter().map(|i| serde_json::json!({
            "iteration": i.iteration,
            "duration_ms": i.duration_ms,
            "finish_reason": i.finish_reason,
            "usage": {
                "input_tokens": i.usage.input_tokens,
                "output_tokens": i.usage.output_tokens,
                "cached_read_tokens": i.usage.cached_read_tokens,
                "reasoning_tokens": i.usage.reasoning_tokens,
                "total_tokens": i.usage.total_tokens,
            },
            "tool_calls": i.tool_calls.iter().map(|t| serde_json::json!({
                "name": t.name,
                "input": t.input,
                "output": t.output,
                "duration_ms": t.duration_ms,
                "error": t.error,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}
