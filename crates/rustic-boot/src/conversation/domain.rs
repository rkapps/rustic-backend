use anyhow::Result;
use chrono::{DateTime, Utc};
use rustic_agent::{
    client::response::CompletionResponseTokenUsage,
    services::config::agent::{CompletionStrategy, HistoryMode},
};
use rustic_storage::core::repository::RepoModel;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::conversation::{CONVERSATION_COLLECTION_NAME, TURN_COLLECTION_NAME};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversationType {
    Chat,
    Agent,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConversationRequest {
    pub conversation_type: ConversationType,
    pub title: Option<String>,

    // chat only
    pub template_id: Option<String>,

    // agent only
    pub agent_id: Option<String>,

    // both
    pub llm: String,   // provider user picked
    pub model: String, // model user picked
    pub stream: bool,
    pub system_prompt: Option<String>,

    pub strategy: Option<CompletionStrategy>,
    pub history_mode: Option<HistoryMode>,
    pub max_turns: Option<u32>, // only valid for trimmed
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Conversation {
    pub id: String,
    pub uid: String,
    pub conversation_type: ConversationType,
    pub title: String,
    pub template_id: Option<String>,   // "rust-expert"
    pub agent_id: Option<String>,      // None for chat
    pub system_prompt: Option<String>, // copied from template at creation
    pub orig_system_prompt: Option<String>,
    pub llm: String,   // provider user picked
    pub model: String, // model user picked
    pub stream: bool,
    pub response_id: Option<String>,
    #[serde(default)]
    pub strategy: CompletionStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_mode: Option<HistoryMode>, // None when strategy=stateless
    #[serde(default)]
    pub max_turns: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub usage: Option<CompletionResponseTokenUsage>,
    pub input_tokens_cost: f64,
    pub cached_read_tokens_cost: f64,
    pub cached_write_tokens_cost: f64,
    pub output_tokens_cost: f64,
    pub total_tokens_cost: f64,
}

impl RepoModel<String> for Conversation {
    fn id(&self) -> String {
        self.clone().id
    }
    fn collection(&self) -> &'static str {
        CONVERSATION_COLLECTION_NAME
    }
}

impl Conversation {
    pub fn from(uid: String, request: ConversationRequest) -> Conversation {
        let id = Uuid::new_v4().to_string();
        let title = request.title.unwrap_or_default();
        let now = Utc::now();

        let history_mode = Some(request.history_mode.unwrap_or_default());
        let max_turns = Some(request.max_turns.unwrap_or_default());

        Conversation {
            agent_id: request.agent_id,
            conversation_type: request.conversation_type,
            created_at: now,
            id,
            title,
            last_updated_at: now,
            llm: request.llm,
            model: request.model,
            stream: request.stream,
            response_id: None,
            system_prompt: request.system_prompt.clone(),
            orig_system_prompt: request.system_prompt,
            template_id: request.template_id,
            uid,
            strategy: request.strategy.unwrap_or(CompletionStrategy::Stateless),
            history_mode,
            max_turns,
            usage: None,
            input_tokens_cost: 0.0,
            cached_read_tokens_cost: 0.0,
            cached_write_tokens_cost: 0.0,
            output_tokens_cost: 0.0,
            total_tokens_cost: 0.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConversationUpdateRequest {
    pub title: Option<String>,
    pub system_prompt: Option<String>,
    pub history_mode: Option<HistoryMode>, // only valid if strategy=stateful
    pub max_turns: Option<u32>,            // only valid if history_mode=trimmed
}

impl Conversation {
    pub fn apply_update(&mut self, request: ConversationUpdateRequest) -> Result<()> {
        if let Some(title) = request.title {
            self.title = title;
        }
        if let Some(system_prompt) = request.system_prompt {
            self.system_prompt = Some(system_prompt);
        }
        if let Some(history_mode) = request.history_mode {
            if self.strategy != CompletionStrategy::Stateful {
                return Err(anyhow::anyhow!(
                    "history_mode only valid for stateful conversations"
                ));
            }
            self.history_mode = Some(history_mode);
        }
        if let Some(max_turns) = request.max_turns {
            if self.history_mode != Some(HistoryMode::Trimmed) {
                return Err(anyhow::anyhow!(
                    "max_turns only valid for trimmed history mode"
                ));
            }
            self.max_turns = Some(max_turns);
        }
        self.last_updated_at = Utc::now();
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Turn {
    pub id: String,
    pub uid: String,
    pub conversation_id: String,
    pub sequence: i32,
    pub user_prompt: String,
    pub response_content: String,
    pub response_id: Option<String>,
    pub usage: Option<CompletionResponseTokenUsage>,
    pub created_at: DateTime<Utc>,
    pub input_tokens_cost: f64,
    pub cached_read_tokens_cost: f64,
    pub cached_write_tokens_cost: f64,
    pub output_tokens_cost: f64,
    pub total_tokens_cost: f64,
    pub execution_time_ms: Option<u64>,
}

impl RepoModel<String> for Turn {
    fn id(&self) -> String {
        self.clone().id
    }
    fn collection(&self) -> &'static str {
        TURN_COLLECTION_NAME
    }
}
