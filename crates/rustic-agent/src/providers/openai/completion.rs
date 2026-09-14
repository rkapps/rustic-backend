use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use rustic_core::{HttpClient, HttpError, HttpResult};
use serde_json::Value;
use tracing::{debug, error, info, trace};

use crate::{
    client::{
        llm::{CompletionStreamResponse, LlmClient},
        request::CompletionRequest,
        response::CompletionChunkResponse,
        tools::ToolCallRequest,
        usage::TokenUsage,
    },
    providers::openai::{
        OPENAI_BASE_URL,
        request::{OpenAICompletionsRequest, OpenAIRequest},
        response::{
            OpenAIChunkResponseData, OpenAICompletionsChunkResponse, OpenAICompletionsUsage,
        },
    },
};

/// [`LlmClient`] implementation for the OpenAI Responses API.
///
/// Translates [`CompletionRequest`] into OpenAI's wire format, handles SSE streaming,
/// and normalises the response into provider-agnostic types.
#[derive(Debug)]
pub struct OpenAIClient {
    pub api_key: String,
    pub base_url: String,
    http_client: HttpClient,
    use_responses_api: bool,
}

#[async_trait]
impl LlmClient for OpenAIClient {
    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {
        if self.use_responses_api {
            self.complete_with_stream_responses_api(request).await
        } else {
            self.complete_with_stream_chat_completions(request).await
        }
    }
}

impl OpenAIClient {
    /// Create a client targeting the default OpenAI API endpoint.
    pub fn new(api_key: String) -> Result<Self> {
        Ok(Self {
            api_key,
            base_url: OPENAI_BASE_URL.to_string(),
            http_client: HttpClient::new()?,
            use_responses_api: true,
        })
    }

    /// Create a openai compatible client like Groq
    pub fn new_with_base_url(base_url: String, api_key: String) -> Result<Self> {
        Ok(Self {
            api_key,
            base_url,
            http_client: HttpClient::new()?,
            use_responses_api: true,
        })
    }

    /// Create a openai compatible client like Groq
    pub fn new_with_chat_completions(base_url: String, api_key: String) -> Result<Self> {
        Ok(Self {
            api_key,
            base_url,
            http_client: HttpClient::new()?,
            use_responses_api: false,
        })
    }

    // get completion usage from openai usage
    fn get_usage(cusage: &OpenAICompletionsUsage) -> TokenUsage {
        // let cusage = oresponse.usage;
        let cached = cusage
            .prompt_tokens_details
            .as_ref()
            .map(|d| d.cached_tokens)
            .unwrap_or(0);

        let reasoning = cusage
            .completion_tokens_details
            .as_ref()
            .map(|d| d.reasoning_tokens)
            .unwrap_or(0);

        TokenUsage {
            input_tokens: cusage.prompt_tokens - cached,
            cached_read_tokens: cached,
            cached_write_tokens: 0, // not in OpenAI chat completions format
            tool_use_tokens: 0,     // not broken out separately in chat completions
            reasoning_tokens: reasoning,
            output_tokens: cusage.completion_tokens - reasoning,
            total_tokens: cusage.total_tokens,
        }
    }

