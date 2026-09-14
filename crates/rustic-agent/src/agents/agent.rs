use std::{collections::HashMap, sync::Arc};

use rustic_core::{HttpError, HttpResult};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{Instrument, debug, error, info, trace};

use crate::{
    TokenUsage,
    agents::{
        domain::{AgentChunkResponse, AgentIteration, AgentResponse, AgentToolCall},
        helper::{merge_tool_output, unwrap_typed_value},
    },
    client::{
        llm::LlmClient,
        message::Message,
        request::{CompletionRequest, ReasoningEffort},
        tools::{ToolCallRequest, ToolDefinition},
    },
    tools::{mcp::MCPRegistry, tool::ToolRegistry},
};

/// Orchestrates LLM completion calls and tool dispatching for a single configured model.
///
/// `Agent` is the main entry point for running multi-turn conversations. It holds a reference
/// to an [`LlmClient`] and two tool registries ([`ToolRegistry`] for in-process tools,
/// [`MCPRegistry`] for remote MCP servers) and exposes four completion modes:
///
/// | Method | Tools | Streaming |
/// |--------|-------|-----------|
/// | [`complete`](Self::complete) | no | no |
/// | [`complete_with_stream`](Self::complete_with_stream) | no | yes |
/// | [`complete_with_tools`](Self::complete_with_tools) | yes | no |
/// | [`complete_with_tools_streaming`](Self::complete_with_tools_streaming) | yes | yes |
#[derive(Debug, Clone)]
pub struct Agent {
    /// Unique identifier for this agent instance, used in log lines and response payloads.
    pub id: String,
    /// Provider label (e.g. `"Anthropic"`) used for logging and routing.
    pub llm: String,
    /// Model identifier forwarded to the provider (e.g. `"claude-sonnet-4-6"`).
    pub model: String,
    /// The underlying LLM backend.
    pub client: Arc<dyn LlmClient>,
    /// System prompt prepended before every conversation.
    pub system_prompt: Option<String>,
    /// Sampling temperature; higher values increase output randomness.
    pub temperature: f32,
    /// Hard cap on tokens in each completion response.
    pub max_tokens: i32,
    /// Whether the provider should persist the conversation for multi-turn continuations.
    pub store: bool,
    /// When `true`, the provider is asked to cache the prompt.
    pub enable_cache: bool,
    /// Controls how much chain-of-thought reasoning the model performs before answering.
    pub reasoning_effort: ReasoningEffort,
    /// Registry of in-process tools the agent can call.
    pub tool_registry: Arc<ToolRegistry>,
    /// Registry of remote MCP server tools the agent can call.
    pub mcp_registry: Arc<MCPRegistry>,
    pub response_format_schema: Option<Value>,
    pub relay_tool_output: bool,
}

