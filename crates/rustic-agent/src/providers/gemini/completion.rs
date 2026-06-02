use anyhow::Result;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::header::HeaderValue;
use rustic_core::{HttpClient, HttpError, HttpResult};
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
    providers::gemini::{
        GEMINI_BASE_URL,
        request::GeminiInteractionsRequest,
        response::{
            GeminiInteractionsChunkResponse, GeminiInteractionsResponse,
            GeminiInteractionsResponseOutput::{FunctionCall, Text, Thought},
        },
    },
};

/// [`LlmClient`] implementation for the Google Gemini Interactions API.
///
/// Translates [`CompletionRequest`] into Gemini's wire format, handles SSE streaming,
/// and normalises the response into provider-agnostic types.
#[derive(Debug)]
pub struct GeminiClient {
    pub api_key: String,
    pub base_url: String,
    http_client: HttpClient,
}

impl GeminiClient {
    /// Create a client targeting the default Gemini API endpoint.
    pub fn new(api_key: String) -> Result<Self> {
        Ok(Self {
            api_key,
            base_url: GEMINI_BASE_URL.to_string(),
            http_client: HttpClient::new()?,
        })
    }

    /// Send a blocking completion request to `POST /v1beta/interactions` and normalise
    /// the response, mapping Gemini's token-usage fields to the canonical
    /// [`CompletionResponseTokenUsage`] shape.
    async fn complete_interactions(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionResponse> {
        let url = format!("{}/v1beta/interactions", self.base_url,);

        let mut headers = reqwest::header::HeaderMap::new();
        let agent_id = request.id.clone();
        let api_key: HeaderValue = self
            .api_key
            .parse()
            .map_err(|_| HttpError::ApiKeyParsingFailed)?;

        headers.insert("x-goog-api-key", api_key);
        headers.insert("Api-Revision", HeaderValue::from_static("2026-05-07"));

        let grequest = GeminiInteractionsRequest::new(request)
            .map_err(|e| HttpError::CompletionRequestError(e.to_string()))?;

        debug!(target: "agent-gemini", "Gemini complete_interactions: {:#?}", grequest);

        let body = serde_json::json!(grequest);
        let gresponse = self
            .http_client
            .post_request::<GeminiInteractionsResponse>(url, Some(headers), body)
            .await?;

        debug!(target: "agent-gemini", "GeminiCompletionResponse: {:#?}", gresponse);

        let id = gresponse.id;
        let mut rcontents: Vec<CompletionResponseContent> = Vec::new();

        for output in gresponse.outputs {
            match output {
                Text { text } => {
                    let rcontent = CompletionResponseContent::Text(text);
                    rcontents.push(rcontent);
                }
                FunctionCall {
                    arguments,
                    id,
                    name,
                } => {
                    let rcontent = CompletionResponseContent::ToolCall(ToolCallRequest {
                        id,
                        name,
                        arguments,
                    });
                    rcontents.push(rcontent);
                }
                Thought { signature } => {
                    let rcontent: CompletionResponseContent =
                        CompletionResponseContent::Thought(signature);
                    rcontents.push(rcontent);
                }
            }
        }

        let cusage = gresponse.usage;
        let usage = CompletionResponseTokenUsage {
            input_tokens: cusage.total_input_tokens - cusage.total_cached_tokens,
            cached_read_tokens: cusage.total_cached_tokens,
            cached_write_tokens: 0,
            tool_use_tokens: cusage.total_tool_use_tokens,
            output_tokens: cusage.total_output_tokens, // Gemini already excludes thought tokens here
            reasoning_tokens: cusage.total_thought_tokens,
            total_tokens: (cusage.total_input_tokens - cusage.total_cached_tokens)  // fresh input
        + cusage.total_cached_tokens                                         // cache reads
        + cusage.total_tool_use_tokens                                       // tools
        + cusage.total_output_tokens                                         // visible output
        + cusage.total_thought_tokens, // reasoning
        };

        let cresponse = CompletionResponse {
            id: agent_id.clone(),
            model: gresponse.model,
            response_id: id.unwrap_or_default(),
            contents: rcontents,
            usage,
        };

        Ok(cresponse)
    }
}

#[async_trait]
impl LlmClient for GeminiClient {
    async fn complete(&self, request: CompletionRequest) -> HttpResult<CompletionResponse> {
        self.complete_interactions(request).await
    }