    async fn complete_with_stream_chat_completions(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {
        let url = format!("{}/chat/completions", self.base_url,);

        let mut headers = reqwest::header::HeaderMap::new();

        let agent_id = request.id.clone();
        let bearer = format!("Bearer {}", self.api_key)
            .parse()
            .map_err(|_| HttpError::ApiKeyParsingFailed)?;
        let event_stream = "text/event-stream"
            .parse()
            .map_err(|_| HttpError::ApiVersionParsingFailed)?;

        let accept_encoding = "identify"
            .parse()
            .map_err(|_| HttpError::ApiVersionParsingFailed)?;

        headers.insert("Authorization", bearer);
        headers.insert("Accept", event_stream);
        headers.insert("Accept-Encoding", accept_encoding);

        info!(
            target: "agent-openai",
            messages = %format!("{:#?}", request.messages),
            "Completion Request"
        );

        let request = OpenAICompletionsRequest::new(request.clone())
            .map_err(|e| HttpError::CompletionRequestError(e.to_string()))?;

        request.log_info();
        request.log_debug();
        request.log_trace();

        let body = serde_json::json!(request);
        let response = self
            .http_client
            .post_stream_request(url, Some(headers), body)
            .await?;
        // debug!("✅ Got response: {:?}", response.error_for_status());
        trace!(
            target: "agent-openai",
            response = %format!("{:#?}", response),
            "Openai Response"
        );

        if response.status() == 400 {
            let error_body = response
                .text()
                .await
                .map_err(|e| HttpError::NetworkError(e.to_string()))?;

            error!("❌ API ERROR BODY: {}", error_body);
            return Err(HttpError::InvalidRequest(error_body));
        }

        let mut event_stream = response.bytes_stream().eventsource();

        let stream = async_stream::stream! {

             let mut finish_reason = false;
             let mut usage = None;
             let mut pending_tool_calls: HashMap<i32, (String, String, String)> = HashMap::new();

             while let Some(event_result) = event_stream.next().await {
                let event = match event_result {
                    Ok(e) => e,
                    Err(e) => {
                        yield Err(HttpError::NetworkError(e.to_string()));
                        break;
                    }
                };

                trace!(
                    target: "agent-openai",
                    "Event: {:?}", event.data
                );

                if event.data.contains("[DONE]") {
                    yield Ok(CompletionChunkResponse::default());
                    break;
                }
                let chunk: OpenAICompletionsChunkResponse =
                    serde_json::from_str(&event.data).map_err(|e| {
                        HttpError::Other(format!(
                            "OpenAIChunkResponse error: {:?} for data {:?}",
                            e, &event.data
                        ))
                    })?;


                // usage = chunk.usage.clone();
                if let Some(cusage) = chunk.usage.clone() {
                    usage = Some(OpenAIClient::get_usage(&cusage));
                }

                let choices = chunk.choices.clone();

                // If choices is empty and finish_reason is true, then send the pending tools (Some source models - GLM)
                if choices.is_empty() {
                    debug!(
                        target: "agent-openai",
                        "Chunk: {:?}", chunk
                    );

                    if finish_reason {

                        info!(
                            target: "agent-openai",
                            "Tool calls: {}", pending_tool_calls.len()
                        );

                        for (id, name, arguments) in pending_tool_calls.values() {
                            if id.is_empty() || name.is_empty() {
                                debug!(target: "agent-openai", "Skipping tool call — missing id or name");
                                continue;
                            }

                            let args = serde_json::from_str(arguments)
                                .unwrap_or(Value::Object(Default::default()));

                            yield Ok(CompletionChunkResponse::tool_call(
                                agent_id.clone(),
                                Some(id.clone()),
                                Some(name.clone()),
                                Some(args),
                            ));
                        }
                        pending_tool_calls.clear();

                        yield Ok(CompletionChunkResponse::stop(
                            agent_id.clone(),
                            String::new(),
                            String::new(),
                            String::new(),
                            usage.clone(),
                        ))
                    }
                }

                for choice in choices {
                    let delta = choice.delta.clone();
                    trace!(
                        target: "agent-openai",
                        _pending_tools = ?pending_tool_calls.len(),
                        "Choice: {:?}", choice
                    );

                    // Accumulate any tool_calls present in THIS chunk first — some
                    // providers (ASI:One) deliver id/name/arguments and finish_reason
                    // together in a single chunk, with no prior partial deltas.
                    if let Some(tool_calls) = &delta.tool_calls {
                        for tool_call in tool_calls {
                            let index = tool_call.index.unwrap_or(0);
                            let entry = pending_tool_calls
                                .entry(index)
                                .or_insert((String::new(), String::new(), String::new()));

                            if let Some(id) = &tool_call.id { entry.0 = id.clone(); }
                            if let Some(name) = &tool_call.function.name { entry.1 = name.clone(); }

                            if let Some(args) = &tool_call.function.arguments && !args.is_empty() {
                                entry.2.push_str(args);
                            }
                        }
                    }

                    if let Some(reason) = &choice.finish_reason {
                        finish_reason = true;

                        if reason == "tool_calls" {
                            for (id, name, arguments) in pending_tool_calls.values() {
                                if id.is_empty() || name.is_empty() {
                                    continue;
                                }
                                let args = serde_json::from_str(arguments)
                                    .unwrap_or(Value::Object(Default::default()));
                                yield Ok(CompletionChunkResponse::tool_call(
                                    agent_id.clone(),
                                    Some(id.clone()),
                                    Some(name.clone()),
                                    Some(args),
                                ));
                            }
                            pending_tool_calls.clear();
                            finish_reason = false;
                        } else if reason == "stop" {
                            yield Ok(CompletionChunkResponse::stop(
                                agent_id.clone(),
                                String::new(), String::new(), String::new(),
                                usage.clone(),
                            ));
                        } else if reason == "length" {
                            yield Err(HttpError::Other(
                                "Response truncated — model hit max_tokens limit...".to_string()
                            ));
                            break;
                        }
                        continue;
                    }

                    let content = delta.content.unwrap_or_default();
                    if !content.is_empty() {
                        yield Ok(CompletionChunkResponse::content(agent_id.clone(), content, String::new()))
                    }
                    // tool_calls already accumulated above — no more else-if needed here
                }
            };
        };

        Ok(Box::pin(stream))
    }

    async fn complete_with_stream_responses_api(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {
        let url = format!("{}/v1/responses", self.base_url,);

        let mut headers = reqwest::header::HeaderMap::new();

        let agent_id = request.id.clone();
        let bearer = format!("Bearer {}", self.api_key)
            .parse()
            .map_err(|_| HttpError::ApiKeyParsingFailed)?;
        let event_stream = "text/event-stream"
            .parse()
            .map_err(|_| HttpError::ApiVersionParsingFailed)?;

        let accept_encoding = "identify"
            .parse()
            .map_err(|_| HttpError::ApiVersionParsingFailed)?;

        headers.insert("Authorization", bearer);
        headers.insert("Accept", event_stream);
        headers.insert("Accept-Encoding", accept_encoding);

        let request = OpenAIRequest::new(request.clone())
            .map_err(|e| HttpError::CompletionRequestError(e.to_string()))?;

        request.log_info();
        request.log_debug();
        request.log_trace();

        let body = serde_json::json!(request);
        let response = self
            .http_client
            .post_stream_request(url, Some(headers), body)
            .await?;
        // debug!("✅ Got response: {:?}", response.error_for_status());
        trace!(
            target: "agent-openai",
            response = %format!("{:#?}", response),
            "Openai Response"
        );

        if response.status() == 400 {
            let error_body = response
                .text()
                .await
                .map_err(|e| HttpError::NetworkError(e.to_string()))?;

            error!("❌ API ERROR BODY: {}", error_body);
            return Err(HttpError::InvalidRequest(error_body));
        }

        let mut event_stream = response.bytes_stream().eventsource();

        let stream = async_stream::stream! {

             let mut tool_buffers: HashMap<String, ToolCallRequest> = HashMap::new();
             let mut tool_values: HashMap<String, String> = HashMap::new();

             while let Some(event_result) = event_stream.next().await {
                let event = match event_result {
                    Ok(e) => e,
                    Err(e) => {
                        yield Err(HttpError::NetworkError(e.to_string()));
                        break;
                    }
                };

                 let chunk: OpenAIChunkResponseData =
                     serde_json::from_str(&event.data).map_err(|e| {
                         HttpError::Other(format!(
                             "OpenAIChunkResponse error: {:?} for data {:?}",
                             e, &event.data
                         ))
                     })?;
                trace!(target: "agent-openai", "delta: {:?}", event.event.as_str());
                trace!(target: "agent-openai", "Chunk: {:?}", chunk);

                match event.event.as_str() {
                     "response.output_text.delta" => {
                        trace!(target: "agent-openai", "Chunk: {:?}", chunk);
                        if let Some(delta) = chunk.delta {
                             yield Ok(CompletionChunkResponse::content(agent_id.clone(), delta, String::new()))
                         }
                     }
                     "response.function_call_arguments.delta" => {
                         if let Some(item_id) = chunk.item_id
                            && let Some(args) = chunk.delta {
                                debug!(target: "agent-openai", "Arguments: {}", args);
                                if let Some(value) = tool_values.get_mut(&item_id) {
                                    value.push_str(&args);
                                }
                            }

                     }
                     "response.function_call_arguments.done" => {
                         if let Some(item_id) = chunk.item_id

                            && let Some(tool_value) = tool_values.get_mut(&item_id) {

                                let args_value: serde_json::Value = serde_json::from_str(tool_value).unwrap_or_else(|_| serde_json::json!({}));
                                if let Some(buffer) = tool_buffers.get_mut(&item_id) {

                                    yield Ok(CompletionChunkResponse::tool_call(
                                         agent_id.clone(),
                                         Some(buffer.id.clone()),
                                         Some(buffer.name.clone()),
                                         Some(args_value)
                                     ))
                                }
                            }

                     }
                     "response.output_item.added" => {
                         if let Some(item) = chunk.item
                             && item.r#type == "function_call" {
                                 trace!(target: "agent-openai", "item: {:?}", item);

                                 tool_buffers.insert(
                                     item.id.clone(),
                                     ToolCallRequest {
                                         name: item.name.unwrap(),
                                         id: item.call_id.unwrap(),
                                         arguments: Value::String("".to_string()),
                                     }
                                 );
                                tool_values.insert(item.id, String::new());

                             }

                     }

                     "response.completed" => {


                         if let Some(response) = chunk.response {
                             let cusage = response.usage.unwrap();
                            debug!(target: "agent-openai", "chunk token: {:#?}", cusage);

                             let usage = TokenUsage {
                                input_tokens: cusage.input_tokens,
                                 cached_read_tokens: cusage.input_tokens_details.cached_tokens,
                                 cached_write_tokens: 0,
                                 output_tokens: cusage.output_tokens,
                                 reasoning_tokens: cusage.output_tokens_details.reasoning_tokens,
                                 tool_use_tokens: 0,
                                 total_tokens: cusage.total_tokens,
                             };

                              debug!(target: "agent-openai", "Response stats - model: {:#?} response_id: {} usage: {:#?}",
                                response.model, response.id, usage );

                                let id = if request.store {
                                    response.id.clone()
                                } else {
                                    String::new()
                                };

                             yield Ok(CompletionChunkResponse::stop(
                                 agent_id.clone(),
                                 response.model,
                                 id,
                                 String::new(),
                                 Some(usage),
                             ))
                         }
                     }
                     _ => yield Ok(CompletionChunkResponse::default()),
                 }
            };
        };

        Ok(Box::pin(stream))
    }
}
