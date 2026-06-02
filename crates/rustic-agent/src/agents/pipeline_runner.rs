//! Multi-agent pipeline execution: orchestrator-driven stage loop and sub-agent dispatch.
//!
//! The core types here are:
//! - [`AgentHandle`] — a polymorphic wrapper that lets a pipeline slot hold either a plain
//!   [`Agent`] or a nested [`PipeLineRunner`], enabling recursive pipeline composition.
//! - [`PipeLineRunner`] — drives the orchestrator → decide → run-sub-agents loop until the
//!   orchestrator sets `stop: true`, then returns or streams the final synthesised response.

use crate::{
    Agent, CompletionChunkResponse, CompletionResponse, CompletionResponseContent,
    CompletionResponseTokenUsage, Message,
    agents::{
        ExecutionMode, StageDecision,
        helper::{
            build_merged_sub_agent_message, build_stage_decision, build_sub_agent_messages,
            unwrap_agent_content,
        },
    },
    services::config::agent::{AgentConfig, AgentContext},
};
use rustic_core::{HttpError, HttpResult};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{Semaphore, mpsc};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, trace, warn};

/// A slot in the pipeline that can be occupied by a leaf [`Agent`] or a nested [`PipeLineRunner`].
///
/// This indirection lets pipelines be composed recursively: a sub-agent in one pipeline can
/// itself be a full pipeline, which the orchestrator treats as a single opaque agent.
pub enum AgentHandle {
    /// A single LLM-backed agent with its own tool registries.
    Single(Agent),
    /// A nested pipeline treated as a single agent by the parent orchestrator.
    Pipeline(Arc<PipeLineRunner>),
}

impl AgentHandle {
    /// Execute this handle as a sub-agent within a pipeline stage and return its response.
    ///
    /// Builds the input message slice according to the agent's configured [`AgentContext`]:
    /// - `Goal` — passes the original user messages unchanged.
    /// - `Last` — passes only the most recent assistant output as a new user message.
    /// - `All`  — merges all pipeline assistant outputs into a synthesis prompt.
    ///
    /// For `Pipeline` handles the `goal` override, if present, replaces the input entirely.
    pub async fn execute_sub(
        &self,
        agent_config: AgentConfig,
        agent_id: &str,
        goal: Option<String>,
        original_messages: &[Message],
        pipeline_messages: &[Message],
    ) -> HttpResult<CompletionResponse> {
        let context = agent_config
            .pipeline
            .as_ref()
            .and_then(|p| p.available_agents.iter().find(|a| a.id == agent_id))
            .map(|a| a.context.clone())
            .unwrap_or(AgentContext::Last);

        let input = match context {
            AgentContext::Goal => AgentHandle::build_goal_input(original_messages),
            AgentContext::Last => {
                AgentHandle::build_last_input(original_messages, pipeline_messages)
            }
            AgentContext::All => AgentHandle::build_all_input(original_messages, pipeline_messages),
        };

        match self {
            AgentHandle::Single(agent) => agent.complete(&input).await,
            AgentHandle::Pipeline(runner) => {
                let input = match goal {
                    Some(override_input) => vec![Message::User {
                        content: override_input.clone(),
                        response_id: None,
                    }],
                    None => vec![original_messages.last().unwrap().clone()],
                };
                Box::pin(runner.run(&input)).await
            }
        }
    }

    /// Execute this handle as a streaming sub-agent, always using the `All` context strategy.
    ///
    /// Only `Single` handles support streaming. Calling this on a `Pipeline` handle returns
    /// [`HttpError::CompletionRequestError`] because a pipeline runner cannot itself be an
    /// orchestrator that streams.
    pub async fn execute_sub_streaming(
        &self,
        agent_id: &str,
        original_messages: &[Message],
        pipeline_messages: &[Message],
    ) -> HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>> {
        let input = AgentHandle::build_all_input(original_messages, pipeline_messages);
        debug!("Agent: {} Input: {:#?}", agent_id, input);
        match self {
            AgentHandle::Single(agent) => agent.complete_with_streaming(&input).await,
            AgentHandle::Pipeline(_) => Err(HttpError::CompletionRequestError(
                "Pipeline cannot be an orchestrator".to_string(),
            )),
        }
    }

