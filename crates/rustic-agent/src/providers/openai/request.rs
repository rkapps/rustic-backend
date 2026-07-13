use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};
use tracing::{debug, info, trace};

use crate::client::{
    message::Message,
    request::{CompletionRequest, ReasoningEffort},
    tools::ToolDefinition,
};

/// Serialized body sent to `POST /v1/responses`.
#[derive(Serialize, Debug)]
pub struct OpenAIRequest {
    model: String,
    /// Maps to the OpenAI `instructions` field (system prompt).
    instructions: String,
    pub input: Vec<OpenAIMessage>,
    pub store: bool,
    stream: bool,
    /// Links this request to a prior response for conversation threading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    max_output_tokens: i32,
    reasoning: Option<OpenAIRequestReasoning>,
    pub tools: Vec<ToolDefinition>,
    pub text: Option<OpenAIRequestText>,
}

impl OpenAIRequest {
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
pub enum OpenAIMessage {
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

impl OpenAIRequest {
    /// Convert a provider-agnostic [`CompletionRequest`] into the OpenAI Responses API format.
    ///
    /// The `response_id` from the last `Assistant` message is extracted and sent as
    /// `previous_response_id` so the API can thread conversation context server-side.
    pub fn new(request: CompletionRequest) -> Result<Self> {
        // let mut id: Option<String> = None;
        let mut inputs = Vec::new();
        let mut user_input: Option<OpenAIMessage> = None;
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
                        user_input = Some(OpenAIMessage::Content {
                            role: "user".to_string(),
                            content,
                        });
                    } else {
                        inputs.push(OpenAIMessage::Content {
                            role: "user".to_string(),
                            content,
                        });
                    }
                }
                Message::Assistant { content } => {
                    if request.store {
                        // id = response_id;
                    } else {
                        inputs.push(OpenAIMessage::Content {
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
                    inputs.push(OpenAIMessage::FunctionCall {
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

                    inputs.push(OpenAIMessage::FunctionCallOutput {
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

        // if response format schema is available, use it
        let text = if let Some(response_format_schema) = request.response_format_schema {
            let response_format = json!({
                "type": "json_schema",
                "name" : "response_object",
                "schema": response_format_schema
            });
            Some(OpenAIRequestText {
                format: response_format,
            })
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
            reasoning: OpenAIRequestReasoning::new(request.reasoning_effort),
            tools: request.definitions,
            text,
        })
    }
}

/// Maps [`ReasoningEffort`] to the OpenAI `reasoning.effort` string field.
#[derive(Serialize, Debug)]
pub struct OpenAIRequestReasoning {
    effort: String,
}

impl OpenAIRequestReasoning {
    /// Build from a [`ReasoningEffort`] level: `None` → `"none"`, …, `High` → `"high"`.
    pub fn new(reasoning_effort: ReasoningEffort) -> Option<Self> {
        let effort = match reasoning_effort {
            ReasoningEffort::None => return None,
            ReasoningEffort::Low => "low".to_string(),
            ReasoningEffort::Medium => "medium".to_string(),
            ReasoningEffort::High => "high".to_string(),
        };
        Some(OpenAIRequestReasoning { effort })
    }
}

/// Maps [`ReasoningEffort`] to the OpenAI `reasoning.effort` string field.
#[derive(Serialize, Debug)]
pub struct OpenAIRequestText {
    format: Value,
}

/// Serialized body sent to `POST /v1/responses`.
#[derive(Serialize, Debug)]
pub struct OpenAICompletionsRequest {
    model: String,
    pub messages: Vec<OpenAICompletionsMessage>,
    stream: bool,
    max_output_tokens: i32,
    reasoning: Option<OpenAIRequestReasoning>,
    pub tools: Vec<OpenAICompletionsToolDefinition>,
    pub text: Option<OpenAIRequestText>,
}

/// A single input item in the OpenAI request, serialized without an enum tag.
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum OpenAICompletionsMessage {
    /// Plain text user or assistant turn.
    Content { role: String, content: String },
    /// Assistant message with tool calls
    ToolCall {
        role: String,
        content: Option<String>,
        tool_calls: Vec<OpenAICompletionsToolCall>,
    },
    /// Tool result
    ToolResult {
        role: String,
        tool_call_id: String,
        content: String,
    },
}

/// Serialized body sent to `POST /v1/responses`.
#[derive(Serialize, Debug)]
pub struct OpenAICompletionsToolCall {
    pub id: String,
    pub r#type: String,
    pub function: OpenAICompletionsToolFunction,
}

/// Serialized body sent to `POST /v1/responses`.
#[derive(Serialize, Debug)]
pub struct OpenAICompletionsToolFunction {
    pub name: String,
    pub arguments: String,
}

/// Serialized body sent to `POST /v1/responses`.
#[derive(Serialize, Debug)]
pub struct OpenAICompletionsToolDefinition {
    pub r#type: String,
    pub function: ToolDefinition,
}

impl OpenAICompletionsRequest {
    pub fn log_info(&self) {
        info!(
            target: "agent-openai",
            model = %self.model,
            messages = self.messages.len(),
            iterations = %format!("{:#?}", self.messages.last()),
            tools = self.tools.len(),
            "Openai request"
        );
    }

    pub fn log_debug(&self) {
        debug!(
            target: "agent-openai",
            model = %self.model,
            max_tokens = self.max_output_tokens,
            messages = %format!("{:#?}", self.messages),
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

    /// Convert a provider-agnostic [`CompletionRequest`] into the OpenAI Responses API format.
    ///
    /// The `response_id` from the last `Assistant` message is extracted and sent as
    /// `previous_response_id` so the API can thread conversation context server-side.
    pub fn new(request: CompletionRequest) -> Result<Self> {
        let mut messages = Vec::new();
        messages.push(OpenAICompletionsMessage::Content {
            role: "system".to_string(),
            content: request.system.unwrap_or_default(),
        });

        let mut tool_calls = Vec::new();
        let mut results = Vec::new();

        let iterations = request.iterations;
        let mut sorted_keys: Vec<usize> = iterations.keys().cloned().collect();
        sorted_keys.sort();

        let imessages: Vec<Message> = sorted_keys
            .iter()
            .flat_map(|k| iterations.get(k).unwrap().clone())
            .collect();

        for message in imessages {
            match message {
                Message::Thought { content: _ } => {}
                Message::User { content } => {
                    messages.push(OpenAICompletionsMessage::Content {
                        role: "user".to_string(),
                        content,
                    });
                }
                Message::Assistant { content } => {
                    messages.push(OpenAICompletionsMessage::Content {
                        role: "assistant".to_string(),
                        content,
                    });
                }
                Message::ToolCall {
                    arguments,
                    call_id,
                    name,
                } => {
                    // // parse string to Value for local models that expect an object
                    // let args_value: Value = serde_json::from_str(&arguments)
                    //     .unwrap_or(Value::Object(Default::default()));

                    tool_calls.push(OpenAICompletionsToolCall {
                        id: call_id,
                        r#type: "function".to_string(),
                        function: OpenAICompletionsToolFunction {
                            name,
                            arguments: arguments,
                        },
                    });
                }
                Message::ToolOutput {
                    call_id,
                    output,
                    name: _,
                } => {
                    let arg_string = serde_json::to_string(&output)
                        .context("Failed to serialize arguments for OpenAI")?;
                    results.push(OpenAICompletionsMessage::ToolResult {
                        role: "tool".to_string(),
                        tool_call_id: call_id,
                        content: arg_string,
                    });
                }
            }
        }

        if !tool_calls.is_empty() {
            messages.push(OpenAICompletionsMessage::ToolCall {
                role: "assistant".to_string(),
                content: None,
                tool_calls,
            });
            messages.extend(results);
        }

        let mut tools = Vec::new();
        for definition in request.definitions {
            let tool = OpenAICompletionsToolDefinition {
                r#type: "function".to_string(),
                function: definition.clone(),
            };
            tools.push(tool);
        }

        // if response format schema is available, use it
        let text = if let Some(response_format_schema) = request.response_format_schema {
            let response_format = json!({
                "type": "json_schema",
                "name" : "response_object",
                "schema": response_format_schema
            });
            Some(OpenAIRequestText {
                format: response_format,
            })
        } else {
            None
        };
        Ok(Self {
            max_output_tokens: request.max_tokens,
            messages,
            model: request.model,
            reasoning: OpenAIRequestReasoning::new(request.reasoning_effort),
            stream: request.stream,
            text,
            tools,
        })
    }
}
