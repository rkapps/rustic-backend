use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, info, trace};

use crate::{
    client::{
        message::Message,
        request::{CompletionRequest, ReasoningEffort},
        tools::ToolDefinition,
    },
    providers::gemini::{
        MODEL_GEMINI_3_FLASH_PREVIEW, helper::clean_for_gemini, response::GeminiTextContent,
    },
};

/// Serialized body sent to `POST /v1beta/interactions`.
#[derive(Debug, Serialize)]
pub struct GeminiInteractionsRequest {
    model: String,
    input: Vec<GeminiStepRequestInput>,
    /// Links this request to a prior interaction for conversation threading.
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_interaction_id: Option<String>,
    system_instruction: String,
    stream: bool,
    store: bool,
    generation_config: GeminiCompletionRequestConfig,
    pub tools: Vec<ToolDefinition>,
}

impl GeminiInteractionsRequest {
    pub fn log_info(&self) {
        info!(
            target: "agent-gemini",
            model = %self.model,
            store = self.store,
            messages = self.input.len(),
            last_message = %format!("{:#?}", self.input.last()),
            last_response_id = self.previous_interaction_id,
            tools = self.tools.len(),
            "Gemini request"
        );
    }

    pub fn log_debug(&self) {
        debug!(
            target: "agent-gemini",
            model = %self.model,
            store = self.store,
            temperature = %format!("{:.1}", self.generation_config.temperature),
            thinking_level = %self.generation_config.thinking_level,
            max_tokens = self.generation_config.max_output_tokens,
            messages = %format!("{:#?}", self.input),
            tools = self.tools.len(),
            "Gemini request"
        );
    }

    pub fn log_trace(&self) {
        trace!(
            target: "agent-gemini",
            request = %format!("{:#?}", self),
            "Gemini full request"
        );
    }
}

/// A single input item in the Gemini request, serialized without an enum tag.
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum GeminiStepRequestInput {
    /// input from the uesr
    #[serde(rename = "user_input")]
    UserInput {
        r#type: String,
        content: Vec<GeminiTextContent>,
    },

    /// input from the model
    #[serde(rename = "model_input")]
    ModelInput {
        r#type: String,
        content: Vec<GeminiTextContent>,
    },

    /// Thought
    #[serde(rename = "thought")]
    Thought {
        r#type: String,
        summary: Option<Vec<GeminiTextContent>>,
        signature: String,
    },

    /// A function/tool call requested by the model.
    #[serde(rename = "function_call")]
    FunctionCall {
        r#type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        // result: Option<Vec<GeminiTextContent>>,
        result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        arguments: Option<Value>,
        name: String,
    },
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
        let crequest = request.clone();
        let mut inputs: Vec<GeminiStepRequestInput> = Vec::new();
        let mut function_result_contents: Vec<GeminiStepRequestInput> = Vec::new();
        let model_contents: Vec<GeminiStepRequestInput> = Vec::new();
        let mut user_input: Option<GeminiStepRequestInput> = None;
        let iterations = request.iterations;
        let current_key = iterations.keys().max().copied().unwrap_or(0);
        let current_iteration = iterations.get(&current_key).cloned().unwrap_or_default();
        let mut sorted_keys: Vec<usize> = iterations.keys().cloned().collect();
        sorted_keys.sort();

        let imessages: Vec<Message> = sorted_keys
            .iter()
            .flat_map(|k| iterations.get(k).unwrap().clone())
            .collect();

        debug!(
            target: "agent-openai",
            request_messages= ?request.messages.len(),
            iterations_messages = format_args!("{:#?}", iterations),
            current_iteration = ?current_iteration
        );
        let pmessages = if request.store {
            if current_iteration.is_empty() {
                // first run — send only last message (the goal/prompt)
                request
                    .messages
                    .last()
                    .cloned()
                    .map(|m| vec![m])
                    .unwrap_or_default()
            } else {
                // subsequent iterations — user message + current iteration tool calls
                imessages
            }
        } else
        // stateless - always send all the iterations messages
        if imessages.is_empty() {
            request.messages
        } else {
            imessages
        };

        for message in pmessages {
            match message {
                Message::Thought { content } => {
                    let signature = content;
                    let user_input = GeminiStepRequestInput::Thought {
                        r#type: "thought".to_string(),
                        summary: None,
                        signature,
                    };
                    inputs.push(user_input);
                }

                Message::User { content } => {
                    // Flush any pending model contents before new user message
                    if !model_contents.is_empty() {
                        // inputs.push(GeminiCompletionRequestInput::ModelContent {
                        //     role: "model".to_string(),
                        //     content: std::mem::take(&mut model_contents),
                        // });
                    }
                    if request.store {
                        let content = GeminiTextContent {
                            r#type: "text".to_string(),
                            text: content,
                        };
                        user_input = Some(GeminiStepRequestInput::UserInput {
                            r#type: "user_input".to_string(),
                            content: vec![content],
                        });
                    } else {
                        let content = GeminiTextContent {
                            r#type: "text".to_string(),
                            text: content,
                        };
                        let user_input = GeminiStepRequestInput::UserInput {
                            r#type: "user_input".to_string(),
                            content: vec![content],
                        };
                        inputs.push(user_input);
                    }
                }

                Message::Assistant { content } => {
                    if request.store {
                    } else {
                        let content = GeminiTextContent {
                            r#type: "text".to_string(),
                            text: content,
                        };
                        let user_input = GeminiStepRequestInput::ModelInput {
                            r#type: "model_output".to_string(),
                            content: vec![content],
                        };
                        inputs.push(user_input);
                    }
                }

                Message::ToolCall {
                    arguments: _,
                    call_id: _,
                    name: _,
                } => {}

                Message::ToolOutput {
                    call_id,
                    output,
                    name,
                } => {
                    // let arg_string = serde_json::to_string(&output)
                    //     .context("Failed to serialize arguments for Gemini")?;
                    function_result_contents.push(GeminiStepRequestInput::FunctionCall {
                        r#type: "function_result".to_string(),
                        call_id: Some(call_id),
                        id: None,
                        name,
                        // result: Some(vec![content]),
                        result: Some(output),
                        arguments: None,
                    });
                }
            }
        }

        // Push user message
        if request.store
            && let Some(input) = user_input
        {
            inputs.push(input);
        }

        // Push model message with thought + function calls combined
        if !model_contents.is_empty() {
            inputs.extend(model_contents);
        }

        // Push tool results
        if !function_result_contents.is_empty() {
            inputs.extend(function_result_contents);
        }

        let interaction_id = if request.store {
            request.last_response_id.filter(|id| !id.is_empty())
        } else {
            None
        };

        let grequest = GeminiInteractionsRequest {
            model: MODEL_GEMINI_3_FLASH_PREVIEW.to_string(),
            input: inputs,
            system_instruction: request.system.clone().unwrap_or_default(),
            previous_interaction_id: interaction_id,
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
