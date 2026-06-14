//! Pure utility functions for building and transforming pipeline messages.
//!
//! These helpers sit between the raw [`CompletionResponse`] values returned by individual agents
//! and the message history maintained by [`PipeLineAgent`](super::runner::PipeLineAgent).
//! They handle `Message` construction from turn history, status-string formatting,
//! multi-agent response merging, and JSON fence stripping.

use serde_json::Value;

use crate::{
    CompletionResponse, CompletionResponseTokenUsage, Message,
    agents::domain::{CompletionTurn, ExecutionMode, StageDecision},
};

/// Build a human-readable status string from an orchestrator decision for display in the stream.
///
/// Returns `"🧠 Synthesising..."` when `stop` is `true`, otherwise lists the chosen agent IDs
/// and their execution mode (e.g. `"⚡ Running: a, b (parallel)"`).
pub fn build_decision_status(decision: &StageDecision) -> String {
    // after decision is made
    match decision.stop {
        true => "🧠 Synthesising...".to_string(),
        false => {
            let agents: Vec<String> = decision.agents.iter().map(|a| a.id.clone()).collect();
            format!(
                "⚡ Running: {} ({})",
                agents.join(", "),
                match decision.execution {
                    ExecutionMode::Parallel => "parallel",
                    ExecutionMode::Sequential => "sequential",
                }
            )
        }
    }
}

/// Convert a slice of [`CompletionTurn`]s into a `(messages, last_response_id)` pair.
///
/// Each turn becomes a `User` message followed by an `Assistant` message. The last
/// `response_id` seen is returned for threading into the next request.
pub fn build_messages_from_turns(turns: &[CompletionTurn]) -> (Vec<Message>, Option<String>) {
    let mut messages = Vec::new();
    let mut response_id = None;
    for turn in turns {
        messages.push(Message::user(turn.user_content.clone()));
        messages.push(Message::assistant(turn.response_content.clone()));
        response_id = turn.response_id.clone();
    }
    (messages, response_id)
}

/// Extract just the last message from a history slice as the pipeline's initial input.
pub fn build_pipeline_input(original_messages: &[Message]) -> Vec<Message> {
    let last_content = original_messages.last().unwrap();
    vec![last_content.clone()]
}

/// Merge multiple sub-agent responses into a single JSON object and a summed token usage.
///
/// Each entry is keyed by `agent_id`. The response text is parsed as JSON if possible;
/// otherwise it is stored as a JSON string. The merged object is serialised back to a string
/// for inclusion in the next orchestrator turn.
pub fn merge_responses(
    responses: &[(String, CompletionResponse)],
) -> (String, CompletionResponseTokenUsage) {
    let mut merged = serde_json::Map::new();
    let mut total_usage = CompletionResponseTokenUsage::default();

    for (agent_id, response) in responses {
        let content = response.text().unwrap_or_default();
        let content = build_clean_json(content);
        let value: Value =
            serde_json::from_str(&content).unwrap_or(Value::String(content.to_string()));
        merged.insert(agent_id.clone(), value);
        total_usage += response.usage.clone();
    }

    (Value::Object(merged).to_string(), total_usage)
}

/// Strip markdown code fences (` ```json ` or ` ``` `) from LLM output.
///
/// Many models wrap JSON responses in fences even when instructed not to; this normalises
/// the text before deserialisation.
pub fn build_clean_json(text: &str) -> String {
    text.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string()
}
