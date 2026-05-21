pub mod domain;
pub mod dto;
pub mod helper;
pub mod service;

pub const CONVERSATION_COLLECTION_NAME: &str = "conversation";
pub const TURN_COLLECTION_NAME: &str = "turn";

pub const FIELD_UID: &str = "uid";
pub const FIELD_ID: &str = "id";
pub const FIELD_CONVERSATION_TYPE: &str = "conversation_type";
pub const FIELD_CONVERSATION_ID: &str = "conversation_id";
pub const FIELD_LLM: &str = "llm";
pub const FIELD_LAST_UPDATED_AT: &str = "last_updated_at";
