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

pub enum AgentHandle {
    Single(Agent),
    Pipeline(Arc<PipeLineRunner>),
}

impl AgentHandle {
    pub async fn execute_sub(
        &self,
        agent_config: AgentConfig,
        agent_id: &str,
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

        debug!(
            "Agent: {} Context: {:?} Input: {:#?}",
            agent_id, context, input
        );
        match self {
            AgentHandle::Single(agent) => agent.complete(&input).await,
            AgentHandle::Pipeline(runner) => Box::pin(runner.run(&input)).await,
        }
    }

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

    pub async fn decide(&self, messages: &[Message]) -> HttpResult<CompletionResponse> {
        match self {
            AgentHandle::Single(agent) => agent.complete(messages).await,
            AgentHandle::Pipeline(_) => Err(HttpError::CompletionRequestError(
                "Pipeline cannot be an orchestrator".to_string(),
            )),
        }
    }

    pub async fn execute(&self, original_messages: &[Message]) -> HttpResult<CompletionResponse> {
        match self {
            AgentHandle::Single(agent) => agent.complete(&original_messages).await,
            AgentHandle::Pipeline(runner) => {
                // force pipeline runner to be stateless
                let last = original_messages.last().unwrap();
                let mut input = Vec::new();
                input.push(last.clone());
                Box::pin(runner.run(&input)).await
            }
        }
    }

    pub async fn execute_streaming(
        &self,
        original_messages: &[Message],
    ) -> HttpResult<ReceiverStream<HttpResult<CompletionChunkResponse>>> {
        match self {
            AgentHandle::Single(agent) => agent.complete_with_streaming(&original_messages).await,
            AgentHandle::Pipeline(runner) => {
                let input = vec![original_messages.last().unwrap().clone()];
                Box::pin(runner.clone().run_dynamic_streaming(&input)).await
            }
        }
    }

    pub fn build_goal_input(original_messages: &[Message]) -> Vec<Message> {
        original_messages.to_vec()
    }

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

pub struct PipeLineRunner {
    pub orchestrator: AgentHandle,
    pub agent_config: AgentConfig,
    pub agent_handles: HashMap<String, AgentHandle>, // pre-built, recursive
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

    pub async fn run(&self, messages: &[Message]) -> HttpResult<CompletionResponse> {
        self.run_dynamic(messages).await
    }

    pub async fn run_dynamic(&self, messages: &[Message]) -> HttpResult<CompletionResponse> {
        let mut iteration = 0;

        let original_messages = messages;
        let mut all_messages = Vec::new();
        all_messages.extend(messages.to_vec());

        const MAX_ITERATIONS: usize = 10;

        info!("PipelineRunner - run_dynamic");
        loop {
            iteration += 1;
            if iteration > MAX_ITERATIONS {
                error!("Error: {}", HttpError::MaxIterationsExceeded);
                return Err(HttpError::MaxIterationsExceeded);
            }

            let pipeline_messages = all_messages.clone();

            info!("Loop: {} messsages: {}", iteration, all_messages.len());
            // only append if last message is not User
            if !matches!(all_messages.last(), Some(Message::User { .. })) {
                all_messages.push(Message::User {
                    content: "Based on the above, decide the next agents to run.".to_string(),
                    response_id: None,
                });
            }

            let response = self.orchestrator.decide(&all_messages).await.map_err(|_| {
                HttpError::CompletionRequestError("No stage decision returned".to_string())
            })?;
            let decision = build_stage_decision(response.clone())?;

            info!(
                "decision: {:?} excecution: {:#?} agents: {:?}",
                decision.stop, decision.execution, decision.agents
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
                response_id: None,
            });

            debug!("sub agent merged messages: {:#?}", merged);

            // if the decision is stop then return the response.
            if decision.stop {
                let final_content = unwrap_agent_content(&merged);
                let mut rcontents: Vec<CompletionResponseContent> = Vec::new();
                rcontents.push(CompletionResponseContent::Text(final_content));
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

        debug!("User Prompt {:#?}", messages);

        tokio::spawn(async move {
            let mut iteration = 0;
            const MAX_ITERATIONS: usize = 10;
            let mut usage = CompletionResponseTokenUsage::default();

            info!("PipelineRunner - run_dynamic_streaming\n");
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
                    "Loop: {} messsages: {}",
                    iteration,
                    spawn_all_messages.len()
                );
                // only append if last message is not User
                if !matches!(spawn_all_messages.last(), Some(Message::User { .. })) {
                    spawn_all_messages.push(Message::User {
                        content: "Based on the above, decide the next agents to run.".to_string(),
                        response_id: None,
                    });
                }

                let response = match runner.orchestrator.decide(&spawn_all_messages).await {
                    // HttpError::CompletionRequestError("No stage decision returned".to_string()),
                    Ok(c) => c,
                    Err(_) => {
                        let _ = tx
                            .send(Err(HttpError::CompletionRequestError(
                                "No stage decision returned".to_string(),
                            )))
                            .await;
                        break;
                    }
                };
                let decision = match build_stage_decision(response.clone()) {
                    Ok(c) => c,
                    Err(_) => {
                        let _ = tx
                            .send(Err(HttpError::CompletionRequestError(
                                "Stage decision build error".to_string(),
                            )))
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

                info!("decision: {:?} status: {:?}", decision.stop, status);
                info!(
                    "    Reasonining: {:?}\n",
                    decision.reasoning.clone().unwrap_or_default()
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
                                let _ = tx.send(Ok(CompletionChunkResponse::content(
                                    String::new(),
                                    format!("  ✅ {:.1}s\n", start.elapsed().as_secs_f32()),
                                    String::new(),
                                ))).await;
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

                                info!("Chunk: {:?}", chunk);

                                let mut final_chunk = chunk.clone();
                                final_chunk.is_final = false;
                                let _ = tx.send(Ok(final_chunk)).await;

                                usage += chunk.usage.unwrap();
                                info!("Synthesising and streaming done");

                                let _ = tx
                                    .send(Ok(CompletionChunkResponse::stop(
                                        agent_id.clone(),
                                        chunk.model,
                                        String::new(),
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

                    debug!("sub agent merged messages: {:#?}", merged);

                    spawn_all_messages.push(Message::Assistant {
                        content: merged.clone(),
                        response_id: Some(response.response_id),
                    });
                }

                let elapsed = loop_start.elapsed();
                let done = format!("  ✅ {:.1}s\n", elapsed.as_secs_f32());
                info!("Loop: {}{}", iteration, done);
            }
        });

        Ok(ReceiverStream::new(rx))
    }

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
                        info!("Sub agent {:?} executing...", sub_agent);
                        let response = agent_handle
                            .execute_sub(
                                self.agent_config.clone(),
                                &sub_agent,
                                original_messages,
                                &pipeline_messages,
                            )
                            .await?;

                        debug!("Sub agent response: {:#?}", response.text());
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

                        async move {
                            let _permit = sem.acquire().await.unwrap();
                            tokio::time::timeout(
                                timeout_duration,
                                handle.execute_sub(agent_config, &id, &all_msgs, &pipeline_msgs),
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
                            debug!("Sub agent response: {:#?}", response.text());
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