    async fn complete_with_stream(
        &self,
        request: CompletionRequest,
    ) -> HttpResult<CompletionStreamResponse> {
        let url = format!("{}/v1beta/interactions", self.base_url,);

        let agent_id = request.id.clone();
        let mut headers = reqwest::header::HeaderMap::new();

        let api_key: HeaderValue = self
            .api_key
            .parse()
            .map_err(|_| HttpError::ApiKeyParsingFailed)?;
        headers.insert("x-goog-api-key", api_key);
        headers.insert("Api-Revision", HeaderValue::from_static("2026-05-07"));

        let grequest = GeminiInteractionsRequest::new(request)
            .map_err(|e| HttpError::CompletionRequestError(e.to_string()))?;
        debug!(target: "agent-gemini", request= %format_args!("{:#?}", grequest), "Gemini Completion");

        let body = serde_json::json!(grequest);
        trace!(target: "agent-gemini", body= ?body, "Gemini Completion body");
        let response = self
            .http_client
            .post_stream_request(url, Some(headers), body)
            .await?;

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

             while let Some(event_result) = event_stream.next().await {
                let event = match event_result {
                    Ok(e) => e,
                    Err(e) => {
                        yield Err(HttpError::NetworkError(e.to_string()));
                        break;
                    }
                };

                if event.data.contains("[DONE]") {
                    yield Ok(CompletionChunkResponse::default());
                    break;
                }
                let chunk: GeminiInteractionsChunkResponse = serde_json::from_str(&event.data)
                    .map_err(|e| {
                        HttpError::Other(format!(
                            "GeminiChunkResponse error: {:?} for data {:?}",
                            e, &event.data
                        ))
                    })?;

                match chunk.event_type.as_str() {
                    "content.delta" => {
                        if let Some(delta) = chunk.delta {
                            let dtype = delta.r#type;
                            // debug!("Type: {}", dtype);
                            if let Some(text) = delta.text {
                                yield Ok(CompletionChunkResponse::content(agent_id.clone(), text, String::new()))
                            } else if let Some(signature) = delta.signature {
                                // debug!("chunk: {:#?}", signature);
                                debug!(target: "agent-gemini", signature= ?signature, "Chunk Signature");

                                yield Ok(CompletionChunkResponse::thought(agent_id.clone(), signature))
                            } else if dtype == "function_call" {
                                yield Ok(CompletionChunkResponse::tool_call(
                                    agent_id.clone(),
                                    delta.id,
                                    delta.name,
                                    delta.arguments,
                                ))
                            }
                        }
                    }
                    "interaction.complete" => {
                        if let Some(interaction) = chunk.interaction {
                            let cusage = interaction.usage.unwrap();

                         //    info!("chunk token: {:#?}", cusage);
                          let usage = CompletionResponseTokenUsage {
                             input_tokens: cusage.total_input_tokens - cusage.total_cached_tokens,
                             cached_read_tokens: cusage.total_cached_tokens,
                             cached_write_tokens: 0,
                             tool_use_tokens: cusage.total_tool_use_tokens,
                             output_tokens: cusage.total_output_tokens,  // Gemini already excludes thought tokens here
                             reasoning_tokens: cusage.total_thought_tokens,
                             total_tokens: (cusage.total_input_tokens - cusage.total_cached_tokens)  // fresh input
                                 + cusage.total_cached_tokens                                         // cache reads
                                 + cusage.total_tool_use_tokens                                       // tools
                                 + cusage.total_output_tokens                                         // visible output
                                 + cusage.total_thought_tokens,                                       // reasoning
                         };

                    debug!(target: "agent-gemini", model= ?interaction.model, response_id= ?interaction.id, usage= ?usage, "Response stats");

                    yield Ok(CompletionChunkResponse::stop(
                            agent_id.clone(),
                                interaction.model,
                                interaction.id,
                                Some(usage),
                            ))
                        }
                    }
                    _ => yield Ok(CompletionChunkResponse::default()),
                }

            }

        };

        Ok(Box::pin(stream))
    }
}
