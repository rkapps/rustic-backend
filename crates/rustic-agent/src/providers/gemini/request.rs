use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::{
    client::{
        message::Message,
        request::{CompletionRequest, ReasoningEffort},
        tools::ToolDefinition,
    },
    providers::gemini::{MODEL_GEMINI_3_FLASH_PREVIEW, helper::clean_for_gemini},
};

/// Serialized body sent to `POST /v1beta/interactions`.
#[derive(Debug, Serialize)]
pub struct GeminiInteractionsRequest {
    model: String,
    input: Vec<GeminiCompletionRequestInput>,
    /// Links this request to a prior interaction for conversation threading.
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_interaction_id: Option<String>,
    system_instruction: String,
    stream: bool,
    store: bool,
    generation_config: GeminiCompletionRequestConfig,
    pub tools: Vec<ToolDefinition>,
}

/// A single input item in the Gemini request, serialized without an enum tag.
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum GeminiCompletionRequestInput {
    /// Plain text user turn.
    Content { role: String, content: String },
    /// Model turn that may contain thoughts and/or function calls interleaved.
    ModelContent {
        role: String,
        content: Vec<GeminiModelContent>,
    },
    /// User turn carrying tool results back to the model.
    FunctionCallResult {
        role: String,
        content: Vec<GeminiCompletionRequestFunctionResult>,
    },
}

/// Generation parameters (temperature, token limit, thinking level).
#[derive(Serialize, Debug)]
pub struct GeminiCompletionRequestGenerationConfig {
    pub temperature: f32,
    pub max_output_tokens: i32,
}

/// A content block within a model turn, serialized without an enum tag.
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum GeminiModelContent {
    /// A thought (chain-of-thought) block; `signature` must be echoed back as-is.
    Thought { r#type: String, signature: String },
    /// A function-call block the model produced in a prior turn.
    FunctionCall {
        r#type: String,
        id: String,
        name: String,
        arguments: Value,
    },
}

/// A function-call input block (unused — superseded by [`GeminiModelContent::FunctionCall`]).
#[derive(Debug, Serialize)]
pub struct GeminiCompletionRequestFunctionCall {
    r#type: String,
    arguments: Value,
    id: String,
    name: String,
}

/// A tool result returned by the caller to the model.
#[derive(Debug, Serialize)]
pub struct GeminiCompletionRequestFunctionResult {
    r#type: String,
    call_id: String,
    result: String,
    name: String,
}

impl GeminiInteractionsRequest {
    /// Convert a provider-agnostic [`CompletionRequest`] into the Gemini Interactions API format.
    ///
    /// Thought signatures are preserved and bundled into model-role content blocks so Gemini
    /// can resume its chain-of-thought across turns. Tool results are collected and emitted as
    /// a single user-role `FunctionCallResult` block.
    pub fn new(request: CompletionRequest) -> Result<Self> {
        let mut inputs = Vec::new();
        let mut function_result_contents = Vec::new();
        let mut model_contents: Vec<GeminiModelContent> = Vec::new();
        let mut user_input: Option<GeminiCompletionRequestInput> = None;
        let mut id: Option<String> = None;

        let crequest = request.clone();
        for message in request.messages {
            match message {
                Message::Thought { content } => {
                    model_contents.push(GeminiModelContent::Thought {
                        r#type: "thought".to_string(),
                        signature: content,
                    });
                }

                Message::User {
                    content,
                    response_id: _,
                } => {
                    // Flush any pending model contents before new user message
                    if !model_contents.is_empty() {
                        inputs.push(GeminiCompletionRequestInput::ModelContent {
                            role: "model".to_string(),
                            content: std::mem::take(&mut model_contents),
                        });
                    }

                    // if state only add the last user
                    if request.store {
                        user_input = Some(GeminiCompletionRequestInput::Content {
                            role: "user".to_string(),
                            content,
                        });
                    } else {
                        let user_input1 = GeminiCompletionRequestInput::Content {
                            role: "user".to_string(),
                            content,
                        };
                        inputs.push(user_input1);
                    }
                }

                Message::Assistant {
                    content,
                    response_id,
                } => {
                    id = response_id;

                    // only add assistant if it is a stateless
                    if !request.store {
                        let user_input = GeminiCompletionRequestInput::Content {
                            role: "model".to_string(),
                            content,
                        };
                        inputs.push(user_input);
                    }
                }

                Message::ToolCall {
                    arguments,
                    call_id,
                    name,
                } => {
                    let value = serde_json::from_str(&arguments)
                        .context("Failed to serialize arguments for Gemini")?;
                    model_contents.push(GeminiModelContent::FunctionCall {
                        r#type: "function_call".to_string(),
                        id: call_id,
                        name,
                        arguments: value,
                    });
                }

                Message::ToolOutput {
                    call_id,
                    output,
                    name,
                } => {
                    let arg_string = serde_json::to_string(&output)
                        .context("Failed to serialize arguments for Gemini")?;
                    function_result_contents.push(GeminiCompletionRequestFunctionResult {
                        r#type: "function_result".to_string(),
                        call_id,
                        name,
                        result: arg_string,
                    });
                }
            }
        }

        // for stateless push the alst input
        if request.store {
            // Push user message
            if let Some(input) = user_input {
                inputs.push(input);
            }
        }

        // Push model message with thought + function calls combined
        if !model_contents.is_empty() {
            inputs.push(GeminiCompletionRequestInput::ModelContent {
                role: "model".to_string(),
                content: model_contents,
            });
        }

        // Push tool results
        if !function_result_contents.is_empty() {
            inputs.push(GeminiCompletionRequestInput::FunctionCallResult {
                role: "user".to_string(),
                content: function_result_contents,
            });
        }

        let grequest = GeminiInteractionsRequest {
            model: MODEL_GEMINI_3_FLASH_PREVIEW.to_string(),
            input: inputs,
            system_instruction: request.system.clone().unwrap_or_default(),
            previous_interaction_id: id,
            stream: request.stream,
            store: request.store,
            generation_config: GeminiCompletionRequestConfig::new(&crequest),
            tools: request
                .definitions
                .into_iter()
                .map(|mut def| {
                    // clean top-level description
                    def.description = def
                        .description
                        .replace('\n', " ")
                        .split_whitespace()
                        .collect::<Vec<&str>>()
                        .join(" ");
                    // clean parameters recursively (handles nested descriptions too)
                    def.parameters = clean_for_gemini(&def.parameters);
                    def
                })
                .collect(),
        };
        Ok(grequest)
    }
}

/// Legacy generate-content request shape (kept for compatibility; not actively used).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCompletionRequest {
    #[serde(rename = "system_instruction")]
    system_instruction: GeminiCompletionRequestSystemInstruction,
    contents: Vec<GeminiCompletionRequestContent>,
    generation_config: GeminiCompletionRequestConfig,
    stream: bool,
}

