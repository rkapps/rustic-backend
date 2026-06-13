use std::{sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use rustic_core::{HttpError, HttpResult};
use tokio::sync::{Semaphore, mpsc};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, trace, warn};

use crate::{
    Agent, CompletionChunkResponse, CompletionResponse, CompletionResponseTokenUsage, Message,
    agents::{
        domain::{AgentGoal, CompletionTurn, ExecutionMode, StageDecision},
        helper::{
            build_clean_json, build_decision_status, build_messages_from_turns, build_user_message,
            merge_responses,
        },
    },
    services::config::agent::CompletionStrategy,
};

/// Per-call timeout applied to each sub-agent execution in [`PipeLineAgent::execute_subs`].
const SUB_AGENT_TIMEOUT: u64 = 120;

/// Common interface for executable agent topologies.
///
/// Implemented by [`SingleAgent`] (direct delegation) and [`PipeLineAgent`] (orchestrated
/// multi-stage loop). Callers receive an `Arc<dyn Runnable>` from
/// [`AgentService::build_runnable`](crate::services::agent::AgentService::build_runnable)
/// and interact only through this trait.
#[async_trait]
pub trait Runnable: Send + Sync {
    /// Execute non-streaming and return the final [`CompletionResponse`].
    async fn execute(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
    ) -> HttpResult<CompletionResponse>;

    /// Execute and stream [`CompletionChunkResponse`] items through the returned channel.
    async fn execute_streaming(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
    ) -> HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>>;

    fn get_agent_id(&self) -> &String;
    fn get_agent(&self) -> &Agent;
    fn get_strategy(&self) -> &CompletionStrategy;
}

/// A type-tagged wrapper around an `Arc<dyn Runnable>` that carries topology information.
///
/// Used where the caller needs to know whether the underlying runnable is a single agent
/// or a pipeline without downcasting.
pub enum RunnableMode {
    SingleAgent(Arc<dyn Runnable>),
    PipelineAgent(Arc<dyn Runnable>),
}

impl RunnableMode {
    pub fn get_agent(&self) -> &Agent {
        match self {
            RunnableMode::SingleAgent(r) => r.get_agent(),
            RunnableMode::PipelineAgent(r) => r.get_agent(),
        }
    }
    pub fn get_strategy(&self) -> &CompletionStrategy {
        match self {
            RunnableMode::SingleAgent(r) => r.get_strategy(),
            RunnableMode::PipelineAgent(r) => r.get_strategy(),
        }
    }
}

/// A [`Runnable`] that delegates directly to a single [`Agent`].
///
/// `execute` rebuilds the message history from `turns` and appends the current prompt,
/// then calls [`Agent::complete`]. `execute_streaming` does the same via
/// [`Agent::complete_with_streaming`].
pub struct SingleAgent {
    pub agent: Agent,
    pub strategy: CompletionStrategy,
}

impl SingleAgent {
    pub fn new(agent: Agent, strategy: CompletionStrategy) -> Self {
        SingleAgent { agent, strategy }
    }
}

/// A [`Runnable`] that orchestrates a set of sub-agents through a multi-stage decision loop.
///
/// The inner `agent` acts as the orchestrator: it receives the accumulated `pipeline_turns`
/// and returns a [`StageDecision`] JSON on each iteration. Sub-agents are stored in `subs`
/// and resolved by ID at runtime. The loop runs up to 10 iterations; the first iteration
/// where `decision.stop == true` triggers synthesis and exits.
///
/// Token usage from the orchestrator and all sub-agent calls is accumulated in `usage`
/// and included in the final [`CompletionChunkResponse::stop`] chunk.
#[derive(Clone)]
pub struct PipeLineAgent {
    pub agent: Agent,
    pub strategy: CompletionStrategy,
    pub subs: Vec<Arc<dyn Runnable>>,
    pub usage: CompletionResponseTokenUsage,
}

