use crate::{
    Agent, CompletionResponse, CompletionResponseContent, Message,
    agents::{
        ExecutionMode,
        helper::{build_agent_messages, build_clean_response_text, build_stage_decision, is_decide_prompt, is_orchestrator_decision},
    },
    services::config::agent::{AgentConfig, AgentContext},
};
use rustic_core::{HttpError, HttpResult};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

pub enum AgentHandle {
    Single(Agent),
    Pipeline(Box<PipeLineRunner>),
}

impl AgentHandle {
    pub async fn execute_sub(
        &self,
        agent_config: AgentConfig,
        agent_id: &str,
        original_messages: &[Message],
        all_messages: &[Message],
        pipeline_messages: &[Message],
    ) -> HttpResult<CompletionResponse> {
        let context = agent_config
            .pipeline
            .as_ref()
            .and_then(|p| p.available_agents.iter().find(|a| a.id == agent_id))
            .map(|a| a.context.clone())
            .unwrap_or(AgentContext::Last);

        let input = match context {
            AgentContext::Goal => original_messages.to_vec(),
            AgentContext::Last => {
                let last = pipeline_messages
                    .iter()
                    .rev()
                    .find(|m| matches!(m, Message::Assistant { .. }));

                match last {
                    Some(Message::Assistant { content, .. }) => vec![Message::User {
                        content: content.clone(),
                        response_id: None,
                    }],
                    _ => original_messages.to_vec(),
                }
            }
            AgentContext::All => {
                let mut input = original_messages.to_vec();
    
                let data: Vec<Message> = all_messages.iter()
                    .filter(|m| !is_orchestrator_decision(m) && !is_decide_prompt(m))
                    .cloned()
                    .collect();
                
                input.extend(data);
                
                // clear handoff instruction
                input.push(Message::User {
                    content: "Synthesise all the research above into a final response for the user.".to_string(),
                    response_id: None,
                });
                
                input     
            }
        };

        debug!(
            "Agent: {} Context: {:?} Input: {:#?}",
            agent_id, context, input
        );
        match self {
            AgentHandle::Single(agent) => agent.complete_with_tools(&input).await,
            AgentHandle::Pipeline(runner) => Box::pin(runner.run(&input)).await,
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

    pub async fn execute(
        &self,
        original_messages: &[Message],
    ) -> HttpResult<CompletionResponse> {

        match self {
            AgentHandle::Single(agent) => agent.complete_with_tools(&original_messages).await,
            AgentHandle::Pipeline(runner) => {
                // force pipeline runner to be stateless
                let last = original_messages.last().unwrap();
                let mut input = Vec::new();
                input.push(last.clone());
                Box::pin(runner.run(&input)).await
            }
        }
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
            let cresponse = response.clone();
            let decision = build_stage_decision(response.clone())?;

            info!(
                "decision: {:?} excecution: {:#?} agents: {:?}",
                decision.stop, decision.execution, decision.agents
            );

            // Collect sub agent assistant messages.
            let mut sub_agent_messages = Vec::new();

            match decision.execution {
                ExecutionMode::Sequential => {
                    for sub_agent in decision.agents {
                        if let Some(agent_handle) = self.agent_handles.get(&sub_agent) {
                            info!("Sub agent {:?} executing...", sub_agent);
                            let response = agent_handle
                                .execute_sub(
                                    self.agent_config.clone(),
                                    &sub_agent,
                                    original_messages,
                                    &all_messages,
                                    &pipeline_messages,
                                )
                                .await?;

                            let nmessages = build_agent_messages(response.clone());
                            info!("Sub agent response: {:#?}", build_clean_response_text(response.text()));

                            sub_agent_messages.extend_from_slice(&nmessages);
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
                            let pipeline_msgs = pipeline_messages.clone();
                            let original_msgs = original_messages.to_vec();
                            let all_msgs = all_messages.to_vec();
                            let timeout_duration = Duration::from_secs(60);
                            let agent_config = self.agent_config.clone();

                            async move {
                                let _permit = sem.acquire().await.unwrap();
                                tokio::time::timeout(
                                    timeout_duration,
                                    handle.execute_sub(
                                        agent_config,
                                        &id,
                                        &all_msgs,
                                        &original_msgs,
                                        &pipeline_msgs,
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
                                let nmessages = build_agent_messages(response.clone());
                                info!("Sub agent response: {:#?}", build_clean_response_text(response.text()));
                                sub_agent_messages.extend_from_slice(&nmessages);
                            }
                            Err(e) => {
                                warn!("Agent call error: {}", e.to_string());
                            }
                        };
                    }
                }
            }

            // debug!("sub agent messages: {:#?}", sub_agent_messages);
            let merged = sub_agent_messages
                .iter()
                .rev()
                .take_while(|m| matches!(m, Message::Assistant { .. }))
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .filter_map(|m| match m {
                    Message::Assistant { content, .. } => Some(content.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            all_messages.push(Message::Assistant {
                content: merged.clone(),
                response_id: Some(cresponse.response_id),
            });

            debug!("sub agent merged messages: {:#?}", merged);

            // if the decision is stop then return the response.
            if decision.stop {
                let mut rcontents: Vec<CompletionResponseContent> = Vec::new();
                rcontents.push(CompletionResponseContent::Text(merged));
                let rresponse = CompletionResponse {
                    model: response.model,
                    response_id: String::new(),
                    contents: rcontents,
                    usage: response.usage,
                };

                return Ok(rresponse);
            }
        }
    }
}
