use std::{sync::Arc, time::Duration};

use rustic_core::{HttpError, HttpResult};
use tokio::{
    sync::{Semaphore, mpsc},
    time::sleep,
};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, trace};

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

        debug!(
            target: "agent-tool",
            defintions= ?definitions,
            "Agent: {} - Tool definitions", self.id
        );

        let mcp_definitions = self.mcp_registry.definitions.clone();
        debug!(
            target: "agent-tool",
            defintions= ?mcp_definitions,
            "Agent: {} - Mcp_definitions", self.id
        );

        mcp_definitions
            .iter()
            .for_each(|e| definitions.push(e.1.clone()));

        // Clone Arcs and Data for the background task
        let agent = self.clone();
        let mut current_messages = messages.to_owned();
        let system_prompt = self.system_prompt.clone();
        let new_definitions = definitions.clone();
        let agent_id = self.id.clone();

        info!(  model= %agent.model,
                temperature= %agent.temperature,
                reasoning= ?&agent.reasoning_effort,
                maxtokens= %agent.max_tokens,
                store= %agent.store,
                "Agent: {} - Completion Start", agent_id
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

            debug!(target: "agent-messages",
                "Agent: {} - Current messages: {:#?}", agent_id, current_messages
            );

            loop {
                iteration += 1;
                if iteration > MAX_ITERATIONS {
                    break;
                }
                info!(iteration= %iteration,
                    messages= ?current_messages.len(),
                    last_response_id = ?last_assistant,
                    "Agent: {} - ", agent_id
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
                    store: agent.store,
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
                // Gemini sends partial thought tokens as random-looking characters; accumulate
                // the full thought before appending it as a Thought message so the model receives
                // a coherent block on the next turn.
                let mut thought_content = String::new();

                // 2. "Pump" the chunks through the channel as they arrive
                while let Some(chunk_result) = llm_stream.next().await {
                    let chunk = match chunk_result {
                        Ok(chunk) => chunk,
                        Err(e) => {
                            tracing::error!("Stream chunk error: {}", e);
                            let _ = tx.send(Err(HttpError::NetworkError(e.to_string()))).await;
                            break;
                        }
                    };
                    if let Some(call) = chunk.tool_call {
                        debug!(agent= %agent_id, tool_call= ?call, "Tool Call");
                        tool_calls.push(call);
                    } else {
                        debug!(
                            chunk= ?chunk,
                            "Agent: {}", agent_id
                        );

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

                info!(
                    tool_calls= %tool_calls.len(),
                    response_id= %response_id,
                    "Agent: {} - ", agent_id
                );

                if tool_calls.is_empty() {
                    info!(
                        model=%model,
                        response_id= ?response_id,
                        usage= %format_args!("{:#?}", usage),
                        "Agent: {} - Response Stats final", agent_id
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
                            // debug!(target: "agent-tool", tool_call= ?tool_call, "Tool Call");
                            debug!(target: "agent-tool",
                                tool_call= ?tool_call,
                                "Agent: {} - ", agent_id
                            );
                            debug!(target: "agent-tool",
                                tool_output= ?tool_output,
                                "Agent: {} - ", agent_id
                            );
                            nmessages.push(tool_call);
                            nmessages.push(tool_output);
                        }
                        Err(e) => {
                            error!(target: "agent-tool",
                                error= ?e,
                                "Agent: {} - Tool Call Error", agent_id
                            );
                        }
                    };
                }

                if !nmessages.is_empty() {
                    info!(
                        nmessages= ?nmessages.len(),
                        "Agent: {} - ", agent_id
                    );

                    if agent.store && !response_id.is_empty() {
                        // keep only the original user message, clear tool calls/results from previous iteration
                        current_messages.retain(|m| matches!(m, Message::User { .. }));
                        // inject last response_id into the user message
                        if let Some(Message::User { content, .. }) = current_messages.first_mut() {
                            let last_response_id = Some(response_id);
                            *current_messages.first_mut().unwrap() = Message::User {
                                content: content.clone(),
                                response_id: last_response_id, // ← inject response_id
                            };
                        }
                    }
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
        let agent_id = &self.id;
        let agent = self.clone();

        let mut definitions: Vec<ToolDefinition> = self
            .tool_registry
            .get_tools()
            // .cloned()
            .iter()
            .map(|e| ToolDefinition::from_tool(e.as_ref()))
            .collect();

        debug!(target: "agent-messages",
          "Agent: {} - Current messages: {:#?}", agent_id, messages
        );
        debug!(target: "agent-tool",
            defintions= ?definitions,
            "Agent: {} - Tool definitions", self.id
        );

        let mcp_definitions = self.mcp_registry.definitions.clone();

        debug!(target: "agent-tool",
            defintions= ?mcp_definitions,
            "Agent: {} - Mcp_definitions", self.id
        );

        info!(  model= %agent.model,
            temperature= %agent.temperature,
            reasoning= ?&agent.reasoning_effort,
            maxtokens= %agent.max_tokens,
            "Agent: {} - Completion Start", agent_id
        );

        mcp_definitions
            .iter()
            .for_each(|e| definitions.push(e.1.clone()));

        let request = CompletionRequest {
            id: self.id.clone(),
            model: self.model.clone(),
            system: self.system_prompt.clone(),
            messages: messages.to_vec(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: false,
            store: self.store,
            reasoning_effort: self.reasoning_effort.clone(),
            enable_cache: self.enable_cache,
            definitions,
        };

        const MAX_ITERATIONS: usize = 10;
        let mut iteration = 0;

        let mut nrequest = request;
        let delay = Duration::from_millis(2000);
        let mut last_response_id = String::default();
        loop {
            iteration += 1;
            info!(
                "Agent: {} Iteration: {}/{} messages: {:?}",
                agent_id,
                iteration,
                MAX_ITERATIONS,
                nrequest.messages.len()
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

            // At the start of each iteration (not after tool calls)
            if agent.store && !last_response_id.is_empty() {
                if let Some(Message::User {
                    content: _,
                    response_id,
                }) = nrequest
                    .messages
                    .iter_mut()
                    .find(|m| matches!(m, Message::User { .. }))
                {
                    *response_id = Some(last_response_id.clone());
                }
            }

            // Call the llm with the request
            let response = self.client.complete(nrequest.clone()).await?;
            last_response_id = response.response_id.clone();

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
                info!(
                    response= %format_args!("{:#?}", response.text() ),
                    usage= %format_args!("{:#?}", response.usage),
                    "Agent: {} - Response Stats final", agent_id
                );

                return Ok(response); // Done - return final answer
            }

            info!(
                tool_calls= ?tool_calls.len(),
                response_id= ?response.response_id,
                "Agent: {} - Response Stats final", agent_id
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
                    let timeout_duration = Duration::from_secs(120);
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
                        info!(target: "agent-tool", tool_call= ?tool_call, "Agent: {} - ", agent_id);
                        info!(target: "agent-tool", tool_output= ?tool_output, "Agent: {} - ", agent_id );
                        nmessages.push(tool_call);
                        nmessages.push(tool_output);
                    }
                    Err(e) => {
                        error!(target: "agent-tool", agent= %agent_id, error= ?e, "Tool Call Error");
                    }
                };
            }
            info!(
                "Agent: {} store: {} last Response Id: {:?} New messages: {:?}",
                agent_id,
                nrequest.store,
                last_response_id,
                nmessages.len()
            );

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
        let agent_id = &self.id;
        let tool_call_message = Message::ToolCall {
            call_id: call.id.clone(),
            arguments: call.arguments.to_string(),
            name: call.name.clone(),
        };

        let output = match self.tool_registry.get_tool(&call.name) {
            Some(tool) => {
                info!(target: "agent-tool",
                    name= ?call.name,
                    arguments= ?call.arguments,
                    "Agent: {} - Executing tool...", agent_id
                );

                tool.execute(call.arguments.clone()).await?
            }
            None => {
                if self.mcp_registry.has_tool(&call.name) {
                    info!(target: "agent-tool",
                        name= ?call.name,
                        arguments= ?call.arguments,
                        "Agent: {} - Executing Mcp tool...", agent_id
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
                                error= ?e,
                                arguments= ?call.arguments,
                                "Agent: {} - Executing McpTool error...", agent_id
                            );

                            serde_json::json!({
                                "error": format!("{:?}", e)
                            })
                        }
                    }
                } else {
                    error!(target: "agent-tool",
                        name= %call.name,
                        "Agent: {} - Tool not found...", agent_id
                    );

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
