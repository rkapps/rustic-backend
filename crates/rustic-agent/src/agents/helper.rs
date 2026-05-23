use rustic_core::{HttpError, HttpResult};
use tracing::trace;

use crate::{CompletionResponse, Message, agents::StageDecision};


pub fn build_agent_messages(response: CompletionResponse) -> Vec<Message> {
    let mut messages = Vec::new();
    let clean = build_clean_response_text(response.text());
    if !clean.is_empty() {
        messages.push(Message::Assistant {
            content: clean,
            response_id: Some(response.response_id),
        });

    }
    messages
}

pub fn build_clean_response_text(text: Option<&str>) -> String {
    if let Some(text) = text 
        && !text.trim().is_empty() {

            // guard against empty
            let clean = text
                .trim()
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim()
                .to_string();
        clean            

    } else {
        return String::new()
    }
}

pub fn is_decide_prompt(m: &Message) -> bool {
    matches!(m, Message::User { content, .. } 
        if content.starts_with("Based on the above, decide"))
}

pub fn build_stage_decision(response: CompletionResponse) -> HttpResult<StageDecision> {
    let content = response.text();
    if let Some(val) = content {
        trace!("val: {}", val);
        let clean = val
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        match serde_json::from_str::<StageDecision>(clean) {
            Ok(decision) => return Ok(decision),
            Err(e) => Err(HttpError::Other(format!(
                "Failed to parse StageDecision: {}",
                e
            ))),
        }
    } else {
        return Err(HttpError::Other(
            "Failed to parse completion response".to_string(),
        ));
    }
}


pub fn is_orchestrator_decision(m: &Message) -> bool {
    match m {
        Message::Assistant { content, .. } => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
                // must have agents array and stop field to be a decision
                v.get("agents").is_some() && v.get("stop").is_some()
            } else {
                false
            }
        }
        _ => false
    }
}