impl Agent {
    /// Run an agentic tool-use loop and return the final [`CompletionResponse`].
    ///
    /// The loop repeats up to `MAX_ITERATIONS` times; a 2-second delay is inserted
    /// after iteration 5 to back off from rate limits. Tool calls are executed
    /// concurrently with a semaphore limiting parallelism to 3 and a 60-second
    /// per-call timeout.
    ///
    /// Returns [`HttpError::MaxIterationsExceeded`] if the model keeps requesting
    /// tools beyond the iteration cap.
    ///
    #[tracing::instrument(
        skip(self, messages, last_response_id),
        fields(
            otel.name = %format!("complete agent: {}", self.id),
x        )
    )]
    pub async fn complete(
        &self,
        messages: &[Message],
        last_response_id: Option<String>,
    ) -> HttpResult<AgentResponse> {
        let stream = self
            .complete_with_streaming(messages, last_response_id.clone())
            .await?;
        tokio::pin!(stream);

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(AgentChunkResponse::Final { response }) => {
                    return Ok(response);
                }
                Ok(_) => {
                    // skip content, thought, status chunks
                }
                Err(e) => return Err(e),
            }
        }

        Err(HttpError::Other(
            "Stream ended without final chunk".to_string(),
        ))
    }

    /// Run an agentic tool-use loop, streaming output chunks to the caller.
    ///
    /// Spawns a background Tokio task that drives the loop and forwards
    /// [`CompletionChunkResponse`] items through an `mpsc` channel (capacity 100).
    /// The loop repeats up to `MAX_ITERATIONS` times; on each iteration it:
    ///
    /// 1. Calls the LLM with the current message history and tool definitions.
    /// 2. Forwards visible content chunks to the caller immediately.
    /// 3. If the model requests tool calls, executes them concurrently and appends
    ///    their results to the message history before the next iteration.
    /// 4. When no tool calls are requested, sends a final [`CompletionChunkResponse::stop`]
    ///    chunk with accumulated token usage and exits.
    #[tracing::instrument(
        skip(self, messages, last_response_id),
        fields(
            otel.name = %format!("complete_with_streaming agent: {}", self.id),
            _last_response_id = ?last_response_id,
            _last_message= ?messages.last(),
            _max_tokens = %self.max_tokens,
            _messages.count = %messages.len(),
            _model = %self.model,
            _provider = %self.llm,
            _reasoning_effort= ?self.reasoning_effort,
            _relay_tool_output= %self.relay_tool_output,
            _store = %self.store,
            _temperature = %self.temperature,
        )
    )]
    pub async fn complete_with_streaming(
        &self,
        messages: &[Message],
        last_response_id: Option<String>,
    ) -> HttpResult<ReceiverStream<HttpResult<AgentChunkResponse>>> {
        let (tx, rx) = mpsc::channel::<Result<AgentChunkResponse, HttpError>>(100);

        //Get tool definitions
        let mut definitions: Vec<ToolDefinition> = self
            .tool_registry
            .get_tools()
            .iter()
            .map(|e| ToolDefinition::from_tool(e.as_ref()))
            .collect();

        trace!(
            target: "agent-tool",
            "Tool definitions: {:?}", definitions,
        );

        let mcp_definitions = self.mcp_registry.definitions.clone();
        trace!(
            target: "agent-tool",
            "Mcp_definitions: {:?}", mcp_definitions
        );

        mcp_definitions
            .iter()
            .for_each(|e| definitions.push(e.1.clone()));

        // Clone Arcs and Data for the background task
        let agent = self.clone();
        let system_prompt = self.system_prompt.clone();
        let new_definitions = definitions.clone();
        let agent_id = self.id.clone();
        let messages = messages.to_vec();
        let mut last_response_id = last_response_id.clone();
        let mut iterations = HashMap::new();
        let relay_tool_output = self.relay_tool_output;

        tokio::spawn(
            async move {
                let mut iteration: usize = 0;
                const MAX_ITERATIONS: usize = 10;

                // collect iterations throughout the loop
                let mut tracked_iterations: Vec<AgentIteration> = vec![];
                let mut total_usage = TokenUsage::default();

                debug!(
                    target: "agent-messages",
                    "Current messages: {:#?}", messages
                );

                loop {
                    iteration += 1;
                    if iteration > MAX_ITERATIONS {
                        break;
                    }
                    info!("Agent: {} Iteration: {:?}", agent.id, iteration);

                    let iter_span = tracing::span!(
                        tracing::Level::INFO,
                        "iteration",
                        otel.name = format!("iteration: {}", iteration),  // ← OTel specific attribute that overrides span name
                        _n = %iteration,
                        _last_response_id = ?last_response_id,
                        _messages= ?iterations.get(&iteration),
                    );
                    let _enter = iter_span.enter();
                    iter_span.in_scope(|| {
                        debug!(
                        _iteration = %iteration,
                        _last_response_id = ?last_response_id,
                        _messages= ?iterations.get(&iteration),
                        );
                    });

                    let request = CompletionRequest {
                        id: agent_id.clone(),
                        provider: agent.llm.clone(),
                        model: agent.model.clone(),
                        system: system_prompt.clone(),
                        messages: messages.clone(),
                        iterations: iterations.clone(),
                        temperature: agent.temperature,
                        max_tokens: agent.max_tokens,
                        reasoning_effort: agent.reasoning_effort.clone(),
                        enable_cache: agent.enable_cache,
                        stream: true,
                        store: agent.store,
                        definitions: new_definitions.clone(),
                        last_response_id: last_response_id.clone(),
                        response_format_schema: agent.response_format_schema.clone(),
                    };

                    let mut llm_stream = match agent
                        .client
                        .complete_with_stream(request)
                        .instrument(tracing::info_span!(
                            parent: &iter_span,
                            "provider.complete",
                            otel.name = format!("provider: {}", agent.model),
                            _store = %agent.store,
                        ))
                        .await
                    {
                        Ok(s) => s,
                        Err(e) => {
                            let _ = tx.send(Err(HttpError::NetworkError(e.to_string()))).await;
                            break;
                        }
                    };

                    let mut tool_call_requests = Vec::new();
                    let mut model = String::new();

                    // Gemini sends partial thought tokens as random-looking characters; accumulate
                    // the full thought before appending it as a Thought message so the model receives
                    // a coherent block on the next turn.
                    let mut thought_content = String::new();
                    let mut final_content = String::new();
                    let mut stream_error = false;
                    let mut usage = TokenUsage::default();

                    let start = std::time::Instant::now();

                    // 2. "Pump" the chunks through the channel as they arrive
                    while let Some(chunk_result) = llm_stream.next().await {
                        let chunk = match chunk_result {
                            Ok(chunk) => chunk,
                            Err(e) => {
                                tracing::error!("Stream chunk error: {}", e);
                                let _ = tx.send(Err(HttpError::NetworkError(e.to_string()))).await;
                                stream_error = true;
                                break;
                            }
                        };

                        if let Some(call) = chunk.tool_call {
                            debug!(agent= %agent_id, tool_call= ?call, "Tool Call");
                            tool_call_requests.push(call);
                        } else if chunk.is_final {
                            debug!(
                                _chunk= ?chunk,
                            );

                            usage = chunk.usage.unwrap_or_default();
                            last_response_id = Some(chunk.response_id);
                            model = chunk.model;
                        } else if !chunk.content.is_empty() {
                            final_content.push_str(&chunk.content);
                            let agent_chunk =
                                AgentChunkResponse::content(agent_id.clone(), chunk.content);
                            let _ = tx.send(Ok(agent_chunk)).await;
                        } else if !chunk.thought.is_empty() {
                            thought_content.push_str(&chunk.thought);
                            // while antropic thoughts are text, gemini are random characters. we need to collect the thoughts because
                            // gemini requires the thoughts to be sent back.
                            // Do not send the chunks for now..
                            // let _ = tx.send(Ok(chunk)).await;
                        }
                    }

                    total_usage += usage.clone();
                    let mut tool_calls = Vec::new();

                    // break outer loop on stream error
                    if stream_error {
                        break;
                    }

                    iter_span.in_scope(|| {
                        info!(
                            _final_content= ?final_content,
                            _tool_calls= %tool_call_requests.len(),
                            _new_response_id= ?last_response_id,
                        );
                    });

                    if tool_call_requests.is_empty() {
                        iter_span.in_scope(|| {
                            debug!(
                                _usage= %format_args!("{:#?}", usage),
                                "Response: {}", model
                            );
                        });

                        let agent_iteration = AgentIteration {
                            iteration: iteration as u32,
                            duration_ms: start.elapsed().as_millis() as u64,
                            finish_reason: String::new(),
                            response_id: last_response_id.clone().unwrap_or_default(),
                            tool_calls,
                            usage: usage.clone(),
                        };
                        tracked_iterations.push(agent_iteration);

                        let duration_ms = start.elapsed().as_millis() as u64;
                        let agent_response = AgentResponse {
                            agent_id: agent_id.clone(),
                            content: final_content,
                            model,
                            response_id: last_response_id.clone().unwrap_or_default(),
                            iterations: tracked_iterations,
                            usage: total_usage,
                            duration_ms,
                        };

                        let _ = tx
                            .send(Ok(AgentChunkResponse::final_response(agent_response)))
                            .await;
                        break;
                    }
                    let tool_futures: Vec<_> = tool_call_requests
                        .into_iter()
                        .map(|call| {
                            let span = tracing::info_span!(
                                parent: &iter_span,
                                "tool.execute",
                                otel.name = format!("tool: {}", call.name),
                                _tool = %call.name,
                                _call_id = %call.id,
                                _arguments = ?call.arguments
                            );
                            agent.execute_tool_call(call.clone()).instrument(span)
                        })
                        .collect();

                    let _ = tx
                        .send(Ok(AgentChunkResponse::content(
                            agent_id.clone(),
                            String::new(),
                        )))
                        .await;

                    let results = futures::future::join_all(tool_futures).await;

                    //Add thoughts to the messages first
                    let mut nmessages: Vec<Message> = Vec::new();
                    if !thought_content.is_empty() {
                        nmessages.push(Message::Thought {
                            content: thought_content,
                        });
                    }

                    let mut merged = serde_json::Map::new();
                    for result in results {
                        match result {
                            Ok((tool_name, tool_call, tool_output)) => {
                                let duration_ms = start.elapsed().as_millis() as u64;

                                nmessages.push(tool_call.clone());
                                nmessages.push(tool_output.clone());
                                merge_tool_output(&mut merged, &tool_output);

                                // extract input from ToolCall message
                                let input = if let Message::ToolCall { arguments, .. } = &tool_call
                                {
                                    serde_json::from_str(arguments)
                                        .unwrap_or(Value::String(arguments.clone()))
                                } else {
                                    Value::Null
                                };

                                // extract output from ToolOutput message
                                let output =
                                    if let Message::ToolOutput { output, .. } = &tool_output {
                                        Some(output.clone())
                                    } else {
                                        None
                                    };

                                // extract call_id
                                let call_id = if let Message::ToolCall { call_id, .. } = &tool_call
                                {
                                    call_id.clone()
                                } else {
                                    String::new()
                                };
                                tool_calls.push(AgentToolCall {
                                    duration_ms,
                                    error: None,
                                    id: call_id,
                                    name: tool_name,
                                    input,
                                    output,
                                })
                            }
                            Err(e) => {
                                error!(
                                    target: "agent-tool",
                                    "Tool Call Error: {:?}", e
                                );
                            }
                        };
                    }

                    iter_span.in_scope(|| {
                        debug!(
                            _new_messages= ?nmessages.len(),
                        );
                    });
                    let agent_iteration = AgentIteration {
                        iteration: iteration as u32,
                        duration_ms: start.elapsed().as_millis() as u64,
                        finish_reason: String::new(),
                        response_id: last_response_id.clone().unwrap_or_default(),
                        tool_calls,
                        usage: usage.clone(),
                    };
                    tracked_iterations.push(agent_iteration);

                    if relay_tool_output {
                        match serde_json::to_string(&merged) {
                            Ok(c) => {
                                iter_span.in_scope(|| {
                                    debug!(
                                        relay_tool_output_response= %format_args!("{:#?}", c),
                                        usage= %format_args!("{:#?}", usage),
                                        "Response Stats final"
                                    );
                                });

                                let chunk =
                                    AgentChunkResponse::content(agent_id.clone(), c.clone());
                                let _ = tx.send(Ok(chunk)).await;

                                let duration_ms = start.elapsed().as_millis() as u64;
                                let agent_response = AgentResponse {
                                    agent_id: agent_id.clone(),
                                    content: c,
                                    model,
                                    response_id: last_response_id.clone().unwrap_or_default(),
                                    iterations: tracked_iterations,
                                    usage: total_usage,
                                    duration_ms,
                                };

                                let _ = tx
                                    .send(Ok(AgentChunkResponse::final_response(agent_response)))
                                    .await;
                                break;
                            }
                            Err(_) => todo!(),
                        };
                    }

                    if !nmessages.is_empty() {
                        iterations.insert(iteration, nmessages);
                    }
                }
            }
            .instrument(tracing::Span::current()),
        );

        Ok(ReceiverStream::new(rx))
    }

    /// Dispatch a single tool call and return the resulting `(ToolCall, ToolOutput)` message pair.
    ///
    /// Resolution order: local [`ToolRegistry`] first, then [`MCPRegistry`]. If the tool is not
    /// found in either registry a JSON error payload is returned to the model so it can recover
    /// gracefully rather than crashing the loop.
    async fn execute_tool_call(
        &self,
        call: ToolCallRequest,
    ) -> anyhow::Result<(String, Message, Message)> {
        let tool_call_message = Message::ToolCall {
            call_id: call.id.clone(),
            arguments: call.arguments.to_string(),
            name: call.name.clone(),
        };

        let output = match self.tool_registry.get_tool(&call.name) {
            Some(tool) => {
                info!(target: "agent-tool",
                    _arguments= ?call.arguments,
                    "Tool call: {:?}", call.name
                );

                // tool.execute(call.arguments.clone()).await?
                let arguments = unwrap_typed_value(call.arguments.clone());
                tool.execute(arguments).await?
            }
            None => {
                if self.mcp_registry.has_tool(&call.name) {
                    info!(target: "agent-tool",
                        _arguments= ?call.arguments,
                        "MCP Tool call: {:?}", call.name
                    );

                    match self
                        .mcp_registry
                        .call_tool(&call.name, call.arguments.clone())
                        .await
                    {
                        Ok(c) => c,
                        Err(e) => {
                            error!(
                                target: "agent-tool",
                                _arguments= ?call.arguments,
                                "Executing McpTool error: {:?}", e
                            );

                            serde_json::json!({
                                "error": format!("{:?}", e)
                            })
                        }
                    }
                } else {
                    error!(
                        target: "agent-tool",
                        "Tool {} not found...", call.name
                    );

                    serde_json::json!({
                        "error": format!("Tool '{}' is not available", call.name)
                    })
                }
            }
        };

        debug!(
            target: "agent-tool",
            _name= ?call.name.clone(),
            _input=?call.arguments,
            _output= format_args!("{:?}", serde_json::to_string_pretty(&output)),
            "Tool output: {:?}", call.name
        );
        let tool_output_message = Message::ToolOutput {
            call_id: call.id.clone(),
            output,
            name: call.name.clone(),
        };

        Ok((call.name.clone(), tool_call_message, tool_output_message))
    }
}
