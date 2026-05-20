use chrono::{DateTime, Utc};
use rustic_agent::client::response::CompletionResponseTokenUsage;
use serde::{Deserialize, Serialize};
use storage_core::core::RepoModel;
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
    pub system_prompt: Option<String>,

    // agent only
    pub agent_id: Option<String>,

    // both
    pub llm: String,   // provider user picked
    pub model: String, // model user picked
    pub stream: bool,
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
    pub llm: String,                   // provider user picked
    pub model: String,                 // model user picked
    pub stream: bool,
    pub response_id: Option<String>,
    // pub strategy: String,                 // "stateful"
    pub history_mode: String, // "trimmed_full"
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

        Conversation {
            agent_id: request.agent_id,
            conversation_type: request.conversation_type,
            created_at: now,
            history_mode: "stateful".to_string(),
            id,
            title,
            last_updated_at: now,
            llm: request.llm,
            model: request.model,
            stream: request.stream,
            response_id: None,
            system_prompt: request.system_prompt,
            template_id: request.template_id,
            uid,
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
}

impl RepoModel<String> for Turn {
    fn id(&self) -> String {
        self.clone().id
    }
    fn collection(&self) -> &'static str {
        TURN_COLLECTION_NAME
    }
}
