use anyhow::Result;
use rustic_storage::{core::{repository::Repository, search::{SearchCriteria, SearchOp, SearchValue}}, mongo::{database::MongoDatabase, repository::MongoRepository}};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::conversation::{CONVERSATION_COLLECTION_NAME, FIELD_CONVERSATION_ID, FIELD_CONVERSATION_TYPE, FIELD_ID, FIELD_LAST_UPDATED_AT, FIELD_LLM, FIELD_UID, TURN_COLLECTION_NAME, domain::{Conversation, Turn}, dto::ConversationsQuery};

#[derive(Debug)]
pub struct BootStorageManager {
    db: MongoDatabase,
}

impl BootStorageManager {
    pub async fn new(uri: &str, name: &str) -> Result<Self> {
        let mut mdb = MongoDatabase::new(uri, name).await?;

        mdb.register_collection::<String, Conversation>(CONVERSATION_COLLECTION_NAME.to_string())
            .await?;

        mdb.register_collection::<String, Turn>(TURN_COLLECTION_NAME.to_string())
            .await?;

        Ok(BootStorageManager { db: mdb })
    }

    pub async fn conversations(&self) -> Result<Arc<Mutex<MongoRepository<String, Conversation>>>> {
        self.db
            .collection::<String, Conversation>(CONVERSATION_COLLECTION_NAME.to_string())
            .await
    }

    pub async fn turns(&self) -> Result<Arc<Mutex<MongoRepository<String, Turn>>>> {
        self.db
            .collection::<String, Turn>(TURN_COLLECTION_NAME.to_string())
            .await
    }

    pub async fn count_turns(&self, uid: &str, conversation_id: &str) -> Result<usize> {
        let turns = self.get_turns(uid, conversation_id).await?;
        Ok(turns.len())
    }

    pub async fn create_conversation(&self, conversation: Conversation) -> Result<()> {
        let Ok(repo) = self.conversations().await else {
            return Err(anyhow::anyhow!("Error getting Conversation Repository",));
        };

        let mut repo = repo.lock().await;
        repo.insert(conversation).await?;
        Ok(())
    }

    pub async fn delete_conversation(&self, uid: &str, id: &str) -> Result<()> {
        let conversation = self.get_conversation(uid, id).await?;
        let Ok(repo) = self.conversations().await else {
            return Err(anyhow::anyhow!("Error getting Conversation Repository",));
        };

        let mut repo = repo.lock().await;
        repo.delete(conversation).await?;
        Ok(())
    }

    pub async fn delete_turns(&self, uid: &str, conversation_id: &str) -> Result<()> {
        let Ok(repo) = self.turns().await else {
            return Err(anyhow::anyhow!("Error getting Conversation Repository",));
        };

        let mut repo = repo.lock().await;
        let mut criteria = SearchCriteria::new();
        criteria.add_condition(
            FIELD_UID,
            SearchOp::Eq,
            SearchValue::String(uid.to_string()),
        );
        criteria.add_condition(
            FIELD_CONVERSATION_ID,
            SearchOp::Eq,
            SearchValue::String(conversation_id.to_string()),
        );

        repo.delete_many(Some(criteria)).await?;
        Ok(())
    }

    pub async fn get_conversations(
        &self,
        uid: &str,
        query: ConversationsQuery,
    ) -> Result<Vec<Conversation>> {
        let Ok(repo) = self.conversations().await else {
            return Err(anyhow::anyhow!("Error getting Conversation Repository",));
        };
        let mut repo = repo.lock().await;
        let mut criteria = SearchCriteria::new();
        criteria.add_condition(
            FIELD_UID,
            SearchOp::Eq,
            SearchValue::String(uid.to_string()),
        );
        if let Some(conversation_type) = query.conversation_type {
            criteria.add_condition(
                FIELD_CONVERSATION_TYPE,
                SearchOp::Eq,
                SearchValue::String(conversation_type),
            );
        }
        if let Some(llm) = query.llm {
            criteria.add_condition(FIELD_LLM, SearchOp::Eq, SearchValue::String(llm));
        }
        if let Some(from_date) = query.from_date {
            criteria.add_condition(
                FIELD_LAST_UPDATED_AT,
                SearchOp::Gte,
                SearchValue::DateTime(from_date),
            );
        }
        if let Some(to_date) = query.to_date {
            criteria.add_condition(
                FIELD_LAST_UPDATED_AT,
                SearchOp::Lte,
                SearchValue::DateTime(to_date),
            );
        }

        repo.find(Some(criteria)).await
    }

    pub async fn get_conversation(&self, uid: &str, id: &str) -> Result<Conversation> {
        let Ok(repo) = self.conversations().await else {
            return Err(anyhow::anyhow!("Error getting Conversation Repository",));
        };
        let mut repo = repo.lock().await;
        let mut criteria = SearchCriteria::new();
        criteria.add_condition(
            FIELD_UID,
            SearchOp::Eq,
            SearchValue::String(uid.to_string()),
        );
        criteria.add_condition(FIELD_ID, SearchOp::Eq, SearchValue::String(id.to_string()));
        repo.find_one(Some(criteria)).await
    }

    pub async fn get_turns(&self, uid: &str, conversation_id: &str) -> Result<Vec<Turn>> {
        let Ok(repo) = self.turns().await else {
            return Err(anyhow::anyhow!("Error getting Turn Repository",));
        };
        let mut repo = repo.lock().await;
        let mut criteria = SearchCriteria::new();
        criteria.add_condition(
            FIELD_UID,
            SearchOp::Eq,
            SearchValue::String(uid.to_string()),
        );
        criteria.add_condition(
            FIELD_CONVERSATION_ID,
            SearchOp::Eq,
            SearchValue::String(conversation_id.to_string()),
        );

        repo.find(Some(criteria)).await
    }

    pub async fn insert_turn(&self, turn: Turn) -> Result<()> {
        let Ok(repo) = self.turns().await else {
            return Err(anyhow::anyhow!("Error getting Turn Repository",));
        };
        let mut repo = repo.lock().await;
        repo.insert(turn).await
    }

    pub async fn update_turn(&self, turn: Turn) -> Result<()> {
        let Ok(repo) = self.turns().await else {
            return Err(anyhow::anyhow!("Error getting Turn Repository",));
        };
        let mut repo = repo.lock().await;
        repo.insert(turn).await
    }

    pub async fn update_conversation(&self, conversation: Conversation) -> Result<()> {
        let Ok(repo) = self.conversations().await else {
            return Err(anyhow::anyhow!("Error getting Turn Repository",));
        };
        let mut repo = repo.lock().await;
        repo.update(conversation).await
    }
}
