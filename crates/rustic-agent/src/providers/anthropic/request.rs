use crate::client::{
    message::Message,
    request::{CompletionRequest, ReasoningEffort},
};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};
use tracing::{debug, info, trace};

/// Serialized body sent to `POST /v1/messages`.
#[derive(Debug, Serialize)]
pub struct AnthropicCompletionRequest {
    pub model: String,
    max_tokens: i32,
    temperature: f32,
    messages: Vec<AnthropicCompletionRequestMessage>,
    cache_control: AnthropicCompletionRequestCache,
    thinking: AnthropicThinking,
    system: Option<String>,
    stream: bool,
    pub tools: Vec<AnthropicToolDefinition>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<AnthropicOutputConfig>
}

impl AnthropicCompletionRequest {
    pub fn log_info(&self) {
        info!(
            target: "agent-anthropic",
            model = %self.model,
            messages = self.messages.len(),
            last_message = %format!("{:#?}", self.messages.last()),
            tools = self.tools.len(),
            "Anthropic request"
        );
    }

    pub fn log_debug(&self) {
        debug!(
            target: "agent-anthropic",
            model = %self.model,
            temperature = %format!("{:.1}", self.temperature),
            thinking_level = ?self.thinking,
            max_tokens = self.max_tokens,
            messages = %format!("{:#?}", self.messages),
            tools = self.tools.len(),
            "Anthropic request"
        );
    }

    pub fn log_trace(&self) {
        trace!(
            target: "agent-anthropic",
            request = ?self,
            "Anthropic full request"
        );
    }
}

// /// System-prompt block (currently unused in favour of a plain string field).
// #[derive(Debug, Serialize)]
// pub struct AnthropicCompletionRequestSystem {
//     r#type: String,
//     text: String,
// }

/// Cache-control marker sent alongside the request to enable ephemeral prompt caching.
#[derive(Debug, Serialize)]
pub struct AnthropicCompletionRequestCache {
    r#type: String,
}

/// A single turn in the Anthropic messages array, serialized without an enum tag.
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum AnthropicCompletionRequestMessage {
    /// Plain text user or assistant turn.
    Content { role: String, content: String },
    /// An assistant turn that contains one or more tool-use blocks.
    ToolUse {
        role: String,
        content: Vec<AnthropicCompletionRequestToolUse>,
    },
    /// A user turn that carries tool results back to the model.
    ToolResult {
        role: String,
        content: Vec<AnthropicCompletionRequestToolResult>,
    },
}

/// A single `tool_use` content block inside an assistant turn.
#[derive(Debug, Serialize)]
pub struct AnthropicCompletionRequestToolUse {
    r#type: String,
    input: Value,
    id: String,
    name: String,
}

/// A single `tool_result` content block inside a user turn.
#[derive(Debug, Serialize)]
pub struct AnthropicCompletionRequestToolResult {
    r#type: String,
    tool_use_id: String,
    content: String,
}