#[async_trait]
impl Runnable for SingleAgent {
    async fn execute(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
    ) -> HttpResult<CompletionResponse> {
        info!(
            turns= ?turns.len(),
            strategy= ?self.get_strategy(),
            temperature= ?self.get_agent().temperature,
            max_tokens= ?self.get_agent().max_tokens,
            reasoning_effort= ?self.get_agent().reasoning_effort,

            "Agent {:?} Executing SingleAgent...", self.get_agent_id()
        );
        let (mut messages, last_response_id) = build_messages_from_turns(&turns);
        messages.push(build_user_message(prompt.to_string(), last_response_id));
        self.agent.complete(&messages).await
    }

    async fn execute_streaming(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
    ) -> HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>> {
        info!(
            turns= ?turns.len(),
            strategy= ?self.get_strategy(),
            temperature= ?self.get_agent().temperature,
            max_tokens= ?self.get_agent().max_tokens,
            reasoning_effort= ?self.get_agent().reasoning_effort,

            "Agent {:?} Executing SingleAgent Streaming...", self.get_agent_id()
        );

        let (mut messages, last_response_id) = build_messages_from_turns(&turns);
        messages.push(build_user_message(prompt.to_string(), last_response_id));
        self.agent.complete_with_streaming(&messages).await
    }

    fn get_agent_id(&self) -> &String {
        &self.agent.id
    }

    fn get_agent(&self) -> &Agent {
        &self.agent
    }

    fn get_strategy(&self) -> &CompletionStrategy {
        &self.strategy
    }
}

#[async_trait]
impl Runnable for PipeLineAgent {
    async fn execute(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
    ) -> HttpResult<CompletionResponse> {
        info!(
            turns= ?turns.len(),
            strategy= ?self.get_strategy(),
            temperature= ?self.get_agent().temperature,
            max_tokens= ?self.get_agent().max_tokens,
            reasoning_effort= ?self.get_agent().reasoning_effort,
            "Agent {:?} Executing PipelineAgent...", self.get_agent_id()
        );

        let (mut messages, last_response_id) = build_messages_from_turns(&turns);
        messages.push(build_user_message(prompt.to_string(), last_response_id));
        self.agent.complete(&Vec::new()).await
    }

