use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::header::HeaderValue;
use rustic_core::{
    error::HttpError,
    http::{HttpClient, HttpResult},
};
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
    providers::anthropic::{
        ANTHROPIC_BASE_URL, ANTHROPIC_VERSION,
        request::AnthropicCompletionRequest,
        response::{
            AnthropicChunkResponse, AnthropicCompletionResponse,
            AnthropicCompletionResponseContent::{Text, Thought, ToolUse},
        },
    },
};

/// [`LlmClient`] implementation for the Anthropic Messages API.
///
/// Translates [`CompletionRequest`] into Anthropic's wire format, handles SSE streaming,
/// and normalises the response into provider-agnostic types.
#[derive(Debug)]
pub struct AnthropicClient {
    api_key: String,
    /// Anthropic-Version header value (e.g. `"2023-06-01"`).
    anthropic_version: String,
    base_url: String,
    http_client: HttpClient,
}

impl AnthropicClient {
    /// Create a client targeting the default Anthropic API endpoint.
    pub fn new(api_key: String) -> Result<Self> {
        Ok(Self {
            api_key,
            anthropic_version: ANTHROPIC_VERSION.to_string(),
            base_url: ANTHROPIC_BASE_URL.to_string(),
            http_client: HttpClient::new()?,
        })
    }

