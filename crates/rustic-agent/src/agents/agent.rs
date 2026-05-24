use std::{sync::Arc, time::Duration};

use rustic_core::{error::HttpError, http::HttpResult};
use tokio::{
    sync::{Semaphore, mpsc},
    time::sleep,
};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, trace, warn};

use crate::{
    client::{
        llm::LlmClient,
        message::Message,
        request::{CompletionRequest, ReasoningEffort},
        response::{CompletionChunkResponse, CompletionResponse, CompletionResponseContent},
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
    // agent id
    pub id: String,
    /// Provider label (e.g. `"Anthropic"`) used for logging and routing.
    pub llm: String,
    /// Model identifier forwarded to the provider (e.g. `"claude-sonnet-4-6"`).
    pub model: String,
    /// The underlying LLM backend.
    pub client: Arc<dyn LlmClient>,
    /// System prompt prepended before every conversation.
    pub system_prompt: Option<String>,
    pub temperature: f32,
    pub max_tokens: i32,
    /// When `true`, the provider is asked to cache the prompt.
    pub enable_cache: bool,
    pub reasoning_effort: ReasoningEffort,
    /// Registry of in-process tools the agent can call.
    pub tool_registry: Arc<ToolRegistry>,
    /// Registry of remote MCP server tools the agent can call.
    pub mcp_registry: Arc<MCPRegistry>,
}

impl Agent {
    // /// Run a single completion pass without tool use.
    // ///
    // /// Useful for simple chat flows where tool calling is not required.
    // pub async fn complete(&self, messages: &[Message]) -> HttpResult<CompletionResponse> {
    //     let request = CompletionRequest {
    //         model: self.model.clone(),
    //         system: self.system_prompt.clone(),
    //         messages: messages.to_owned(),
    //         temperature: self.temperature,
    //         max_tokens: self.max_tokens,
    //         reasoning_effort: self.reasoning_effort.clone(),
    //         enable_cache: self.enable_cache,
    //         stream: false,
    //         definitions: Vec::new(),
    //     };

    //     self.client.complete(request).await
    // }

    // /// Run a single completion pass without tool use, returning a token stream.
    // pub async fn complete_with_stream(
    //     &self,
    //     messages: &[Message],
    // ) -> HttpResult<CompletionStreamResponse> {
    //     let request = CompletionRequest {
    //         model: self.model.clone(),
    //         system: self.system_prompt.clone(),
    //         messages: messages.to_owned(),
    //         temperature: self.temperature,
    //         max_tokens: self.max_tokens,
    //         reasoning_effort: self.reasoning_effort.clone(),
    //         enable_cache: self.enable_cache,
    //         stream: true,
    //         definitions: Vec::new(),
    //     };

    //     self.client.complete_with_stream(request).await
    // }

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
    pub async fn complete_with_streaming(
        &self,
        messages: &[Message],
    ) -> HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>> {
        let (tx, rx) = mpsc::channel::<Result<CompletionChunkResponse, HttpError>>(100);

        //Get tool definitions
        let mut definitions: Vec<ToolDefinition> = self
            .tool_registry
            .get_tools()
            .iter()
            .map(|e| ToolDefinition::from_tool(e.as_ref()))
            .collect();
        let mcp_definitions = self.mcp_registry.definitions.clone();

        mcp_definitions
            .iter()
            .for_each(|e| definitions.push(e.1.clone()));

        // Clone Arcs and Data for the background task
        let agent = self.clone();
        let mut current_messages = messages.to_owned();
        let system_prompt = self.system_prompt.clone();
        let new_definitions = definitions.clone();
        let agent_id = self.id.clone();

        info!(
            "Agent: {}, Model: {} tokens: {} temperature: {} reasoning_effort: {:?}",
            agent.id, agent.model, agent.max_tokens, agent.temperature, agent.reasoning_effort
        );

        tokio::spawn(async move {
            let mut iteration = 0;
            const MAX_ITERATIONS: usize = 10;

            let last_assistant = current_messages.iter().rev().find_map(|m| {
                if let Message::Assistant { response_id, .. } = m {
                    response_id.clone()
                } else {
                    None
                }
            });
            let mut usage = crate::client::response::CompletionResponseTokenUsage::default();

            trace!("Message: {:#?} ", current_messages);

            loop {
                iteration += 1;
                if iteration > MAX_ITERATIONS {
                    break;
                }
                info!(
                    "Agent: {} Iteration: {} messsages: {} last_response_id: {:?}",
                    agent_id,
                    iteration,
                    current_messages.len(),
                    last_assistant
                );

                let request = CompletionRequest {
                    id: agent_id.clone(),
                    model: agent.model.clone(),
                    system: system_prompt.clone(),
                    messages: current_messages.clone(),
                    temperature: agent.temperature,
                    max_tokens: agent.max_tokens,
                    reasoning_effort: agent.reasoning_effort.clone(),
                    enable_cache: agent.enable_cache,
                    stream: true,
                    definitions: new_definitions.clone(),
                };

                let mut llm_stream = match agent.client.complete_with_stream(request).await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx.send(Err(HttpError::NetworkError(e.to_string()))).await;
                        break;
                    }
                };

                let mut tool_calls = Vec::new();

                let mut model = String::new();
                let mut response_id = String::new();
                let mut thought_content = String::new();
                // let agent_id = agent_id.clone();

                // 2. "Pump" the chunks through the channel as they arrive
                while let Some(chunk_result) = llm_stream.next().await {
                    // debug!("chunk result: {:?}", chunk_result);
                    let chunk = match chunk_result {
                        Ok(chunk) => chunk,
                        Err(e) => {
                            tracing::error!("Stream chunk error: {}", e);
                            let _ = tx.send(Err(HttpError::NetworkError(e.to_string()))).await;
                            break;
                        }
                    };
                    if let Some(call) = chunk.tool_call {
                        debug!("Agent: {} call: {:?}", agent_id, call);
                        tool_calls.push(call);
                    } else {
                        trace!("Agent: {} chunk: {:?}", agent_id, chunk);

                        if chunk.is_final {
                            usage += chunk.usage.unwrap();
                            response_id = chunk.response_id;
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

                info!("Agent: {} Tool Calls: {}", agent_id, tool_calls.len());
                if tool_calls.is_empty() {
                    info!(
                        "Agent: {} Final Response stats - model: {:#?} response_id: {} usage: {:#?}",
                        agent_id, model, response_id, usage
                    );
                    let _ = tx
                        .send(Ok(CompletionChunkResponse::stop(
                            agent_id.clone(),
                            model,
                            response_id,
                            Some(usage),
                        )))
                        .await;
                    break;
                }
                let tool_futures: Vec<_> = tool_calls
                    .into_iter()
                    .map(|call| agent.execute_tool_call(call.clone()))
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
                            debug!("Agent: {}  Tool Call: {:?}", agent_id, tool_call);
                            debug!("Agent: {}     Output: {:?}", agent_id, tool_output);
                            nmessages.push(tool_call);
                            nmessages.push(tool_output);
                        }
                        Err(e) => {
                            warn!("Tool Call error: {}", e.to_string());
                        }
                    };
                }

                if !nmessages.is_empty() {
                    current_messages.extend(nmessages);
                }
            }
        });

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
    pub async fn complete(&self, messages: &[Message]) -> HttpResult<CompletionResponse> {
        let mut definitions: Vec<ToolDefinition> = self
            .tool_registry
            .get_tools()
            // .cloned()
            .iter()
            .map(|e| ToolDefinition::from_tool(e.as_ref()))
            .collect();
        debug!("Message: {:#?}", messages);
        trace!("too definitions: {:#?}", definitions);
        let mcp_definitions = self.mcp_registry.definitions.clone();
        mcp_definitions
            .iter()
            .for_each(|e| definitions.push(e.1.clone()));
        // debug!("All definitions: {:#?}", definitions);
        trace!("Mcp_definitions: {:#?}", mcp_definitions);

        let request = CompletionRequest {
            id: self.id.clone(),
            model: self.model.clone(),
            system: self.system_prompt.clone(),
            messages: messages.to_vec(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: false,
            reasoning_effort: self.reasoning_effort.clone(),
            enable_cache: self.enable_cache,
            definitions,
        };

        const MAX_ITERATIONS: usize = 10;
        let mut iteration = 0;

        let mut nrequest = request;
        let delay = Duration::from_millis(2000);
        let agent_id = self.id.clone();

        loop {
            iteration += 1;
            info!(
                "Agent: {} Iteration: {}/{}",
                agent_id, iteration, MAX_ITERATIONS
            );
            if iteration > 5 {
                sleep(delay).await;
            }

            if iteration > MAX_ITERATIONS {
                error!(
                    "Agent: {}, Max tool iterations exceeded: {}",
                    agent_id, iteration
                );
                return Err(HttpError::MaxIterationsExceeded);
            }

            trace!("CompletionRequest: {:#?}", nrequest);

            // Call the llm with the request
            let response = self.client.complete(nrequest.clone()).await?;

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

            if tool_calls.is_empty() {
                debug!(
                    "Agent: {} CompletionResponse: {:#?}",
                    agent_id,
                    response.text()
                );
                return Ok(response); // Done - return final answer
            }

            info!(
                "Agent: {} CompletionResponse: {:#?} tool calls: {}",
                agent_id,
                response.response_id,
                tool_calls.len()
            );
            trace!("CompletionResponse: {:#?}", response);

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
                    let timeout_duration = Duration::from_secs(60);
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
                })
                .collect();

            let results = futures::future::join_all(tool_futures).await;

            //Add thoughts to the messages first
            let mut nmessages: Vec<Message> = Vec::new();
            nmessages.extend(thoughts);
            for result in results {
                match result {
                    Ok((tool_call, tool_output)) => {
                        trace!("Tool Call: {:?}", tool_call);
                        debug!("     Output: {:?}", tool_output);
                        nmessages.push(tool_call);
                        nmessages.push(tool_output);
                    }
                    Err(e) => {
                        warn!("Tool Call error: {}", e.to_string());
                    }
                };
            }
            debug!("Agent: {} New messages: {:?}", agent_id, nmessages.len());

            if !nmessages.is_empty() {
                nrequest.messages.extend(nmessages);
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

        info!(
            "Agent: {} Executing tool: {:#?} args: {:?}",
            self.id, call.name, call.arguments
        );

        let output = match self.tool_registry.get_tool(&call.name) {
            Some(tool) => tool.execute(call.arguments.clone()).await?,
            None => {
                if self.mcp_registry.has_tool(&call.name) {
                    // info!("Executing MCP call_tool: {} args: {:?}", call.name, call.arguments);
                    match self
                        .mcp_registry
                        .call_tool(&call.name, call.arguments.clone())
                        .await
                    {
                        Ok(c) => c,
                        Err(e) => {
                            serde_json::json!({
                                "error": format!("{:?}", e)
                            })
                        }
                    }
                } else {
                    // return error message to LLM — let it recover
                    serde_json::json!({
                        "error": format!("Tool '{}' is not available", call.name)
                    })
                }
            }
        };

        let tool_output_message = Message::ToolOutput {
            call_id: call.id.clone(),
            output,
            name: call.name.clone(),
        };

        Ok((tool_call_message, tool_output_message))
    }
}