    /// Ask the orchestrator to produce a [`StageDecision`] JSON from the current message history.
    ///
    /// Only `Single` handles may act as orchestrators; a `Pipeline` handle returns an error
    /// because nested pipelines are opaque sub-agents, not decision-makers.
    pub async fn decide(
        &self,
        agent_id: &str,
        messages: &[Message],
    ) -> HttpResult<CompletionResponse> {
        info!(
            messages= %format_args!("{:#?}", messages),
            "Agent: {} Deciding...", agent_id
        );

        match self {
            AgentHandle::Single(agent) => agent.complete(messages).await,
            AgentHandle::Pipeline(_) => Err(HttpError::CompletionRequestError(
                "Pipeline cannot be an orchestrator".to_string(),
            )),
        }
    }

    /// Execute this handle directly (not as a sub-agent) and return its response.
    ///
    /// `Pipeline` handles are run statelessly: only the last message from `original_messages`
    /// is forwarded so the nested pipeline starts fresh rather than inheriting outer context.
    pub async fn execute(&self, original_messages: &[Message]) -> HttpResult<CompletionResponse> {

        info!("Executing agent...");
        match self {
            AgentHandle::Single(agent) => {
                let last_message;
                let messages: &[Message] = if agent.store {
                    original_messages
                } else {
                    last_message = vec![original_messages.last().unwrap().clone()];
                    &last_message
                };
                agent.complete(messages).await
            }
            AgentHandle::Pipeline(runner) => {
                // force pipeline runner to be stateless
                let last = original_messages.last().unwrap();
                let mut input = Vec::new();
                input.push(last.clone());
                Box::pin(runner.run(&input)).await
            }
        }
    }

    /// Execute this handle with streaming output.
    ///
    /// `Pipeline` handles delegate to [`PipeLineRunner::run_dynamic_streaming`], forwarding
    /// only the last message to keep the nested pipeline stateless.
    pub async fn execute_streaming(
        &self,
        original_messages: &[Message],
    ) -> HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>> {

        info!("Executing agent...");

        match self {
            AgentHandle::Single(agent) => {
                // let last_message;
                // let messages: &[Message] = if agent.store {
                //     original_messages
                // } else {
                //     last_message = vec![original_messages.last().unwrap().clone()];
                //     &last_message
                // };
                agent.complete_with_streaming(original_messages).await
            }
            AgentHandle::Pipeline(runner) => {
                Box::pin(runner.clone().run_dynamic_streaming(original_messages)).await
            }
        }
    }

    /// Return the original user messages unchanged.
    ///
    /// Used by sub-agents configured with [`AgentContext::Goal`] that need the raw user request
    /// regardless of what earlier pipeline stages produced.
    pub fn build_goal_input(original_messages: &[Message]) -> Vec<Message> {
        original_messages.to_vec()
    }

    /// Build an input containing only the most recent assistant output as a `User` message.
    ///
    /// Used by sub-agents configured with [`AgentContext::Last`] so each stage sees only the
    /// immediately preceding result rather than the full accumulated history. Falls back to
    /// `original_messages` if no assistant output exists yet.
    pub fn build_last_input(
        original_messages: &[Message],
        pipeline_messages: &[Message],
    ) -> Vec<Message> {
        let last_content = pipeline_messages
            .iter()
            .rev()
            .find_map(|m| match m {
                Message::Assistant { content, .. } => Some(unwrap_agent_content(content)),
                _ => None,
            })
            .unwrap_or_default();

        if !last_content.is_empty() {
            vec![Message::User {
                content: last_content,
                response_id: None,
            }]
        } else {
            original_messages.to_vec()
        }
    }