    /// Create a client with a custom base URL and API version.
    ///
    /// Useful for local proxies or alternative Anthropic-compatible endpoints
    /// (e.g. Ollama via [`LocalClient`](crate::providers::local::completion::LocalClient)).
    pub fn new_with_base_url(
        api_key: String,
        anthropic_version: String,
        base_url: String,
    ) -> Result<Self> {
        Ok(Self {
            api_key,
            anthropic_version,
            base_url,
            http_client: HttpClient::new()?,
        })
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn complete(&self, request: CompletionRequest) -> HttpResult<CompletionResponse> {
        let url = format!("{}/v1/messages", self.base_url);

        let agent_id = request.id.clone();
        let mut headers = reqwest::header::HeaderMap::new();
        let api_key: HeaderValue = self
            .api_key
            .parse()
            .map_err(|_| HttpError::ApiKeyParsingFailed)?;
        let anthropic_version = self
            .anthropic_version
            .parse()
            .map_err(|_| HttpError::ApiVersionParsingFailed)?;

        headers.insert("x-api-key", api_key);
        headers.insert("anthropic-version", anthropic_version);

        let arequest = AnthropicCompletionRequest::new(request)
            .map_err(|e| HttpError::CompletionRequestError(e.to_string()))?;

        debug!("AnthropicCompletionRequest {:#?}", arequest);

        let body = serde_json::json!(arequest);
        let aresponse = self
            .http_client
            .post_request::<AnthropicCompletionResponse>(url, Some(headers), body)
            .await?;

        debug!("Response: {:#?}", aresponse);

        let mut rcontents: Vec<CompletionResponseContent> = Vec::new();
        for content in aresponse.content {
            match content {
                Text { text } => {
                    let rcontent = CompletionResponseContent::Text(text);
                    rcontents.push(rcontent);
                }
                Thought { thinking } => {
                    let rcontent = CompletionResponseContent::Thought(thinking);
                    rcontents.push(rcontent);
                }
                ToolUse { id, name, input } => {
                    let rcontent = CompletionResponseContent::ToolCall(ToolCallRequest {
                        id,
                        name,
                        arguments: input,
                    });
                    rcontents.push(rcontent);
                }
            }
        }

        let cusage = aresponse.usage;
        let read_input_tokens = cusage.cache_read_input_tokens.unwrap_or_default();
        let creation_input_tokens = cusage.cache_creation_input_tokens.unwrap_or_default();

        let total =
            cusage.input_tokens + read_input_tokens + creation_input_tokens + cusage.output_tokens;

        let usage = CompletionResponseTokenUsage {
            // Fresh tokens + the tokens used to build the cache
            input_tokens: cusage.input_tokens,
            // Tokens saved (Read) or specifically marked as "Written" (Creation)
            cached_read_tokens: read_input_tokens,
            cached_write_tokens: creation_input_tokens,
            tool_use_tokens: 0,
            reasoning_tokens: 0,
            output_tokens: cusage.output_tokens,
            total_tokens: total,
        };

        let cresponse = CompletionResponse {
            id: agent_id,
            model: aresponse.model,
            response_id: String::new(),
            contents: rcontents,
            usage,
        };

        Ok(cresponse)
    }

    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {
        let url = format!("{}/v1/messages", self.base_url);

        let agent_id = request.id.clone();
        let mut headers = reqwest::header::HeaderMap::new();
        let api_key: HeaderValue = self
            .api_key
            .parse()
            .map_err(|_| HttpError::ApiKeyParsingFailed)?;
        let anthropic_version = self
            .anthropic_version
            .parse()
            .map_err(|_| HttpError::ApiVersionParsingFailed)?;
        let event_stream = "text/event-stream"
            .parse()
            .map_err(|_| HttpError::ApiVersionParsingFailed)?;

        headers.insert("x-api-key", api_key);
        headers.insert("anthropic-version", anthropic_version);
        headers.insert("Accept", event_stream);

        let arequest = AnthropicCompletionRequest::new(request)
            .map_err(|e| HttpError::CompletionRequestError(e.to_string()))?;
        debug!("AnthropicCompletionRequest {:#?}", arequest);

        let body = serde_json::json!(arequest);

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
        trace!("response: {:#?}", response);

        let mut event_stream = response.bytes_stream().eventsource();

        let stream = async_stream::stream! {

             let mut tool_buffers: HashMap<i32, ToolCallRequest> = HashMap::new();
             let mut tool_values: HashMap<i32, String> = HashMap::new();

             while let Some(event_result) = event_stream.next().await {
                let event = match event_result {
                    Ok(e) => e,
                    Err(e) => {
                        yield Err(HttpError::NetworkError(e.to_string()));
                        break;
                    }
                };

                let chunk: AnthropicChunkResponse =
                    serde_json::from_str(&event.data).map_err(|e| {
                        HttpError::Other(format!(
                            "AnthropicChunkResponse error: {:?} for data {:?}",
                            e, &event.data
                        ))
                    })?;

                // info!("Chunk: {:?}", chunk);
                // Transform to CompletionChunkResponse
                match chunk.r#type.as_str() {
                    "content_block_start" => {
                        if let Some(content_block) = chunk.clone().content_block {

                            let index = chunk.index.unwrap();
                            // info!("content_block: {:?}", content_block);
                            if content_block.r#type == "tool_use" {
                                let value = content_block.input;

                                tool_buffers.insert(index, ToolCallRequest{
                                    id: content_block.id.unwrap(),
                                    name: content_block.name.unwrap(),
                                    arguments: value.unwrap()
                                });
                                tool_values.insert(index, String::new());
                            }
                            // yield Ok(CompletionChunkResponse::default())

                        } else {
                            // yield Ok(CompletionChunkResponse::default())
                        }
                    }

                    "content_block_delta" => {
                        if let Some(delta) = chunk.delta {
                            let index = chunk.index.unwrap();
                            let dtype = delta.r#type.unwrap();
                            // info!("chunk: {:#?}-{:?}-{:?}", index, dtype, delta.partial_json);
                            match dtype.as_str() {
                                "input_json_delta" => {
                                    if let Some(tool_value) = tool_values.get_mut(&index) {
                                        tool_value.push_str(&delta.partial_json.unwrap());
                                    };
                                }
                                "text_delta" => {
                                    if let Some(text) = delta.text {
                                        trace!("chunk: {:#?}-{:?}-{:?}", index, dtype, text);
                                         yield Ok(CompletionChunkResponse::content(
                                            agent_id.clone(),
                                            text.to_string(),
                                            String::new(),
                                         ))
                                    }
                                }
                                _ => {
                                    if let Some(thinking) = delta.thinking {
                                        // info!("chunk: {:#?}-{:?}-{:?}", index, dtype, thinking);
                                         yield Ok(CompletionChunkResponse::thought(
                                            agent_id.clone(),
                                            thinking,
                                         ))
                                    }
                                }
                            }
                        }
                    }
                    "content_block_stop" => {
                        // info!("Chunk: {:?}", chunk);
                        let index = chunk.index.unwrap();
                        if let Some(tool_value) = tool_values.get_mut(&index) {
                            // info!("tool_value: {:?}", tool_value);
                            let args_value: serde_json::Value = serde_json::from_str(tool_value).unwrap_or_else(|_| serde_json::json!({}));

                            if let Some(buffer) = tool_buffers.get_mut(&index) {
                                    yield Ok(CompletionChunkResponse::tool_call(
                                         agent_id.clone(),
                                         Some(buffer.id.clone()),
                                         Some(buffer.name.clone()),
                                         Some(args_value.clone())
                                     ))
                            }
                       }
                    }

                    "message_delta" => {
                        trace!("Chunk: {:#?}", chunk.usage);
                        let cusage = chunk.usage.unwrap();
                        let read_input_tokens = cusage.cache_read_input_tokens.unwrap_or_default();
                        let creation_input_tokens = cusage.cache_creation_input_tokens.unwrap_or_default();
                        let total = cusage.input_tokens + read_input_tokens + creation_input_tokens + cusage.output_tokens;

                        let usage = CompletionResponseTokenUsage {
                            // Fresh tokens + the tokens used to build the cache
                            input_tokens: cusage.input_tokens,
                            // Tokens saved (Read) or specifically marked as "Written" (Creation)
                            cached_read_tokens: read_input_tokens,
                            cached_write_tokens: creation_input_tokens,
                            tool_use_tokens: 0,
                            reasoning_tokens: 0,
                            output_tokens: cusage.output_tokens,
                            total_tokens: total,
                        };

                        debug!("Response stats - model: {:#?} response_id: {} usage: {:#?}",
                            arequest.model, String::new(), usage
                        );

                        yield Ok(CompletionChunkResponse::stop(
                            agent_id.clone(),
                            arequest.model.clone(),
                            String::new(),
                            Some(usage),
                        ))
                    }
                    _ => yield Ok(CompletionChunkResponse::default()),
                }

            }
        };

        Ok(Box::pin(stream))
    }
}
