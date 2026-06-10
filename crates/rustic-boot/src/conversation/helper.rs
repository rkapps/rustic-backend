use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::info;

use rustic_agent::{
    AgentService,
    agents::{
        domain::{AgentInput, CompletionTurn},
        runner::Runnable,
    },
    client::response::CompletionResponseTokenUsage,
    services::{
        config::agent::{CompletionStrategy, HistoryMode},
        registry::provider::ProviderRegistry,
    },
};

use crate::{Conversation, conversation::domain::Turn};

pub async fn build_agent_runner(
    agent_service: Arc<AgentService>,
    conversation: &Conversation,
) -> Result<Arc<dyn Runnable>> {
    let agent_id = conversation.agent_id.clone().context(format!(
        "Conversation {} does not have an agent",
        conversation.id
    ))?;

    let input = AgentInput::new(
        agent_id,
        conversation.llm.clone(),
        conversation.model.clone(),
        conversation.system_prompt.clone(),
        conversation.strategy.clone(),
        None,
    );

    agent_service.build_runnable(&input).await
}

pub fn build_completion_turns(
    conversation: &Conversation,
    turns: Vec<Turn>,
) -> Vec<CompletionTurn> {
    info!(
        "Conversation strategy: {:?} history mode: {:?}",
        conversation.strategy, conversation.history_mode
    );

    match conversation.strategy {
        CompletionStrategy::Stateless => {
            // no history — current message will be added after
            vec![]
        }
        CompletionStrategy::Stateful => {
            let turns = match (conversation.history_mode.clone(), conversation.max_turns) {
                (Some(HistoryMode::Trimmed), Some(max)) => {
                    let skip = turns.len().saturating_sub(max as usize);
                    turns.into_iter().skip(skip).collect::<Vec<_>>()
                }
                _ => turns, // full — all turns
            };

            let mut completion_turns = Vec::new();
            for turn in turns {
                completion_turns.push(CompletionTurn {
                    sequence: turn.sequence as u32,
                    user_content: turn.user_prompt,
                    response_content: turn.response_content,
                    response_id: turn.response_id,
                })
            }
            completion_turns
        }
    }

}


pub fn calculate_turn_cost(
    llm: &str,
    model: &str,
    usage: &Option<CompletionResponseTokenUsage>,
    provider_registry: &ProviderRegistry,
) -> (f64, f64, f64, f64, f64) {
    if let Some(usage) = usage
        && let Some(provider) = provider_registry.find(llm)
        && let Some(model_config) = provider.clone().models.iter().find(|m| m.id == model)
    {
        let input_tokens_cost =
            (usage.input_tokens as f64 / 1000.0) * model_config.input_cost_per_1k;
        let cached_read_tokens_cost =
            (usage.cached_read_tokens as f64 / 1000.0) * model_config.cached_read_cost_per_1k;
        let cached_write_tokens_cost =
            (usage.cached_write_tokens as f64 / 1000.0) * model_config.cached_write_cost_per_1k;
        let output_tokens_cost =
            (usage.output_tokens as f64 / 1000.0) * model_config.output_cost_per_1k;

        let total_cost = input_tokens_cost
            + cached_read_tokens_cost
            + cached_write_tokens_cost
            + output_tokens_cost;

        return (
            input_tokens_cost,
            cached_read_tokens_cost,
            cached_write_tokens_cost,
            output_tokens_cost,
            total_cost,
        );
    }
    (0.0, 0.0, 0.0, 0.0, 0.0)
}