    /// Run the pipeline and stream status + content chunks to the caller.
    ///
    /// Spawns a background task that drives the orchestration loop (up to 10 iterations):
    ///
    /// 1. Calls [`decide`](PipeLineAgent::decide) with the accumulated `pipeline_turns`.
    /// 2. Sends a status chunk describing the chosen agents (via [`build_decision_status`]).
    /// 3. If `decision.stop == true`: streams the synthesis agent's output directly to the
    ///    caller, appends a final [`CompletionChunkResponse::stop`] with total usage, and exits.
    /// 4. Otherwise: runs the nominated sub-agents via [`execute_subs`](PipeLineAgent::execute_subs),
    ///    merges their JSON responses, appends a [`CompletionTurn`] to `pipeline_turns`, and loops.
    async fn execute_streaming(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
    ) -> HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>> {
        info!(
            turns= ?turns.len(),
            strategy= ?self.get_strategy(),
            temperature= ?self.get_agent().temperature,
            max_tokens= ?self.get_agent().max_tokens,
            reasoning_effort= ?self.get_agent().reasoning_effort,
            "Agent {:?} Executing PipelineAgent Streaming...", self.get_agent_id()
        );

        let (tx, rx) = mpsc::channel::<Result<CompletionChunkResponse, HttpError>>(200);
        let agent_id = self.get_agent_id().clone();
        let self_clone = Arc::new(self.clone());
        let original_turns = turns.clone();
        let original_prompt = prompt.to_string();
        let mut last_response_id = None;

        tokio::spawn(async move {
            let mut iteration = 0;
            const MAX_ITERATIONS: usize = 10;
            let mut usage = CompletionResponseTokenUsage::default();
            let mut pipeline_turns = Vec::new();

            loop {
                iteration += 1;
                if iteration > MAX_ITERATIONS {
                    error!("Error: {}", HttpError::MaxIterationsExceeded);
                    let _ = tx.send(Err(HttpError::MaxIterationsExceeded)).await;
                    break;
                }

                let new_prompt = if iteration == 1 {
                    original_prompt.clone()
                } else {
                    "Based on the prior agent outputs above, decide the next agent or agents to run. Follow your sequencing rules exactly.".to_string()
                };

                let mut dturns = Vec::new();
                dturns.extend(original_turns.clone());
                dturns.extend(pipeline_turns.clone());

                let (response, decision) = match self_clone
                    .decide(&dturns, &new_prompt, last_response_id.clone())
                    .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx
                            .send(Err(HttpError::CompletionRequestError(e.to_string())))
                            .await;
                        break;
                    }
                };

                info!(
                    stop= %decision.stop,
                    execution= ?decision.execution,
                    agents= ?format_args!("{:#?}", decision.agents),
                    "Agent: {} Decision", agent_id
                );

                // set the last resonse id.
                last_response_id = if response.response_id.clone().is_empty() {
                    None
                } else {
                    Some(response.response_id.clone())
                };

                // add the usage
                let dusage = response.usage;

                let status = build_decision_status(&decision);
                let _ = tx
                    .send(Ok(CompletionChunkResponse::status(status.clone())))
                    .await;

                if decision.stop {
                    let start = std::time::Instant::now();

                    if decision.agents.len() > 1 {
                        let _ = tx
                            .send(Err(HttpError::Other(
                                "Stop decision must have exactly one agent".to_string(),
                            )))
                            .await;
                        break;
                    }

                    if matches!(decision.execution, ExecutionMode::Parallel) {
                        let _ = tx
                            .send(Err(HttpError::Other(
                                "Stop decision must be sequential not parallel".to_string(),
                            )))
                            .await;
                        break;
                    }
                    let subs = match self_clone.resolve_sub_agents(&decision.agents) {
                        Ok(c) => c,
                        Err(e) => {
                            let _ = tx
                                .send(Err(HttpError::Other(format!(
                                    "Resolving Synthesizer error: {}",
                                    e
                                ))))
                                .await;
                            break;
                        }
                    };

                    info!(
                        turns = format_args!("{:#?}", pipeline_turns),
                        "Agent: {} Synthesising...", agent_id
                    );
                    let synthesizer = subs[0].clone();
                    let mut stream = match synthesizer
                        .0
                        .execute_streaming(pipeline_turns, &synthesizer.1)
                        .await
                    {
                        Ok(c) => c,
                        Err(e) => {
                            let _ = tx
                                .send(Err(HttpError::Other(format!(
                                    "Resolving Synthesizer error: {}",
                                    e
                                ))))
                                .await;
                            break;
                        }
                    };

                    // pipe synthesizer stream to tx
                    let mut chunk_count = 0;
                    while let Some(chunk_result) = stream.next().await {
                        if chunk_count == 0 {
                            let _ = tx
                                .send(Ok(CompletionChunkResponse::status(format!(
                                    "  ✅ {:.1}s\n",
                                    start.elapsed().as_secs_f32()
                                ))))
                                .await;
                        }
                        chunk_count += 1;

                        let chunk = match chunk_result {
                            Ok(chunk) => chunk,
                            Err(e) => {
                                tracing::error!("Stream chunk error: {}", e);
                                let _ = tx.send(Err(HttpError::Other(e.to_string()))).await;
                                break;
                            }
                        };

                        trace!("Chunk: {:?}", chunk);

                        if chunk.is_final {
                            info!(
                                chunk = format_args!("{:#?}", chunk),
                                "Agent: {} Synthesising done.", agent_id
                            );

                            // let response_id = chunk.response_id.clone();
                            let mut final_chunk = chunk.clone();
                            final_chunk.is_final = false;
                            let _ = tx.send(Ok(final_chunk)).await;

                            usage += chunk.usage.unwrap();

                            let _ = tx
                                .send(Ok(CompletionChunkResponse::stop(
                                    agent_id.clone(),
                                    chunk.model,
                                    last_response_id.clone().unwrap_or_default(),
                                    Some(usage.clone()),
                                )))
                                .await;
                        } else {
                            let _ = tx.send(Ok(chunk)).await;
                        }
                    }

                    break;
                }

                let start = std::time::Instant::now();

                // execute the sub agents now.
                let (merged, sub_usage) = match self_clone.execute_subs(&decision).await {
                    Ok(c) => c,
                    Err(e) => {
                        let error = format!("Executing sub agent error: {}", e);
                        error!(error);
                        let _ = tx.send(Err(HttpError::Other(error.to_string()))).await;
                        break;
                    }
                };
                let elapsed = start.elapsed();
                let done = format!("  ✅ {:.1}s\n", elapsed.as_secs_f32());
                let _ = tx.send(Ok(CompletionChunkResponse::status(done))).await;

                info!(
                    merged= ?format_args!("{:#?}", merged),
                    "Agent: {} Decision", agent_id
                );

                // sum up the decide and sub usages.
                usage += sub_usage;
                usage += dusage;

                pipeline_turns.push(CompletionTurn {
                    response_content: merged,
                    response_id: last_response_id.clone(),
                    sequence: iteration as u32,
                    user_content: new_prompt,
                });
            }
        });

        Ok(ReceiverStream::new(rx))
    }

    fn get_agent_id(&self) -> &String {
        &self.agent.id
    }

    fn get_agent(&self) -> &Agent {
        &self.agent
    }

    fn get_strategy(&self) -> &CompletionStrategy {
        &self.strategy
    }
}

