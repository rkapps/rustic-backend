use std::{collections::HashMap, sync::Arc, time::Duration};

use rustic_core::{HttpError, HttpResult};
use serde_json::Value;
use tokio::{
    sync::{Semaphore, mpsc},
    time::sleep,
};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{Instrument, debug, error, info, trace};

use crate::{
    agents::helper::unwrap_typed_value, client::{
        llm::LlmClient,
        message::Message,
        request::{CompletionRequest, ReasoningEffort},
        response::{CompletionChunkResponse, CompletionResponse, CompletionResponseContent},
        tools::{ToolCallRequest, ToolDefinition},
    }, tools::{mcp::MCPRegistry, tool::ToolRegistry},
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
}

impl Agent {
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
            _store = %self.store,
            _temperature = %self.temperature,
        )
    )]
    pub async fn complete_with_streaming(
        &self,
        messages: &[Message],
        last_response_id: Option<String>,
    ) -> HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>> {
        let (tx, rx) = mpsc::channel::<Result<CompletionChunkResponse, HttpError>>(100);

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

        tokio::spawn(
            async move {
                let mut iteration = 0;
                const MAX_ITERATIONS: usize = 10;

                let mut usage = crate::client::response::CompletionResponseTokenUsage::default();

                debug!(
                    target: "agent-messages",
                    "Current messages: {:#?}", messages
                );

                loop {
                    iteration += 1;
                    if iteration > MAX_ITERATIONS {
                        break;
                    }

                    let iter_span = tracing::span!(
                        tracing::Level::INFO,
                        "iteration",
                        otel.name = format!("iteration: {}", iteration),  // ← OTel specific attribute that overrides span name
                        _n = %iteration,
                        _last_response_id = ?last_response_id,
                        _messages= ?iterations.get(&iteration),
                    );
                    // let _enter = iter_span.enter();

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

                    let mut tool_calls = Vec::new();

                    let mut model = String::new();

                    // Gemini sends partial thought tokens as random-looking characters; accumulate
                    // the full thought before appending it as a Thought message so the model receives
                    // a coherent block on the next turn.
                    let mut thought_content = String::new();
                    let mut stream_error = false;

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
                            tool_calls.push(call);
                        } else {
                            trace!(
                                _chunk= ?chunk,
                            );

                            if chunk.is_final {
                                debug!(
                                    _chunk= ?chunk,
                                );

                                usage += chunk.usage.unwrap_or_default();
                                last_response_id = Some(chunk.response_id);
                                model = chunk.model;
                            } else if !chunk.content.is_empty() {
                                let _ = tx.send(Ok(chunk)).await;
                            } else if !chunk.thought.is_empty() {
                                thought_content.push_str(&chunk.thought);
                                // while antropic thoughts are text, gemini are random characters. we need to collect the thoughts because
                                // gemini requires the thoughts to be sent back.
                                // Do not send the chunks for now..
                                // let _ = tx.send(Ok(chunk)).await;
                            }
                        }
                    }

                    // break outer loop on stream error
                    if stream_error {
                        break;
                    }
                    
                    iter_span.in_scope(|| {
                        info!(
                            _tool_calls= %tool_calls.len(),
                            _new_response_id= ?last_response_id,
                        );
                    });

                    if tool_calls.is_empty() {
                        iter_span.in_scope(|| {
                            info!(
                                _usage= %format_args!("{:#?}", usage),
                                "Response: {}", model
                            );
                        });

                        let _ = tx
                            .send(Ok(CompletionChunkResponse::stop(
                                agent_id.clone(),
                                model,
                                last_response_id.clone().unwrap_or_default(),
                                Some(usage),
                            )))
                            .await;
                        break;
                    }
                    let tool_futures: Vec<_> = tool_calls
                        .into_iter()
                        .map(|call| {
                            let span = tracing::info_span!(
                                parent: &iter_span,
                                "tool.execute",
                                otel.name = format!("tool: {}", call.name),
                                _tool = %call.name,
                                _call_id = %call.id,
                            );
                            agent.execute_tool_call(call.clone()).instrument(span)
                        })
                        .collect();

                    let _ = tx
                        .send(Ok(CompletionChunkResponse::content(
                            agent_id.clone(),
                            String::new(),
                            "Executing tools...".into(),
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

                    for result in results {
                        match result {
                            Ok((tool_call, tool_output)) => {
                                nmessages.push(tool_call);
                                nmessages.push(tool_output);
                            }
                            Err(e) => {
                                error!(
                                    target: "agent-tool",
                                    "Tool Call Error: {:?}", e
                                );
                            }
                        };
                    }

                    if !nmessages.is_empty() {
                        iter_span.in_scope(|| {
                            info!(
                                nmessages= ?nmessages.len(),
                            );
                        });
                        iterations.insert(iteration, nmessages);
                    }
                }
            }
            .instrument(tracing::Span::current()),
        );

        Ok(ReceiverStream::new(rx))
    }

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
            _last_response_id = ?last_response_id,
            _max_tokens = %self.max_tokens,
            _messages.count = %messages.len(),
            _model = %self.model,
            _provider = %self.llm,
            _reasoning_effort= ?self.reasoning_effort,
            _store = %self.store,
            _temperature = %self.temperature,
        )
    )]
    pub async fn complete(
        &self,
        messages: &[Message],
        last_response_id: Option<String>,
    ) -> HttpResult<CompletionResponse> {
        let agent_id = &self.id;

        let mut definitions: Vec<ToolDefinition> = self
            .tool_registry
            .get_tools()
            .iter()
            .map(|e| ToolDefinition::from_tool(e.as_ref()))
            .collect();

        debug!("Current messages: {:?}", messages);
        trace!(
            target: "agent-tool",
            "Tool definitions: {:?}", definitions
        );

        let mcp_definitions = self.mcp_registry.definitions.clone();

        trace!(
            target: "agent-tool",
            "Mcp_definitions: {:?}", mcp_definitions
        );

        mcp_definitions
            .iter()
            .for_each(|e| definitions.push(e.1.clone()));

        let mut last_response_id = last_response_id.clone();
        let mut iterations = HashMap::new();

        let request = CompletionRequest {
            id: self.id.clone(),
            provider: self.llm.clone(),
            model: self.model.clone(),
            system: self.system_prompt.clone(),
            messages: messages.to_vec(),
            iterations: iterations.clone(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: false,
            store: self.store,
            reasoning_effort: self.reasoning_effort.clone(),
            enable_cache: self.enable_cache,
            definitions,
            last_response_id: None,
            response_format_schema: self.response_format_schema.clone(),
        };

        const MAX_ITERATIONS: usize = 10;
        let mut iteration = 0;

        let mut nrequest = request;
        let delay = Duration::from_millis(2000);

        loop {

            let iter_span = tracing::span!(
                    tracing::Level::INFO,
                    "iteration",
                    otel.name = format!("iteration: {}", iteration),  // ← OTel specific attribute that overrides span name
                    n = %iteration,
                    _last_response_id = ?last_response_id,
                    _messages= format_args!("{:#?}", messages),
                    _iterations= format_args!("{:#?}", iterations.get(&iteration)),
            );
            // let _enter = iter_span.enter();

            iteration += 1;
            if iteration > 5 {
                sleep(delay).await;
            }

            if iteration > MAX_ITERATIONS {
                iter_span.in_scope(|| {
                    error!(
                        "Agent: {}, Max tool iterations exceeded: {}",
                        agent_id, iteration
                    );
                });

                return Err(HttpError::MaxIterationsExceeded);
            }

            iter_span.in_scope(|| {
                trace!("CompletionRequest: {:#?}", nrequest);
            });

            // Call the llm with the request
            nrequest.last_response_id = last_response_id.clone();
            nrequest.iterations = iterations.clone();


            let response = self
                .client
                .complete(nrequest.clone())
                // .instrument(tracing::Span::current())
                .instrument(tracing::info_span!(
                    parent: &iter_span,
                    "provider.complete",
                    otel.name = format!("provider: {}", nrequest.model),
                    _model = %nrequest.model,
                    _store = %nrequest.store,
                ))
                .await?;
            last_response_id = Some(response.response_id.clone());

            // Get the tools
            let tool_calls: Vec<&ToolCallRequest> = response
                .contents
                .iter()
                .filter_map(|c| {
                    if let CompletionResponseContent::ToolCall(call) = c {
                        Some(call)
                    } else {
                        None
                    }
                })
                .collect();

            iter_span.in_scope(|| {
                info!(
                    _new_response_id= ?last_response_id,
                    "Tool calls: {}", tool_calls.len()
                );
            });

            if tool_calls.is_empty() {
                iter_span.in_scope(|| {
                    info!(
                        response= %format_args!("{:#?}", response.text() ),
                        usage= %format_args!("{:#?}", response.usage),
                        "Response Stats final"
                    );
                });

                return Ok(response); // Done - return final answer
            }

            iter_span.in_scope(|| {
                trace!("CompletionResponse: {:#?}", response);
            });

            let thoughts: Vec<Message> = response
                .contents
                .iter()
                .filter_map(|c| {
                    if let CompletionResponseContent::Thought(text) = c {
                        Some(Message::Thought {
                            content: text.clone(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            let semaphore = Arc::new(Semaphore::new(3)); // max 3 parallel

            let tool_futures: Vec<_> = tool_calls
                .into_iter()
                .map(|call| {
                    let sem = semaphore.clone();
                    let timeout_duration = Duration::from_secs(120);
                    let span = tracing::info_span!(
                        parent: &iter_span,
                        "tool.execute",
                        otel.name = format!("tool: {}", call.name),
                        _tool = %call.name,
                        _call_id = %call.id,
                    );

                    async move {
                        let _permit = sem.acquire().await.unwrap();
                        match tokio::time::timeout(
                            timeout_duration,
                            self.execute_tool_call(call.clone()),
                        )
                        .await
                        {
                            Ok(result) => result,
                            Err(_) => Err(anyhow::anyhow!("Timeout: {}", call.name)),
                        }
                    }
                    .instrument(span)
                })
                .collect();

            let results = futures::future::join_all(tool_futures)
                .instrument(tracing::Span::current())
                .await;

            //Add thoughts to the messages first
            let mut nmessages: Vec<Message> = Vec::new();
            nmessages.extend(thoughts);
            for result in results {
                match result {
                    Ok((tool_call, tool_output)) => {
                        nmessages.push(tool_call);
                        nmessages.push(tool_output);
                    }
                    Err(e) => {
                        iter_span.in_scope(|| {
                            error!(target: "agent-tool", agent= %agent_id, error= ?e, "Tool Call Error");
                        });
                    }
                };
            }

            iter_span.in_scope(|| {
                info!(
                    _last_response_id = ?last_response_id,
                    _new_mesages = ?nmessages.len()
                );
            });

            if !nmessages.is_empty() {
                iterations.insert(iteration, nmessages);
            }
        }
    }

    /// Dispatch a single tool call and return the resulting `(ToolCall, ToolOutput)` message pair.
    ///
    /// Resolution order: local [`ToolRegistry`] first, then [`MCPRegistry`]. If the tool is not
    /// found in either registry a JSON error payload is returned to the model so it can recover
    /// gracefully rather than crashing the loop.
    async fn execute_tool_call(&self, call: ToolCallRequest) -> anyhow::Result<(Message, Message)> {
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
            _output= format_args!("{:?}", serde_json::to_string_pretty(&output)),
            "Tool output: {:?}", call.name
        );

        let tool_output_message = Message::ToolOutput {
            call_id: call.id.clone(),
            output,
            name: call.name.clone(),
        };

        Ok((tool_call_message, tool_output_message))
    }
}