    /// Build an input that merges all pipeline assistant outputs into a synthesis prompt.
    ///
    /// Used by sub-agents configured with [`AgentContext::All`] (typically the final synthesiser).
    /// Produces a two-message slice: the original user request followed by a `User` message that
    /// concatenates all prior assistant outputs and asks the agent to synthesise them.
    pub fn build_all_input(
        original_messages: &[Message],
        pipeline_messages: &[Message],
    ) -> Vec<Message> {
        let merged = pipeline_messages
            .iter()
            .filter_map(|m| match m {
                Message::Assistant { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        vec![
            original_messages[0].clone(),
            Message::User {
                content: format!(
                    "Research data from specialist agents:\n\n{}\n\nSynthesise all the research above into a final response for the user.",
                    merged
                ),
                response_id: None,
            },
        ]
    }
}

/// Orchestrates a multi-stage pipeline by repeatedly asking an orchestrator agent to decide
/// which sub-agents to run next, executing them, and feeding their outputs back until the
/// orchestrator signals `stop: true`.
///
/// The `agent_handles` map is pre-built at construction time and may contain nested
/// `Pipeline` entries, so the entire structure forms a recursive agent tree.
pub struct PipeLineRunner {
    /// The decision-making agent that produces [`StageDecision`] JSON each iteration.
    pub orchestrator: AgentHandle,
    /// Configuration for this pipeline (id, available agents, context strategy, etc.).
    pub agent_config: AgentConfig,
    /// Pre-built handles keyed by agent id; supports recursive pipeline nesting.
    pub agent_handles: HashMap<String, AgentHandle>,
}

impl PipeLineRunner {
    pub fn new(
        orchestrator: AgentHandle,
        agent_config: AgentConfig,
        agent_handles: HashMap<String, AgentHandle>,
    ) -> PipeLineRunner {
        PipeLineRunner {
            orchestrator,
            agent_config,
            agent_handles,
        }
    }

    /// Entry point — delegates to [`run_dynamic`](Self::run_dynamic).
    pub async fn run(&self, messages: &[Message]) -> HttpResult<CompletionResponse> {
        self.run_dynamic(messages).await
    }

    /// Drive the orchestrator loop and return the final [`CompletionResponse`].
    ///
    /// Each iteration:
    /// 1. Appends a "decide" prompt (unless the last message is already a `User`).
    /// 2. Asks the orchestrator to emit a [`StageDecision`].
    /// 3. Runs the chosen sub-agents via [`run_sub_agents`](Self::run_sub_agents).
    /// 4. Appends their merged output to the conversation history.
    /// 5. Stops and returns when `decision.stop == true`.
    ///
    /// Returns [`HttpError::MaxIterationsExceeded`] if the orchestrator has not stopped after 10 loops.
    pub async fn run_dynamic(&self, messages: &[Message]) -> HttpResult<CompletionResponse> {
        let mut iteration = 0;

        let original_messages = messages;
        let mut all_messages = Vec::new();
        all_messages.extend(messages.to_vec());

        const MAX_ITERATIONS: usize = 10;

        info!(
            "Agent: {} - Pipeline_runner run_dynamic",
            self.agent_config.id
        );

        loop {
            iteration += 1;
            if iteration > MAX_ITERATIONS {
                error!("Error: {}", HttpError::MaxIterationsExceeded);
                return Err(HttpError::MaxIterationsExceeded);
            }

            let pipeline_messages = all_messages.clone();

            info!(
                iteration= %iteration,
                messages= ?all_messages.len(),
                "Agent: {}", self.agent_config.id
            );

            // only append if last message is not User
            if !matches!(all_messages.last(), Some(Message::User { .. })) {
                // build the decide message with the last response_id
                let last_response_id = all_messages.iter().rev().find_map(|m| match m {
                    Message::Assistant { response_id, .. } => response_id.clone(),
                    _ => None,
                });

                let decide_content = "Based on the prior agent outputs above, decide the next agent or agents to run. Follow your sequencing rules exactly.".to_string();

                let decide_message = Message::User {
                    content: decide_content,
                    response_id: last_response_id, // ← carry forward
                };

                all_messages.push(decide_message);
            }

            let response = self
                .orchestrator
                .decide(&self.agent_config.id, &all_messages)
                .await
                .map_err(|_| {
                    HttpError::CompletionRequestError("No stage decision returned".to_string())
                })?;
            let decision = build_stage_decision(response.clone())?;

            info!(
                // "decision: {:?} excecution: {:#?} agents: {:?} goal: {:?}",
                stop= %decision.stop,
                execution= ?decision.execution,
                agents= ?decision.agents,
                goal= ?decision.goal,
                "Agent: {} Decision", self.agent_config.id
            );

            let merged = self
                .run_sub_agents(
                    &decision,
                    original_messages,
                    &pipeline_messages,
                    &all_messages,
                )
                .await?;

            all_messages.push(Message::Assistant {
                content: merged.clone(),
                response_id: Some(response.response_id),
            });

            debug!("sub agent merged messages: {:#?}", merged);

            // if the decision is stop then return the response.
            if decision.stop {
                let final_content = unwrap_agent_content(&merged);
                let rcontents = vec![CompletionResponseContent::Text(final_content)];
                // rcontents.push();
                let rresponse = CompletionResponse {
                    id: response.id,
                    model: response.model,
                    response_id: String::new(),
                    contents: rcontents,
                    usage: response.usage,
                };
                return Ok(rresponse);
            }
        }
    }

    /// Drive the orchestrator loop and stream output chunks to the caller.
    ///
    /// Behaviour mirrors [`run_dynamic`](Self::run_dynamic) but the final synthesiser agent
    /// is executed with streaming enabled; intermediate status lines (e.g. "⚡ Running: …") and
    /// per-stage elapsed-time markers are injected as content chunks so the client gets
    /// real-time feedback while waiting for the pipeline to complete.
    ///
    /// Spawns a background task; returns immediately with a [`ReceiverStream`] (capacity 200).
    pub async fn run_dynamic_streaming(
        self: Arc<Self>,
        messages: &[Message],
    ) -> HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>> {
        let (tx, rx) = mpsc::channel::<Result<CompletionChunkResponse, HttpError>>(200);

        let original_messages = messages;
        let mut all_messages = Vec::new();
        all_messages.extend(messages.to_vec());
        let runner = self.clone();

        let mut spawn_all_messages = all_messages.to_owned();
        let spawn_original_messages = original_messages.to_owned();
        // debug!("User Prompt {:#?}", messages);
        info!(
            user_prompt=%format_args!("{:#?}", messages),
            "Agent: {} - Pipeline_runner run_dynamic", self.agent_config.id
        );

        tokio::spawn(async move {
            let mut iteration = 0;
            const MAX_ITERATIONS: usize = 10;
            let mut usage = CompletionResponseTokenUsage::default();

            info!(
                "Agent: {} - Pipeline_runner run_dynamic",
                self.agent_config.id
            );

            loop {
                iteration += 1;
                if iteration > MAX_ITERATIONS {
                    error!("Error: {}", HttpError::MaxIterationsExceeded);
                    let _ = tx.send(Err(HttpError::MaxIterationsExceeded)).await;
                    break;
                }

                let pipeline_messages = spawn_all_messages.clone();
                let loop_start = std::time::Instant::now();

                info!(
                    iteration= %iteration,
                    messages= ?spawn_all_messages.len(),
                    "Agent: {}", self.agent_config.id
                );

                // only append if last message is not User
                if !matches!(spawn_all_messages.last(), Some(Message::User { .. })) {
                    // build the decide message with the last response_id
                    let last_response_id = spawn_all_messages.iter().rev().find_map(|m| match m {
                        Message::Assistant { response_id, .. } => response_id.clone(),
                        _ => None,
                    });

                    let decide_content = "Based on the prior agent outputs above, decide the next agent or agents to run. Follow your sequencing rules exactly.".to_string();

                    let decide_message = Message::User {
                        content: decide_content,
                        response_id: last_response_id, // ← carry forward
                    };

                    spawn_all_messages.push(decide_message);
                }

                let response = match runner
                    .orchestrator
                    .decide(&self.agent_config.id, &spawn_all_messages)
                    .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx
                            .send(Err(HttpError::CompletionRequestError(format!(
                                "No stage decision returned error: {}",
                                e
                            ))))
                            .await;
                        break;
                    }
                };

                let decision = match build_stage_decision(response.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        error!(
                            "Stage decision build response: {:#?} error: {:?} ",
                            response, e
                        );
                        let _ = tx
                            .send(Err(HttpError::CompletionRequestError(format!(
                                "Stage decision build error: {}",
                                e
                            ))))
                            .await;
                        break;
                    }
                };

                // after decision is made
                let status = match decision.stop {
                    true => "🧠 Synthesising...".to_string(),
                    false => format!(
                        "⚡ Running: {} ({})",
                        decision.agents.join(", "),
                        match decision.execution {
                            ExecutionMode::Parallel => "parallel",
                            ExecutionMode::Sequential => "sequential",
                        }
                    ),
                };

                info!(
                    // "decision: {:?} excecution: {:#?} agents: {:?} goal: {:?}",
                    stop= %decision.stop,
                    execution= ?decision.execution,
                    agents= ?decision.agents,
                    goal= ?decision.goal,
                    "Agent: {} Decision", self.agent_config.id
                );

                let _ = tx
                    .send(Ok(CompletionChunkResponse::content(
                        String::new(),
                        status.clone(),
                        String::new(),
                    )))
                    .await;

                if decision.stop {
                    if let Some(handle) = runner.agent_handles.get(&decision.agents[0]) {
                        let agent_id = &decision.agents[0];
                        let start = std::time::Instant::now();

                        let input = match handle
                            .execute_sub_streaming(
                                &decision.agents[0],
                                &spawn_original_messages,
                                &pipeline_messages,
                            )
                            .await
                        {
                            Ok(c) => c,
                            Err(_) => todo!(),
                        };

                        // pipe synthesizer stream to tx
                        let mut stream = input;
                        let mut chunk_count = 0;
                        while let Some(chunk_result) = stream.next().await {
                            if chunk_count == 0 {
                                let _ = tx
                                    .send(Ok(CompletionChunkResponse::content(
                                        String::new(),
                                        format!("  ✅ {:.1}s\n", start.elapsed().as_secs_f32()),
                                        String::new(),
                                    )))
                                    .await;
                            }
                            chunk_count += 1;

                            let chunk = match chunk_result {
                                Ok(chunk) => chunk,
                                Err(e) => {
                                    tracing::error!("Stream chunk error: {}", e);
                                    let _ =
                                        tx.send(Err(HttpError::NetworkError(e.to_string()))).await;
                                    break;
                                }
                            };

                            trace!("Chunk: {:?}", chunk);

                            if chunk.is_final {
                                info!(
                                    chunk = format_args!("{:#?}", chunk),
                                    "Agent: {} Synthesising done.", agent_id
                                );

                                let response_id = chunk.response_id.clone();
                                let mut final_chunk = chunk.clone();
                                final_chunk.is_final = false;
                                let _ = tx.send(Ok(final_chunk)).await;

                                usage += chunk.usage.unwrap();

                                let _ = tx
                                    .send(Ok(CompletionChunkResponse::stop(
                                        agent_id.clone(),
                                        chunk.model,
                                        response_id,
                                        Some(usage.clone()),
                                    )))
                                    .await;
                            } else {
                                let _ = tx.send(Ok(chunk)).await;
                            }
                        }
                    }
                    break;
                } else {
                    let start = std::time::Instant::now();

                    //run the sub agents and merge
                    let merged = match runner
                        .run_sub_agents(
                            &decision,
                            &spawn_original_messages,
                            &pipeline_messages,
                            &spawn_all_messages,
                        )
                        .await
                    {
                        Ok(c) => c,
                        Err(_) => {
                            let _ = tx
                                .send(Err(HttpError::CompletionRequestError(
                                    "Sstage decision build error".to_string(),
                                )))
                                .await;
                            break;
                        }
                    };
                    let elapsed = start.elapsed();
                    let done = format!("  ✅ {:.1}s\n", elapsed.as_secs_f32());
                    let _ = tx
                        .send(Ok(CompletionChunkResponse::content(
                            String::new(),
                            done,
                            String::new(),
                        )))
                        .await;

                    debug!(
                        merged= ?merged,
                        "Agent: {}", self.agent_config.id
                    );

                    spawn_all_messages.push(Message::Assistant {
                        content: merged.clone(),
                        response_id: Some(response.response_id),
                    });
                }

                let elapsed = loop_start.elapsed();
                let done = format!("  ✅ {:.1}s\n", elapsed.as_secs_f32());
                info!(
                    "Agent: {} Loop: {}-{}",
                    self.agent_config.id, iteration, done,
                );
            }
        });

        Ok(ReceiverStream::new(rx))
    }

