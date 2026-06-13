use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use rustic_core::{HttpClient, HttpError, HttpResult};
use serde_json::Value;
use tracing::{debug, error, trace};

use crate::{
    client::{
        llm::{CompletionStreamResponse, LlmClient},
        request::CompletionRequest,
        response::{
            CompletionChunkResponse, CompletionResponse, CompletionResponseContent,
            CompletionResponseTokenUsage,
        },
        tools::ToolCallRequest,
    },
    providers::openai::{
        OPENAI_BASE_URL,
        request::OpenAICompletionRequest,
        response::{
            OpenAIChunkResponseData, OpenAICompletionResponse,
            OpenAICompletionResponseOutput::{FunctionCall, Message, Reasoning},
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
}

impl OpenAIClient {
    /// Create a client targeting the default OpenAI API endpoint.
    pub fn new(api_key: String) -> Result<Self> {
        Ok(Self {
            api_key,
            base_url: OPENAI_BASE_URL.to_string(),
            http_client: HttpClient::new()?,
        })
    }
}

#[async_trait]
impl LlmClient for OpenAIClient {
    async fn complete(&self, request: CompletionRequest) -> HttpResult<CompletionResponse> {
        let url = format!("{}/v1/responses", self.base_url,);

        let agent_id = request.id.clone();
        let mut headers = reqwest::header::HeaderMap::new();
        let bearer = format!("Bearer {}", self.api_key)
            .parse()
            .map_err(|_| HttpError::ApiKeyParsingFailed)?;

        headers.insert("Authorization", bearer);

        let orequest = OpenAICompletionRequest::new(request)
            .map_err(|e| HttpError::CompletionRequestError(e.to_string()))?;

        debug!(target: "agent-openai", 
            store= ?orequest.store,
            response_id = ?orequest.previous_response_id,
            "OpenAICompletionRequest: {:#?}", orequest.input.len()
        );
        

        let body = serde_json::json!(orequest);
        // debug!("Body: {:#?}", body);
        let oresponse = self
            .http_client
            .post_request::<OpenAICompletionResponse>(url, Some(headers), body)
            .await?;

        debug!(
            target: "agent-openai",
            "OpenAICompletionResponse: {:#?}", oresponse
        );

        let mut rcontents: Vec<CompletionResponseContent> = Vec::new();
        let id = oresponse.id;

        for output in oresponse.output {
            match output {
                Message {
                    id: _,
                    status,
                    content,
                } => {
                    if status == "completed" {
                        for content in content {
                            if content.r#type == "output_text" {
                                let rcontent = CompletionResponseContent::Text(content.text);
                                rcontents.push(rcontent);
                                break;
                            }
                        }
                    }
                }
                FunctionCall {
                    status,
                    arguments,
                    call_id,
                    name,
                } => {
                    if status == "completed" {
                        let arguments: Value = match serde_json::from_str(arguments.as_str()) {
                            Ok(c) => c,
                            Err(e) => {
                                return Err(HttpError::Other(format!(
                                    "Error parsing function arguments: {:#?}",
                                    e
                                )));
                            }
                        };

                        let rcontent = CompletionResponseContent::ToolCall(ToolCallRequest {
                            id: call_id,
                            name,
                            arguments,
                        });
                        rcontents.push(rcontent);
                    }
                }
                Reasoning { id: _, summary: _ } => {}
            }
        }

        let cusage = oresponse.usage;
        let usage = CompletionResponseTokenUsage {
            input_tokens: cusage.input_tokens - cusage.input_tokens_details.cached_tokens, // fresh only
            cached_read_tokens: cusage.input_tokens_details.cached_tokens,
            cached_write_tokens: 0,
            tool_use_tokens: 0,
            output_tokens: cusage.output_tokens - cusage.output_tokens_details.reasoning_tokens, // visible only
            reasoning_tokens: cusage.output_tokens_details.reasoning_tokens,
            total_tokens: (cusage.input_tokens - cusage.input_tokens_details.cached_tokens)
                + cusage.input_tokens_details.cached_tokens
                + cusage.output_tokens_details.reasoning_tokens
                + (cusage.output_tokens - cusage.output_tokens_details.reasoning_tokens),
        };

        let cresponse = CompletionResponse {
            id: agent_id,
            model: oresponse.model,
            response_id: id,
            contents: rcontents,
            usage,
        };

        Ok(cresponse)
    }

    async fn complete_with_stream(
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

        let request = OpenAICompletionRequest::new(request)
            .map_err(|e| HttpError::CompletionRequestError(e.to_string()))?;

        debug!(
            target: "agent-openai", 
            "OpenAI Request: {:#?}", request
        );

        let body = serde_json::json!(request);
        let response = self
            .http_client
            .post_stream_request(url, Some(headers), body)
            .await?;

        // debug!("✅ Got response: {:?}", response.error_for_status());
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

                             let usage = CompletionResponseTokenUsage {
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

                             yield Ok(CompletionChunkResponse::stop(
                                 agent_id.clone(),
                                 response.model,
                                 response.id,
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