impl PipeLineAgent {
    pub fn new(agent: Agent, strategy: CompletionStrategy, subs: Vec<Arc<dyn Runnable>>) -> Self {
        Self {
            agent,
            strategy,
            subs,
            usage: CompletionResponseTokenUsage::default(),
        }
    }

    /// Parse a [`StageDecision`] from the orchestrator's [`CompletionResponse`].
    ///
    /// Strips JSON fences before deserialisation. Returns [`HttpError::Other`] if the
    /// response contains no text or cannot be parsed as a valid [`StageDecision`].
    pub fn build_decision(&self, response: &CompletionResponse) -> HttpResult<StageDecision> {
        let content = response.text();
        if let Some(val) = content {
            trace!("val: {}", val);
            let clean = &build_clean_json(val);

            match serde_json::from_str::<StageDecision>(clean) {
                Ok(decision) => Ok(decision),
                Err(e) => Err(HttpError::Other(format!(
                    "Failed to parse StageDecision: {}",
                    e
                ))),
            }
        } else {
            Err(HttpError::Other(
                "Failed to parse completion response".to_string(),
            ))
        }
    }

    /// Build the message list to send to the orchestrator on a decision turn.
    ///
    /// Replays `turns` as alternating User/Assistant pairs, then appends the new
    /// `prompt` as the next User message. `response_id` is threaded in for multi-turn
    /// context continuation; if `None`, falls back to the last turn's response ID.
    pub fn build_orchesrator_messages(
        &self,
        turns: &[CompletionTurn],
        prompt: &str,
        response_id: Option<String>,
    ) -> Vec<Message> {
        let agent = self.get_agent();
        info!(
            turns= ?turns.len(),
            prompt= ?prompt,
            "Agent: {}", agent.id
        );
        let (mut messages, last_response_id) = build_messages_from_turns(turns);
        let nresponse_id = response_id.or(last_response_id);

        messages.push(build_user_message(prompt.to_string(), nresponse_id));
        messages
    }