/// Tool schema in Anthropic's format (`input_schema` instead of `parameters`).
#[derive(Debug, Serialize)]
pub struct AnthropicToolDefinition {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

/// Extended-thinking configuration sent with every request.
///
/// `budget_tokens` is capped below `max_tokens` to ensure there is always
/// room for visible output.
#[derive(Debug, Serialize)]
pub struct AnthropicThinking {
    r#type: String,
    budget_tokens: i32,
}

impl AnthropicThinking {
    /// Build the thinking config from a [`ReasoningEffort`] level.
    ///
    /// `None` → `disabled`; `Low`/`Medium`/`High` → `enabled` with increasing token budgets.
    pub fn new(reasoning_effort: ReasoningEffort) -> Self {
        match reasoning_effort {
            ReasoningEffort::None => AnthropicThinking {
                r#type: "disabled".to_string(),
                budget_tokens: 0,
            },
            ReasoningEffort::Low => AnthropicThinking {
                r#type: "enabled".to_string(),
                budget_tokens: 2048,
            },
            ReasoningEffort::Medium => AnthropicThinking {
                r#type: "enabled".to_string(),
                budget_tokens: 4096,
            },
            ReasoningEffort::High => AnthropicThinking {
                r#type: "enabled".to_string(),
                budget_tokens: 8000,
            },
        }
    }
}


/// Tool schema in Anthropic's format (`input_schema` instead of `parameters`).
#[derive(Debug, Serialize)]
pub struct AnthropicOutputConfig {
    format: Value,
}

// #[derive(Debug, Serialize)]
// pub struct AnthropicOutputConfigFormat {
//     r#type: String,
//     schema: Value
// }


impl AnthropicCompletionRequest {
    /// Convert a provider-agnostic [`CompletionRequest`] into Anthropic's wire format.
    ///
    /// Tool calls and results are grouped into the correct role/content structure,
    /// and `temperature` is forced to `1.0` when extended thinking is enabled
    /// (Anthropic requires this).
    pub fn new(request: CompletionRequest) -> Result<AnthropicCompletionRequest> {
        let mut messages: Vec<AnthropicCompletionRequestMessage> = Vec::new();
        let mut tool_result_contents = Vec::new();
        let mut tool_use_contents = Vec::new();
        let arequest = request.clone();
        let iterations = request.iterations;

        let mut sorted_keys: Vec<usize> = iterations.keys().cloned().collect();
        sorted_keys.sort();

        let imessages: Vec<Message> = sorted_keys
            .iter()
            .flat_map(|k| iterations.get(k).unwrap().clone())
            .collect();

        let pmessages = if request.store {
            // if stateful alway send all the messages
            let mut nmessages = request.messages.clone();
            nmessages.extend(imessages);
            nmessages
        } else if imessages.is_empty() {
            request.messages
        } else {
            imessages
        };

        for message in pmessages {
            match message {
                Message::Thought { content: _ } => {}
                Message::User { content } => {
                    messages.push(AnthropicCompletionRequestMessage::Content {
                        role: "user".to_string(),
                        content,
                    });
                    // messages.push(amessage);
                }
                Message::Assistant { content } => {
                    messages.push(AnthropicCompletionRequestMessage::Content {
                        role: "assistant".to_string(),
                        content,
                    });
                }
                Message::ToolCall {
                    arguments,
                    call_id,
                    name,
                } => {
                    let value = serde_json::from_str(&arguments)
                        .context("Failed to serialize arguments for OpenAI")?;

                    let content = AnthropicCompletionRequestToolUse {
                        r#type: "tool_use".to_string(),
                        input: value,
                        id: call_id,
                        name,
                    };
                    tool_use_contents.push(content);
                }
                Message::ToolOutput {
                    call_id,
                    output,
                    name: _,
                } => {
                    let arg_string = serde_json::to_string(&output)
                        .context("Failed to serialize arguments for Anthropic")?;

                    let content = AnthropicCompletionRequestToolResult {
                        r#type: "tool_result".to_string(),
                        content: arg_string,
                        tool_use_id: call_id,
                    };
                    tool_result_contents.push(content);
                }
            }
        }

        if !tool_use_contents.is_empty() {
            messages.push(AnthropicCompletionRequestMessage::ToolUse {
                role: "assistant".to_string(),
                content: tool_use_contents,
            });
        }

        if !tool_result_contents.is_empty() {
            messages.push(AnthropicCompletionRequestMessage::ToolResult {
                role: "user".to_string(),
                content: tool_result_contents,
            });
        }

        let mut atools = Vec::new();
        for tool in request.definitions {
            let atool = AnthropicToolDefinition {
                name: tool.name,
                description: tool.description,
                input_schema: tool.parameters,
            };
            atools.push(atool);
        }

        let cache_control = AnthropicCompletionRequestCache {
            r#type: "ephemeral".to_string(),
        };
        let mut thinking = AnthropicThinking::new(request.reasoning_effort);
        let temperature = match arequest.reasoning_effort {
            ReasoningEffort::None => 0.7,
            _ => 1.0,
        };
        let budget_tokens = if thinking.budget_tokens >= request.max_tokens {
            request.max_tokens - 1 // cap thinking, preserve max_tokens
        } else {
            thinking.budget_tokens
        };
        thinking.budget_tokens = budget_tokens;

        debug!("thikning: {:?}", thinking);

        // // if response format schema is available, use it
        // let output_config  = if let Some(response_format_schema) = request.response_format_schema {
        //     let response_format = json!({
        //         "type": "json_schema",
        //         "schema": response_format_schema                
        //     });
        //     Some(AnthropicOutputConfig { format: response_format })
        // } else {
        //     None
        // };
        let output_config = None;

        let arequest = AnthropicCompletionRequest {
            max_tokens: request.max_tokens,
            messages,
            model: request.model,
            cache_control,
            system: request.system,
            temperature,
            thinking,
            stream: request.stream,
            tools: atools,
            output_config
        };

        Ok(arequest)
    }
}