    /// Execute the sub-agents named in `decision` and return their merged output string.
    ///
    /// - `Sequential`: agents run in order; each sees the outputs of previous agents via
    ///   `pipeline_messages` growing between calls.
    /// - `Parallel`: agents run concurrently, bounded to 5 in-flight at once with a 120-second
    ///   per-agent timeout. Errors are logged as warnings and the failed agent is skipped.
    ///
    /// The individual responses are normalised via [`build_sub_agent_messages`] and then joined
    /// by [`build_merged_sub_agent_message`] before being returned.
    pub async fn run_sub_agents(
        &self,
        decision: &StageDecision,
        original_messages: &[Message],
        pipeline_messages: &[Message],
        all_messages: &[Message],
    ) -> HttpResult<String> {
        // Collect sub agent assistant messages.
        let mut sub_agent_messages = Vec::new();

        match decision.execution {
            ExecutionMode::Sequential => {
                for sub_agent in decision.agents.clone() {
                    if let Some(agent_handle) = self.agent_handles.get(&sub_agent) {
                        info!(
                            "Agent: {} Executing sub agent: {}",
                            self.agent_config.id, sub_agent
                        );

                        let response = agent_handle
                            .execute_sub(
                                self.agent_config.clone(),
                                &sub_agent,
                                decision.goal.clone(),
                                original_messages,
                                pipeline_messages,
                            )
                            .await?;

                        debug!(
                            response= ?response.text(),
                            "Agent: {}", self.agent_config.id
                        );

                        build_sub_agent_messages(&mut sub_agent_messages, &response);
                    };
                }
            }
            ExecutionMode::Parallel => {
                let semaphore = Arc::new(Semaphore::new(5)); // max 3 parallel

                let futures: Vec<_> = decision
                    .agents
                    .iter()
                    .filter_map(|id| self.agent_handles.get(id).map(|h| (id.clone(), h)))
                    .map(|(id, handle)| {
                        let sem = semaphore.clone();
                        let pipeline_msgs = pipeline_messages;
                        let all_msgs = all_messages.to_vec();
                        let timeout_duration = Duration::from_secs(120);
                        let agent_config = self.agent_config.clone();

                        info!(
                            "Agent: {} Executing sub agent: {}",
                            self.agent_config.id, id
                        );
                        async move {
                            let _permit = sem.acquire().await.unwrap();
                            tokio::time::timeout(
                                timeout_duration,
                                handle.execute_sub(
                                    agent_config,
                                    &id,
                                    decision.goal.clone(),
                                    &all_msgs,
                                    pipeline_msgs,
                                ),
                            )
                            .await
                            .map_err(|_| HttpError::Timeout)?
                        }
                    })
                    .collect();

                let results = futures::future::join_all(futures).await;
                for result in results {
                    match result {
                        Ok(response) => {
                            debug!(
                                response= ?response.text(),
                                "Agent: {}", self.agent_config.id
                            );
                            build_sub_agent_messages(&mut sub_agent_messages, &response);
                        }
                        Err(e) => {
                            warn!("Agent call error: {}", e.to_string());
                        }
                    };
                }
            }
        }

        let merged = build_merged_sub_agent_message(&mut sub_agent_messages);

        Ok(merged)
    }
}
