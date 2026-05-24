use rustic_core::{HttpError, HttpResult};
use tracing::{debug, trace};

use crate::{CompletionResponse, Message, agents::{StageDecision, SubAgentResponse}};


pub fn build_merged_sub_agent_message(messages: &mut Vec<Message>) -> String {

      let merged = messages
      .iter()
      .rev()
      .take_while(|m| matches!(m, Message::Assistant { .. }))
      .collect::<Vec<_>>()
      .iter()
      .rev()
      .filter_map(|m| match m {
          Message::Assistant { content, .. } => Some(content.as_str()),
          _ => None,
      })
      .collect::<Vec<_>>()
      .join("\n\n");
    merged
}


pub fn build_sub_agent_messages(messages: &mut Vec<Message>, response: &CompletionResponse ) {
    if let Some(sub_response) = build_sub_agent_response(response) {

        let content = serde_json::json!({
            "agent": sub_response.agent_id,
            "content": sub_response.content
        }).to_string();
        
        debug!(content);

        messages.push(Message::Assistant {
            content: content,
            response_id: None,
        });
    }
}

pub fn build_sub_agent_response(response: &CompletionResponse) -> Option<SubAgentResponse> {
    // if is_orchestrator_decision_response(response) {
    //     return None;
    // }
    response.text().map(|t| SubAgentResponse {
        agent_id: response.id.clone(),
        content: build_clean_json(t),
    })
}


pub fn unwrap_agent_content(content: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(inner) = v.get("content") {
            // handle both string and object
            match inner {
                serde_json::Value::String(s) => s.clone(),
                _ => inner.to_string(),  // serialize object back to string
            }
        } else {
            content.to_string()
        }
    } else {
        content.to_string()
    }
}

// pub fn strip_agent_label(content: &str) -> String {
//     if content.starts_with("## ") {
//         content.lines()
//             .skip(1)
//             .collect::<Vec<_>>()
//             .join("\n")
//             .trim()
//             .to_string()
//     } else {
//         content.to_string()
//     }
// }

// pub fn build_agent_messages(response: CompletionResponse) -> Vec<Message> {
//     let mut messages = Vec::new();
//     let clean = build_clean_response_text(response.text());
//     if !clean.is_empty() {
//         messages.push(Message::Assistant {
//             content: clean,
//             response_id: Some(response.response_id),
//         });

//     }
//     messages
// }

// pub fn build_clean_response_text(text: Option<&str>) -> String {
//     if let Some(text) = text 
//         && !text.trim().is_empty() {
//         build_clean_json(text)
//     } else {
//         return String::new()
//     }
// }

pub fn build_clean_json(text: &str) -> String {
    text
    .trim()
    .trim_start_matches("```json")
    .trim_start_matches("```")
    .trim_end_matches("```")
    .trim()
    .to_string()

}

pub fn is_decide_prompt(m: &Message) -> bool {
    matches!(m, Message::User { content, .. } 
        if content.starts_with("Based on the above, decide"))
}

pub fn build_stage_decision(response: CompletionResponse) -> HttpResult<StageDecision> {
    let content = response.text();
    if let Some(val) = content {
        trace!("val: {}", val);
        let clean = &build_clean_json(val);

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