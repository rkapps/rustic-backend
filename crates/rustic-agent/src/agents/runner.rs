use std::{fmt::Debug, sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use rustic_core::{HttpError, HttpResult};
use tokio::sync::{
    Semaphore,
    mpsc::{self, Sender},
};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{Instrument, debug, error, info, trace, warn};

use crate::{
    Agent, Message, TokenUsage,
    agents::{
        domain::{
            AgentChunkResponse, AgentGoal, AgentResponse, CompletionTurn, ExecutionMode,
            StageDecision, StageResponse, TurnChunkResponse, TurnResponse,
        },
        helper::{
            build_clean_json, build_decision_status, build_messages_from_turns, merge_responses,
        },
    },
    services::config::agent::PipelineStage,
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
pub trait Runnable: Send + Sync + Debug {
    /// Execute non-streaming and return the final [`CompletionResponse`].
    async fn execute(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
        store: bool,
    ) -> HttpResult<TurnResponse>;

    /// Execute and stream [`CompletionChunkResponse`] items through the returned channel.
    async fn execute_streaming(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
        store: bool,
    ) -> HttpResult<ReceiverStream<HttpResult<TurnChunkResponse>>>;

    fn get_agent_id(&self) -> &String;
    fn get_agent(&self) -> &Agent;
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
}

/// A [`Runnable`] that delegates directly to a single [`Agent`].
///
/// `execute` rebuilds the message history from `turns` and appends the current prompt,
/// then calls [`Agent::complete`]. `execute_streaming` does the same via
/// [`Agent::complete_with_streaming`].
#[derive(Clone, Debug)]
pub struct SingleAgent {
    pub agent: Agent,
}

impl SingleAgent {
    pub fn new(agent: Agent) -> Self {
        SingleAgent { agent }
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
#[derive(Clone, Debug)]
pub struct PipeLineAgent {
    pub agent: Agent,
    pub pipeline_type: String,
    pub stages: Vec<PipelineStage>,
    pub subs: Vec<Arc<dyn Runnable>>,
    pub usage: TokenUsage,
}

#[async_trait]
impl Runnable for SingleAgent {
    #[tracing::instrument(
        skip(self, turns),
        fields(
            otel.name = %format!("execute agent: {}", self.agent.id),
            _agent_id = %self.agent.id,
            _max_tokens= ?self.get_agent().max_tokens,
            _prompt=%prompt,
            _reasoning_effort= ?self.get_agent().reasoning_effort,
            _temperature= ?self.get_agent().temperature,
            _turns = turns.len()
        )
    )]
    async fn execute(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
        store: bool,
    ) -> HttpResult<TurnResponse> {
        let (mut messages, last_response_id) = build_messages_from_turns(&turns);
        messages.push(Message::user(prompt.to_string()));

        // mutate agent with store
        let mut agent = self.get_agent().clone();
        agent.store = store;

        let agent_response = agent.complete(&messages, last_response_id).await?;
        Ok(TurnResponse::for_single_agent(
            prompt.to_string(),
            agent_response,
        ))
    }

    #[tracing::instrument(
        skip(self, turns),
        fields(
            _agent_id = %self.agent.id,
            _max_tokens= ?self.get_agent().max_tokens,
            _prompt=%prompt,
            _reasoning_effort= ?self.get_agent().reasoning_effort,
            _temperature= ?self.get_agent().temperature,
            _turns = turns.len()
        )
    )]
    async fn execute_streaming(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
        store: bool,
    ) -> HttpResult<ReceiverStream<HttpResult<TurnChunkResponse>>> {
        let (mut messages, last_response_id) = build_messages_from_turns(&turns);
        messages.push(Message::user(prompt.to_string()));

        // mutate agent with store
        let mut agent = self.get_agent().clone();
        agent.store = store;
        let mut agent_stream = agent
            .complete_with_streaming(&messages, last_response_id)
            .await?;

        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let prompt_clone = prompt.to_string().clone();

        tokio::spawn(async move {
            while let Some(chunk) = agent_stream.next().await {
                match chunk {
                    Ok(AgentChunkResponse::Content { content, .. }) => {
                        let _ = tx
                            .send(Ok(TurnChunkResponse::Content {
                                agent_id: agent.id.clone(),
                                content,
                            }))
                            .await;
                    }
                    Ok(AgentChunkResponse::Thought { thought, .. }) => {
                        let _ = tx
                            .send(Ok(TurnChunkResponse::Thought {
                                agent_id: agent.id.clone(),
                                thought,
                            }))
                            .await;
                    }
                    // Ok(AgentChunkResponse::Status { status, .. }) => {
                    //     let _ = tx.send(Ok(TurnChunkResponse::Status { status })).await;
                    // }
                    Ok(AgentChunkResponse::Final { response }) => {
                        let turn_response =
                            TurnResponse::for_single_agent(prompt_clone.clone(), response);
                        let _ = tx
                            .send(Ok(TurnChunkResponse::Final {
                                response: turn_response,
                            }))
                            .await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                }
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
}

#[async_trait]
impl Runnable for PipeLineAgent {
    #[tracing::instrument(
        skip(self, turns, prompt),
        fields(
            otel.name = %format!("execute pipeline_agent: {}", self.agent.id),
            _agent_id = %self.agent.id,
            _max_tokens= ?self.get_agent().max_tokens,
            _pipeline_type = %self.pipeline_type,
            _prompt=%prompt,
            _reasoning_effort= ?self.get_agent().reasoning_effort,
            _temperature= ?self.get_agent().temperature,
            _turns = turns.len()
        )
    )]
    async fn execute(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
        store: bool,
    ) -> HttpResult<TurnResponse> {
        let stream = if self.pipeline_type == "deterministic" {
            self.execute_streaming_deterministic(turns, prompt, store)
                .await?
        } else {
            self.execute_streaming_dynamic(turns, prompt, store).await?
        };

        tokio::pin!(stream);

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(TurnChunkResponse::Final { response }) => {
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
    ///
    #[tracing::instrument(
        skip(self, turns),
        fields(
            otel.name = %format!("execute_streaming pipeline_agent: {}", self.agent.id),
            _agent_id = %self.agent.id,
            _max_tokens= ?self.get_agent().max_tokens,
            _pipeline_type = %self.pipeline_type,
            _prompt=%prompt,
            _reasoning_effort= ?self.get_agent().reasoning_effort,
            _temperature= ?self.get_agent().temperature,
            _turns = turns.len()
        )
    )]
    async fn execute_streaming(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
        store: bool,
    ) -> HttpResult<ReceiverStream<HttpResult<TurnChunkResponse>>> {
        info!(
            _agent_id = %self.agent.id,
            _max_tokens=?self.get_agent().max_tokens,
            _pipeline_type = %self.pipeline_type,
            _prompt=%prompt,
            _reasoning_effort= ?self.get_agent().reasoning_effort,
            _store=%store,
            _temperature= ?self.get_agent().temperature,
            _turns=?turns
        );

        if self.pipeline_type == "deterministic" {
            self.execute_streaming_deterministic(turns, prompt, store)
                .await
        } else {
            self.execute_streaming_dynamic(turns, prompt, store).await
        }
    }

    fn get_agent_id(&self) -> &String {
        &self.agent.id
    }

    fn get_agent(&self) -> &Agent {
        &self.agent
    }
}

impl PipeLineAgent {
    pub fn new(
        agent: Agent,
        pipeline_type: String,
        stages: Vec<PipelineStage>,
        subs: Vec<Arc<dyn Runnable>>,
    ) -> Self {
        Self {
            agent,
            pipeline_type,
            stages,
            subs,
            usage: TokenUsage::default(),
        }
    }

    /// Parse a [`StageDecision`] from the orchestrator's [`CompletionResponse`].
    ///
    /// Strips JSON fences before deserialisation. Returns [`HttpError::Other`] if the
    /// response contains no text or cannot be parsed as a valid [`StageDecision`].
    pub fn build_decision(&self, response: &AgentResponse) -> HttpResult<StageDecision> {
        let content = response.content.clone();
        if !content.is_empty() {
            // trace!("val: {}", val);
            let clean = &build_clean_json(&content);

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
        // response_id: Option<String>,
    ) -> Vec<Message> {
        debug!(
            turns= ?turns.len(),
            "Prompt: {}", prompt
        );
        let (mut messages, _) = build_messages_from_turns(turns);
        messages.push(Message::user(prompt.to_string()));
        messages
    }

    /// Ask the orchestrator to decide the next stage and return the raw response alongside
    /// the parsed [`StageDecision`].
    #[tracing::instrument(
        skip(self, turns),
        fields(
            _agent_id = %self.get_agent().id,
            _prompt = ?prompt
        )
    )]
    pub async fn decide(
        &self,
        turns: &[CompletionTurn],
        prompt: &str,
        response_id: Option<String>,
        store: bool,
    ) -> Result<(AgentResponse, StageDecision)> {
        let mut agent = self.get_agent().clone();
        agent.store = store;
        let messages = self.build_orchesrator_messages(turns, prompt);
        debug!("Message: {:?}", messages);
        let response = agent
            .complete(&messages, response_id)
            .instrument(tracing::Span::current())
            .await?;
        let decision = self.build_decision(&response)?;
        Ok((response, decision))
    }

    #[tracing::instrument(skip(self, turns))]
    async fn execute_streaming_deterministic(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
        store: bool,
    ) -> HttpResult<ReceiverStream<HttpResult<TurnChunkResponse>>> {
        let (tx, rx) = mpsc::channel::<Result<TurnChunkResponse, HttpError>>(200);
        let self_clone = Arc::new(self.clone());
        let original_prompt = prompt.to_string();

        info!("Agent: {:?}", self.get_agent_id());

        tokio::spawn(
            async move {
                let (_, last_response_id) = build_messages_from_turns(&turns);

                let mut turn_response = TurnResponse::for_deterministic(
                    &original_prompt.clone(),
                    self_clone.get_agent_id(),
                );

                let mut new_content = original_prompt.clone();
                let length = self_clone.stages.len();
                for (index, stage) in self_clone.stages.iter().enumerate() {
                    let execution = if stage.parallel {
                        ExecutionMode::Parallel
                    } else {
                        ExecutionMode::Sequential
                    };

                    let goal_text = if index == 0 {
                        original_prompt.clone()
                    } else {
                        let gtext = format!(
                            "{}\n\nFiscal Context from previous stage:\n{}",
                            original_prompt, new_content
                        );
                        gtext
                    };

                    let sub_agents = stage
                        .sub_agents
                        .iter()
                        .map(|s| AgentGoal {
                            id: s.id.clone(),
                            goal: Some(goal_text.clone()),
                        })
                        .collect();

                    let new_decision = StageDecision {
                        agents: sub_agents,
                        execution,
                        stop: false,
                        reasoning: None,
                    };

                    info!(
                        _agents= ?format_args!("{:#?}", new_decision.agents),
                        _decision = ?new_decision.execution,
                        "Agent: {:?}", self_clone.get_agent_id()
                    );

                    let start = std::time::Instant::now();
                    let status = build_decision_status(&new_decision);
                    let _ = tx
                        .send(Ok(TurnChunkResponse::status(
                            self_clone.get_agent_id().clone(),
                            status.clone(),
                        )))
                        .await;

                    // info!("Stage: {:?}", stage.name);
                    if (index + 1) == length {
                        let turn = CompletionTurn {
                            response_content: new_content,
                            response_id: last_response_id.clone(),
                            sequence: 1,
                            user_content: original_prompt.clone(),
                        };

                        self_clone
                            .execute_synthesizer(
                                &original_prompt,
                                &new_decision,
                                last_response_id.clone(),
                                vec![turn],
                                false,
                                &mut turn_response,
                                tx,
                            )
                            .await;
                        break;
                    } else {
                        // let (merged, sub_usage) =
                        let responses = match self_clone.execute_subs(&new_decision, store).await {
                            Ok(c) => c,
                            Err(e) => {
                                let error = format!("Executing sub agent error: {}", e);
                                error!(error);
                                let _ = tx.send(Err(HttpError::Other(error.to_string()))).await;
                                break;
                            }
                        };

                        let (merged, _sub_usage) = merge_responses(&responses);

                        let stage_response = StageResponse {
                            responses,
                            duration_ms: start.elapsed().as_millis() as u64,
                            name: stage.name.clone(),
                        };
                        turn_response.add_stage(stage_response);

                        info!(
                            _merged= ?format_args!("{:#?}", merged),
                            "Agent: {:?}", self_clone.get_agent_id()
                        );

                        new_content = merged;
                        // cusage += sub_usage;
                        // turn_response.usage = cusage.clone();
                    }

                    let elapsed = start.elapsed();
                    let done = format!("  ✅ {:.1}s\n", elapsed.as_secs_f32());
                    let _ = tx
                        .send(Ok(TurnChunkResponse::status(
                            self_clone.get_agent_id().clone(),
                            done,
                        )))
                        .await;
                }
            }
            .instrument(tracing::Span::current()),
        );

        Ok(ReceiverStream::new(rx))
    }

    async fn execute_streaming_dynamic(
        &self,
        turns: Vec<CompletionTurn>,
        prompt: &str,
        store: bool,
    ) -> HttpResult<ReceiverStream<HttpResult<TurnChunkResponse>>> {
        let (tx, rx) = mpsc::channel::<Result<TurnChunkResponse, HttpError>>(200);
        let self_clone = Arc::new(self.clone());
        let original_turns = turns.clone();
        let original_prompt = prompt.to_string();
        let (_, last_response_id) = build_messages_from_turns(&turns);
        let mut last_response_id = last_response_id;

        // mutate the input store
        let mut store = store;

        tokio::spawn(async move {
            let mut iteration = 0;
            const MAX_ITERATIONS: usize = 10;
            let mut usage = TokenUsage::default();
            let mut pipeline_turns = Vec::new();
            let mut turn_response = TurnResponse::for_dynamic(self_clone.get_agent_id().clone(),  original_prompt.clone());

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

                let (response, decision) = match self_clone.execute_decide_streaming(
                            &turns,
                            &original_prompt,
                            last_response_id.clone(),
                            store,
                            &tx,
                        )
                        .await
                    {
                        Some(c) => c,
                        None => break,
                    };

                // set store to false after the first loop of the orchestrator.
                store = false;

                debug!(
                    _agents= ?format_args!("{:#?}", decision.agents),
                    "Decision: {:?} Stop: {}", decision.execution, decision.stop
                );
                if !decision.stop && decision.agents.is_empty() {
                    let _ = tx
                    .send(Err(HttpError::Other("Orchestrator decision did not return any agents to run".to_string())))
                        .await;
                    break;
                }

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
                    .send(Ok(TurnChunkResponse::status(self_clone.get_agent_id().to_string(), status.clone())))
                    .await;

                if decision.stop {
                    self_clone.execute_synthesizer(&original_prompt, &decision, last_response_id, pipeline_turns, store, &mut turn_response, tx).await;
                    break;
                }

                let start = std::time::Instant::now();
                // let (merged, sub_usage) = 
                let responses =
                match self_clone.execute_subs(&decision, store).await {
                    Ok(c) => c,
                    Err(e) => {
                        let error = format!("Executing sub agent error: {}", e);
                        error!(error);
                        let _ = tx.send(Err(HttpError::Other(error.to_string()))).await;
                        break;
                    }
                };
                let (merged, sub_usage) = merge_responses(&responses);

                let elapsed = start.elapsed();
                let done = format!("  ✅ {:.1}s\n", elapsed.as_secs_f32());
                let _ = tx.send(Ok(TurnChunkResponse::status(self_clone.get_agent_id().to_string(), done))).await;

                info!(
                    "Merged: {:#?}", merged
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
        }.instrument(tracing::Span::current()));

        Ok(ReceiverStream::new(rx))
    }

    async fn execute_decide_streaming(
        &self,
        turns: &[CompletionTurn],
        prompt: &str,
        last_response_id: Option<String>,
        store: bool,
        tx: &Sender<Result<TurnChunkResponse, HttpError>>,
    ) -> Option<(AgentResponse, StageDecision)> {
        match self.decide(turns, prompt, last_response_id, store).await {
            Ok(c) => Some(c),
            Err(e) => {
                let _ = tx
                    .send(Err(HttpError::CompletionRequestError(e.to_string())))
                    .await;
                None
            }
        }
    }

    /// Run the sub-agents nominated by `decision` and return the merged JSON string
    /// plus the total token usage across all sub-calls.
    ///
    /// Sequential mode runs sub-agents one at a time in order. Parallel mode runs up to
    /// 5 concurrently (semaphore-bounded) each with a [`SUB_AGENT_TIMEOUT`]-second timeout;
    /// individual failures are logged and skipped rather than aborting the whole stage.
    #[tracing::instrument(
        skip(self),
        fields(
            _agent_id = %self.get_agent().id,
        )
    )]
    pub async fn execute_subs(
        &self,
        decision: &StageDecision,
        store: bool,
    ) -> Result<Vec<TurnResponse>> {
        // ) -> Result<(String, TokenUsage)> {
        // let mut responses: Vec<(String, TurnResponse)> = Vec::new();
        let mut responses = Vec::new();
        // execute the sub agents now.
        let subs = self.resolve_sub_agents(&decision.agents)?;

        match decision.execution {
            ExecutionMode::Sequential => {
                for sub in subs {
                    let response = sub.0.execute(Vec::new(), &sub.1, store).await?;
                    responses.push(response);
                    // debug!("Response: {:?}", response.content);
                    // responses.push((sub.0.get_agent_id().to_string(), response));
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
                                agent.execute(Vec::new(), &prompt, store),
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
                                "Agent: {} Response: {:?}",
                                response.agent_id, response.content
                            );
                            // responses.push((response.agent_id.clone(), response));
                            responses.push(response);
                        }
                        Err(e) => {
                            warn!("Agent call error: {}", e.to_string());
                        }
                    };
                }
            }
        }

        // Ok(merge_responses(&responses))
        Ok(responses)
    }

    #[tracing::instrument(
        skip(self),
        fields(
            _agent_id = %self.get_agent().id,
        )
    )]
    async fn execute_synthesizer(
        &self,
        prompt: &str,
        decision: &StageDecision,
        last_response_id: Option<String>,
        pipeline_turns: Vec<CompletionTurn>,
        store: bool,
        turn_response: &mut TurnResponse,
        tx: Sender<Result<TurnChunkResponse, HttpError>>,
    ) {
        let start = std::time::Instant::now();

        if decision.agents.len() > 1 {
            let _ = tx
                .send(Err(HttpError::Other(
                    "Synthesizer must have exactly one agent".to_string(),
                )))
                .await;
            return;
        }

        if matches!(decision.execution, ExecutionMode::Parallel) {
            let _ = tx
                .send(Err(HttpError::Other(
                    "Synthesizer must be sequential not parallel".to_string(),
                )))
                .await;
            return;
        }
        let subs = match self.resolve_sub_agents(&decision.agents) {
            Ok(c) => c,
            Err(e) => {
                let _ = tx
                    .send(Err(HttpError::Other(format!(
                        "Resolving Synthesizer error: {}",
                        e
                    ))))
                    .await;
                return;
            }
        };

        info!(
            _turns = format_args!("{:#?}", pipeline_turns),
            "Synthesizing..."
        );
        let synthesizer = subs[0].clone();

        let mut stream = match synthesizer
            .0
            .execute_streaming(pipeline_turns, &synthesizer.1, store)
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
                return;
            }
        };

        // pipe synthesizer stream to tx
        let mut chunk_count = 0;
        let agent_id = self.get_agent_id().clone();

        let streaming_start = std::time::Instant::now();

        while let Some(chunk_result) = stream.next().await {
            if chunk_count == 0 {
                let status = format!("  ✅ {:.1}s\n", start.elapsed().as_secs_f32());
                let _ = tx
                    .send(Ok(TurnChunkResponse::status(agent_id.clone(), status)))
                    .await;
                let status = format!("⚡ Streaming Response...");
                let _ = tx
                    .send(Ok(TurnChunkResponse::status(agent_id.clone(), status)))
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

            // info!("Turn Response: {:#?}", turn_response);

            trace!("Chunk: {:?}", chunk);

            if chunk.is_final() {
                // extract the agent response from the final chunk
                if let TurnChunkResponse::Final {
                    response: synthesizer_response,
                } = &chunk
                {
                    let final_response = synthesizer_response.clone();
                    turn_response.set_synthesizer(final_response);

                    info!(
                        _turn_response = format_args!(
                            "{:?} {:#?}",
                            turn_response.agent_id, turn_response.content
                        ),
                        "Synthesising done."
                    );

                    let status = format!("  ✅ {:.1}s\n", streaming_start.elapsed().as_secs_f32());
                    let _ = tx
                        .send(Ok(TurnChunkResponse::status(agent_id.clone(), status)))
                        .await;

                    let _ = tx
                        .send(Ok(TurnChunkResponse::final_response(turn_response.clone())))
                        .await;
                }
            } else {
                let _ = tx.send(Ok(chunk)).await;
            }
        }
    }

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