#[derive(Debug, Serialize)]
pub struct GeminiCompletionRequestSystemInstruction {
    parts: Vec<GeminiCompletionRequestPart>,
}

#[derive(Debug, Serialize)]
pub struct GeminiCompletionRequestContent {
    role: String,
    parts: Vec<GeminiCompletionRequestPart>,
}

#[derive(Debug, Serialize)]
pub struct GeminiCompletionRequestPart {
    text: String,
}

/// Generation configuration for the Interactions API.
///
/// `thinking_level` maps [`ReasoningEffort`] to Gemini's `"minimal"`/`"low"`/`"medium"`/`"high"` strings.
#[derive(Debug, Serialize)]
pub struct GeminiCompletionRequestConfig {
    pub temperature: f32,
    pub max_output_tokens: i32,
    pub thinking_level: String,
    pub thinking_summaries: String,
}

impl GeminiCompletionRequestConfig {
    /// Build config from a [`CompletionRequest`], translating [`ReasoningEffort`] to Gemini's string level.
    pub fn new(request: &CompletionRequest) -> Self {
        let thinking_level = match request.reasoning_effort {
            ReasoningEffort::None => "minimal".to_string(),
            ReasoningEffort::Low => "low".to_string(),
            ReasoningEffort::Medium => "medium".to_string(),
            ReasoningEffort::High => "high".to_string(),
        };
        let thinking_summaries = "auto".to_string();

        GeminiCompletionRequestConfig {
            temperature: request.temperature,
            max_output_tokens: request.max_tokens,
            thinking_level,
            thinking_summaries,
        }
    }
}
