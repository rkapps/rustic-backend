use anyhow::{Context, Result};
use serde::Serialize;
use tracing::{debug, info, trace};

use crate::client::{
    message::Message,
    request::{CompletionRequest, ReasoningEffort},
    tools::ToolDefinition,
};

/// Serialized body sent to `POST /v1/responses`.
#[derive(Serialize, Debug)]
pub struct OpenAICompletionRequest {
    model: String,
    /// Maps to the OpenAI `instructions` field (system prompt).
    instructions: String,
    pub input: Vec<OpenAICompletionRequestMessage>,
    pub store: bool,
    stream: bool,
    /// Links this request to a prior response for conversation threading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    max_output_tokens: i32,
    reasoning: Option<OpenAICompletionRequestReasoning>,
    pub tools: Vec<ToolDefinition>,
}

impl OpenAICompletionRequest {
    pub fn log_info(&self) {
        info!(
            target: "agent-openai",
            model = %self.model,
            store = self.store,
            messages = self.input.len(),
            iterations = %format!("{:#?}", self.input.last()),
            last_response_id = self.previous_response_id,
            tools = self.tools.len(),
            "Openai request"
        );
    }

    pub fn log_debug(&self) {
        debug!(
            target: "agent-openai",
            model = %self.model,
            store = self.store,
            max_tokens = self.max_output_tokens,
            messages = %format!("{:#?}", self.input),
            tools = self.tools.len(),
            "Openai request"
        );
    }

    pub fn log_trace(&self) {
        trace!(
            target: "agent-openai",
            request = %format!("{:#?}", self),
            "Openai full request"
        );
    }
}

/// A single input item in the OpenAI request, serialized without an enum tag.
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum OpenAICompletionRequestMessage {
    /// Plain text user or assistant turn.
    Content { role: String, content: String },
    /// A tool invocation the model previously requested.
    FunctionCall {
        r#type: String,
        arguments: String,
        call_id: String,
        name: String,
    },
    /// The result of a tool invocation.
    FunctionCallOutput {
        r#type: String,
        call_id: String,
        output: String,
    },
}

impl OpenAICompletionRequest {
    /// Convert a provider-agnostic [`CompletionRequest`] into the OpenAI Responses API format.
    ///
    /// The `response_id` from the last `Assistant` message is extracted and sent as
    /// `previous_response_id` so the API can thread conversation context server-side.
    pub fn new(request: CompletionRequest) -> Result<Self> {
        // let mut id: Option<String> = None;
        let mut inputs = Vec::new();
        let mut user_input: Option<OpenAICompletionRequestMessage> = None;
        let iterations = request.iterations;
        let mut sorted_keys: Vec<usize> = iterations.keys().cloned().collect();
        sorted_keys.sort();
        let current_key = iterations.keys().max().copied().unwrap_or(0);
        let current_iteration = iterations.get(&current_key).cloned().unwrap_or_default();

        let imessages: Vec<Message> = sorted_keys
            .iter()
            .flat_map(|k| iterations.get(k).unwrap().clone())
            .collect();

        debug!(
            target: "agent-openai",
            request_messages= ?request.messages.len(),
            iterations_messages = format_args!("{:#?}", imessages)
        );

        let pmessages = if request.store {
            // stateful — last user message + current iteration tool calls
            let mut msgs = request
                .messages
                .last()
                .cloned()
                .map(|m| vec![m])
                .unwrap_or_default();
            msgs.extend(current_iteration);
            msgs
        } else {
            // stateless — all iterations or original messages
            if imessages.is_empty() {
                request.messages
            } else {
                imessages
            }
        };

        for message in pmessages {
            match message {
                Message::Thought { content: _ } => {}
                Message::User { content } => {
                    if request.store {
                        // id = response_id;
                        user_input = Some(OpenAICompletionRequestMessage::Content {
                            role: "user".to_string(),
                            content,
                        });
                    } else {
                        inputs.push(OpenAICompletionRequestMessage::Content {
                            role: "user".to_string(),
                            content,
                        });
                    }
                }
                Message::Assistant { content } => {
                    if request.store {
                        // id = response_id;
                    } else {
                        inputs.push(OpenAICompletionRequestMessage::Content {
                            role: "assistant".to_string(),
                            content,
                        });
                    }
                }

                Message::ToolCall {
                    arguments,
                    call_id,
                    name,
                } => {
                    // if !request.store {
                    inputs.push(OpenAICompletionRequestMessage::FunctionCall {
                        r#type: "function_call".to_string(),
                        arguments,
                        call_id,
                        name,
                    });
                    // }
                }
                Message::ToolOutput {
                    call_id,
                    output,
                    name: _,
                } => {
                    // if !request.store {
                    let arg_string = serde_json::to_string(&output)
                        .context("Failed to serialize arguments for OpenAI")?;

                    inputs.push(OpenAICompletionRequestMessage::FunctionCallOutput {
                        r#type: "function_call_output".to_string(),
                        call_id,
                        output: arg_string,
                    });
                    // }
                }
            }
        }

        // Push user message
        if request.store
            && let Some(input) = user_input
        {
            inputs.push(input);
        }

        let response_id = if request.store {
            request.last_response_id.filter(|id| !id.is_empty())
        } else {
            None
        };
        Ok(Self {
            model: request.model,
            instructions: request.system.unwrap_or_default(),
            input: inputs,
            store: request.store,
            stream: request.stream,
            previous_response_id: response_id,
            max_output_tokens: request.max_tokens,
            reasoning: OpenAICompletionRequestReasoning::new(request.reasoning_effort),
            tools: request.definitions,
        })
    }
}

/// Maps [`ReasoningEffort`] to the OpenAI `reasoning.effort` string field.
#[derive(Serialize, Debug)]
pub struct OpenAICompletionRequestReasoning {
    effort: String,
}

impl OpenAICompletionRequestReasoning {
    /// Build from a [`ReasoningEffort`] level: `None` → `"none"`, …, `High` → `"high"`.
    pub fn new(reasoning_effort: ReasoningEffort) -> Option<Self> {
        let effort = match reasoning_effort {
            ReasoningEffort::None => return None,
            ReasoningEffort::Low => "low".to_string(),
            ReasoningEffort::Medium => "medium".to_string(),
            ReasoningEffort::High => "high".to_string(),
        };
        Some(OpenAICompletionRequestReasoning { effort })
    }
}