    /// Ask the orchestrator to decide the next stage and return the raw response alongside
    /// the parsed [`StageDecision`].
    pub async fn decide(
        &self,
        turns: &[CompletionTurn],
        prompt: &str,
        response_id: Option<String>,
    ) -> Result<(CompletionResponse, StageDecision)> {
        let agent = self.get_agent();
        let messages = self.build_orchesrator_messages(turns, prompt, response_id);
        debug!(
            messages= ?messages,
            "Agent: {}", agent.id
        );
        let response = agent.complete(&messages).await?;
        let decision = self.build_decision(&response)?;
        Ok((response, decision))
    }

    /// Run the sub-agents nominated by `decision` and return the merged JSON string
    /// plus the total token usage across all sub-calls.
    ///
    /// Sequential mode runs sub-agents one at a time in order. Parallel mode runs up to
    /// 5 concurrently (semaphore-bounded) each with a [`SUB_AGENT_TIMEOUT`]-second timeout;
    /// individual failures are logged and skipped rather than aborting the whole stage.
    pub async fn execute_subs(
        &self,
        decision: &StageDecision,
    ) -> Result<(String, CompletionResponseTokenUsage)> {
        let subs = self.resolve_sub_agents(&decision.agents)?;
        let mut responses: Vec<(String, CompletionResponse)> = Vec::new();

        match decision.execution {
            ExecutionMode::Sequential => {
                for sub in subs {
                    let response = sub.0.execute(Vec::new(), &sub.1).await?;
                    debug!(
                        response = format_args!("{:?}", response.text()),
                        "Agent: {}", response.id,
                    );
                    responses.push((sub.0.get_agent_id().to_string(), response));
                }
            }
            ExecutionMode::Parallel => {
                let semaphore = Arc::new(Semaphore::new(5)); // max 5 parallel
                let futures: Vec<_> = subs
                    .iter()
                    .map(|s| {
                        let sem = semaphore.clone();
                        let agent = s.0.clone();
                        let prompt = s.1.clone();
                        let timeout_duration = Duration::from_secs(SUB_AGENT_TIMEOUT);

                        async move {
                            let _permit = sem.acquire().await.unwrap();
                            tokio::time::timeout(
                                timeout_duration,
                                agent.execute(Vec::new(), &prompt),
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
                                response = format_args!("{:?}", response.text()),
                                "Agent: {}", response.id,
                            );
                            responses.push((response.id.clone(), response));
                        }
                        Err(e) => {
                            warn!("Agent call error: {}", e.to_string());
                        }
                    };
                }
            }
        }

        Ok(merge_responses(&responses))
    }

    // pub async fn execute_synthesizer(
    //     &self,
    //     decision: &StageDecision,
    //     pipeline_turns: &[CompletionTurn],
    //     tx: Sender<Result<CompletionChunkResponse, HttpError>>,
    // ) -> Result<HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>>> {
    //     // validate single agent for stop
    //     // Ok(())
    // }

    /// Resolve a list of [`AgentGoal`]s to `(Arc<dyn Runnable>, goal_string)` pairs.
    ///
    /// Returns [`HttpError::Other`] if any goal is missing or the agent ID is not
    /// present in `self.subs`.
    fn resolve_sub_agents(
        &self,
        agent_goals: &[AgentGoal],
    ) -> HttpResult<Vec<(Arc<dyn Runnable>, String)>> {
        agent_goals
            .iter()
            .map(|agent_goal| {
                let goal = agent_goal.goal.clone().ok_or_else(|| {
                    HttpError::Other(format!("Agent {} missing goal", agent_goal.id))
                })?;

                let sub = self
                    .subs
                    .iter()
                    .find(|s| s.get_agent().id == agent_goal.id)
                    .cloned()
                    .ok_or_else(|| {
                        HttpError::Other(format!("Sub agent not found: {}", agent_goal.id))
                    })?;

                Ok((sub, goal))
            })
            .collect()
    }
}
