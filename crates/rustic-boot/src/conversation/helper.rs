use rustic_agent::{
    client::{message::Message, response::CompletionResponseTokenUsage},
    services::{config::agent::{ConversationStrategy, HistoryMode}, registry::provider::ProviderRegistry},
};

use crate::conversation::domain::Turn;

pub fn build_completions_messages(
    turns: Vec<Turn>,
    strategy: &ConversationStrategy,
    history_mode: Option<&HistoryMode>,
    max_turns: Option<u32>,
) -> Vec<Message> {
    match strategy {
        ConversationStrategy::Stateless => {
            // no history — current message will be added after
            vec![]
        }
        ConversationStrategy::Stateful => {
            let turns = match (history_mode, max_turns) {
                (Some(HistoryMode::Trimmed), Some(max)) => {
                    let skip = turns.len().saturating_sub(max as usize);
                    turns.into_iter().skip(skip).collect::<Vec<_>>()
                }
                _ => turns, // full — all turns
            };

            let mut messages = Vec::new();
            for turn in turns {
                messages.push(Message::User {
                    content: turn.user_prompt,
                    response_id: None,
                });
                messages.push(Message::Assistant {
                    content: turn.response_content,
                    response_id: turn.response_id,
                });
            }
            messages
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
