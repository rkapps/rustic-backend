use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ConversationsQuery {
    pub conversation_type: Option<String>,
    pub llm: Option<String>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TurnRequest {
    pub prompt: String,
    // pub attachments: Option<Vec<Attachment>>,
}

#[derive(Debug, Serialize)]
pub struct TurnResponse {
    pub role: String,
    pub content: Option<String>,
    pub response_id: Option<String>,
}
