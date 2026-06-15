//! Application bootstrap layer for rustic-ai: wires configuration, storage,
//! conversation management, and HTTP routes into a running Axum server.
//!
//! # Re-exports
//!
//! ```no_run
//! // Boot
//! use rustic_boot::{BootState, AgenticBootBuilder};
//!
//! // Conversation
//! use rustic_boot::{Conversation, ConversationRequest, ConversationType, Turn};
//! use rustic_boot::{ConversationService, TurnRequest, TurnResponse};
//!
//! // Auth
//! use rustic_boot::{FirebaseClaims, firebase_auth_middleware};
//! ```

pub mod auth;
pub mod boot;
pub mod config;
pub mod conversation;
pub mod routes;
pub mod storage;
pub mod schema;

// Boot
pub use boot::{AgenticBootBuilder, BootState, McpServerEntry};

// Conversation domain and service
pub use conversation::domain::{Conversation, ConversationRequest, ConversationType, Turn};
pub use conversation::dto::{ConversationsQuery, TurnRequest, TurnResponse};
pub use conversation::service::ConversationService;

// Storage
pub use storage::manager::BootStorageManager;

// Auth
pub use auth::firebase::{FirebaseClaims, FirebaseKeyCache, firebase_auth_middleware};
