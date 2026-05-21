use anyhow::Result;
use chrono::Utc;
use rustic_agent::{
    agents::Agent,
    client::{
        llm::CompletionStreamResponse,
        message::Message,
        response::{CompletionResponseContent, CompletionResponseTokenUsage},
    },
    services::agent::AgentService,
};
use std::sync::Arc;
use tracing::debug;

use crate::{
    conversation::{
        domain::{Conversation, ConversationRequest, ConversationType, Turn},
        dto::{ConversationsQuery, TurnRequest, TurnResponse},
        helper::{build_completions_message, calculate_turn_cost},
    },
    storage::manager::BootStorageManager,
};

pub struct ConversationService {
    agent_service: Arc<AgentService>,
    storage_manager: Arc<BootStorageManager>,
}

impl ConversationService {
    pub fn new(agent_service: Arc<AgentService>, storage_manager: Arc<BootStorageManager>) -> Self {
        Self {
            agent_service,
            storage_manager,
        }
    }

    pub async fn create_conversation(
        &self,
        uid: String,
        request: ConversationRequest,
    ) -> Result<Conversation> {
        let conversation = Conversation::from(uid, request);
        debug!(
            "Conversation: {:?} type: {:?} llm: {} model: {}",
            conversation.id, conversation.conversation_type, conversation.llm, conversation.model
        );

        self.storage_manager
            .create_conversation(conversation.clone())
            .await
            .map_err(|e| anyhow::anyhow!(format!("Create Conversation error: {}", e)))?;

        Ok(conversation)
    }

    pub async fn delete_conversation(&self, uid: &str, id: &str) -> Result<()> {
        self.storage_manager.delete_conversation(uid, id).await?;
        self.storage_manager.delete_turns(uid, id).await?;
        Ok(())
    }

    pub async fn get_conversations(
        &self,
        uid: String,
        query: ConversationsQuery,
    ) -> Result<Vec<Conversation>> {
        let conversations = self
            .storage_manager
            .get_conversations(&uid, query)
            .await
            .map_err(|e| anyhow::anyhow!(format!("Create Conversation error: {}", e)))?;

        debug!("Conversations: {}", conversations.len());
        Ok(conversations)
    }

    pub async fn get_conversation(&self, uid: &str, id: &str) -> Result<Conversation> {
        let conversation = self
            .storage_manager
            .get_conversation(uid, id)
            .await
            .map_err(|e| anyhow::anyhow!(format!("Get Conversation error: {}", e)))?;

        debug!("Conversations: {}", conversation.id);

        Ok(conversation)
    }

    pub async fn get_turns(&self, uid: &str, conversation_id: &str) -> Result<Vec<Turn>> {
        let turns = self
            .storage_manager
            .get_turns(uid, conversation_id)
            .await
            .map_err(|e| anyhow::anyhow!(format!("Get Turn error: {}", e)))?;

        Ok(turns)
    }

    pub async fn save_turn(
        &self,
        uid: &str,
        conversation_id: &str,
        user_prompt: String,
        response_content: String,
        response_id: Option<String>,
        usage: Option<CompletionResponseTokenUsage>,
    ) -> Result<Turn> {
        let conversation = self
            .get_conversation(uid, conversation_id)
            .await
            .map_err(|e| {
                anyhow::anyhow!(format!(
                    "Conversation {} does not exit. Error: {}",
                    conversation_id, e
                ))
            })?;

        let id = uuid::Uuid::new_v4().to_string();
        let sequence = self
            .storage_manager
            .count_turns(uid, conversation_id)
            .await?
            + 1;

        let mut turn = Turn {
            conversation_id: conversation_id.to_string(),
            id,
            response_content,
            response_id: response_id.clone(),
            sequence: sequence as i32,
            uid: uid.to_string(),
            user_prompt,
            usage: usage.clone(),
            created_at: Utc::now(),
            input_tokens_cost: 0.0,
            cached_read_tokens_cost: 0.0,
            cached_write_tokens_cost: 0.0,
            output_tokens_cost: 0.0,
            total_tokens_cost: 0.0,
        };

        // update turn cost.
        self.update_turn_cost(&conversation.llm, &conversation.model, &mut turn);

        // save turne
        self.storage_manager.insert_turn(turn.clone()).await?;

        // update converstation and cost
        if let Some(usage) = usage {
            self.update_conversation_for_turn(uid, conversation_id, response_id, usage, &turn)
                .await?;
        }

        Ok(turn)
    }

    pub async fn send_turn(
        &self,
        uid: &str,
        conversation_id: &str,
        request: TurnRequest,
    ) -> Result<TurnResponse> {
        debug!("Turn: {}", conversation_id);
        let conversation = self
            .get_conversation(uid, conversation_id)
            .await
            .map_err(|e| {
                anyhow::anyhow!(format!(
                    "Conversation {} does not exit. Error: {}",
                    conversation_id, e
                ))
            })?;

        let turns = self.get_turns(uid, conversation_id).await?;
        let mut messages = build_completions_message(turns);
        let nmessage = Message::User {
            content: request.prompt.clone(),
            response_id: None,
        };
        messages.push(nmessage);

        let agent = self.build_conversation_agent(&conversation).await?;
        let cresponse = agent.complete(&messages).await?;

        let mut rcontent = String::new();
        let response = cresponse.clone();
        let usage = cresponse.usage;
        let response_id = response.response_id.clone();
        for content in response.contents {
            if let CompletionResponseContent::Text(val) = content {
                // println!("The text is: {}", val);
                rcontent = val.to_string();
                // chat.update_assistant_message(val.to_string(), response.response_id.clone());
            }
        }

        // save turn
        self.save_turn(
            uid,
            conversation_id,
            request.prompt,
            rcontent.clone(),
            Some(response_id.clone()),
            Some(usage.clone()),
        )
        .await?;

        let tresponse = TurnResponse {
            role: "assistant".to_string(),
            content: Some(rcontent),
            response_id: Some(response.response_id),
        };

        Ok(tresponse)
    }

    pub async fn send_turn_streaming(
        &self,
        uid: &str,
        conversation_id: &str,
        request: TurnRequest,
    ) -> Result<CompletionStreamResponse> {
        debug!("Turn: {}", conversation_id);
        let conversation = self
            .get_conversation(uid, conversation_id)
            .await
            .map_err(|e| {
                anyhow::anyhow!(format!(
                    "Conversation {} does not exit. Error: {}",
                    conversation_id, e
                ))
            })?;

        let turns = self.get_turns(uid, conversation_id).await?;
        let mut messages = build_completions_message(turns);
        let nmessage = Message::User {
            content: request.prompt,
            response_id: None,
        };
        messages.push(nmessage);

        let agent = self.build_conversation_agent(&conversation).await?;
        let stream = agent.complete_with_tools_streaming(&messages).await?;
        Ok(Box::pin(stream))
    }

    pub async fn build_conversation_agent(&self, conversation: &Conversation) -> Result<Agent> {
        match conversation.conversation_type {
            ConversationType::Chat => {
                self.agent_service
                    .build_chat_agent(
                        &conversation.llm,
                        &conversation.model,
                        conversation.system_prompt.clone(),
                    )
                    .await
            }
            ConversationType::Agent => {
                let agent_id = conversation
                    .agent_id
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Agent conversation missing agent_id"))?;

                self.agent_service
                    .build_agent_for_id(agent_id, &conversation.llm, &conversation.model)
                    .await
            }
        }
    }

    pub async fn recalculate_conversation_usage_cost(&self, uid: &str, id: &str) -> Result<()> {
        let mut conversation = self.get_conversation(uid, id).await.map_err(|e| {
            anyhow::anyhow!(format!("Conversation {} does not exit. Error: {}", id, e))
        })?;

        conversation.usage = None;
        conversation.input_tokens_cost = 0.0;
        conversation.cached_read_tokens_cost = 0.0;
        conversation.cached_write_tokens_cost = 0.0;
        conversation.output_tokens_cost = 0.0;
        conversation.total_tokens_cost = 0.0;

        let turns = self.get_turns(uid, id).await?;
        for mut turn in turns {
            self.update_turn_cost(&conversation.llm, &conversation.model, &mut turn);

            // save turn
            self.storage_manager.update_turn(turn.clone()).await?;
        }

        // update conversation
        self.storage_manager.update_conversation(conversation).await
    }

    pub async fn update_conversation_for_turn(
        &self,
        uid: &str,
        conversation_id: &str,
        response_id: Option<String>,
        usage: CompletionResponseTokenUsage, // conversation: Conversation
        turn: &Turn,
    ) -> Result<()> {
        let mut conversation = self
            .get_conversation(uid, conversation_id)
            .await
            .map_err(|e| {
                anyhow::anyhow!(format!(
                    "Conversation {} does not exit. Error: {}",
                    conversation_id, e
                ))
            })?;

        // update conversation
        if let Some(mut tusage) = conversation.usage {
            tusage += usage.clone();
            conversation.usage = Some(tusage);
        } else {
            conversation.usage = Some(usage.clone());
        }
        conversation.last_updated_at = Utc::now();
        conversation.response_id = response_id;
        conversation.input_tokens_cost += turn.input_tokens_cost;
        conversation.cached_read_tokens_cost += turn.cached_read_tokens_cost;
        conversation.cached_write_tokens_cost += turn.cached_write_tokens_cost;
        conversation.output_tokens_cost += turn.output_tokens_cost;
        conversation.total_tokens_cost += turn.total_tokens_cost;

        self.storage_manager.update_conversation(conversation).await
    }

    pub fn update_turn_cost(&self, llm: &str, model: &str, turn: &mut Turn) {
        let (
            input_tokens_cost,
            cached_read_tokens_cost,
            cached_write_tokens_cost,
            output_tokens_cost,
            total_tokens_cost,
        ) = calculate_turn_cost(
            llm,
            model,
            &turn.usage,
            &self.agent_service.provider_registry,
        );

        turn.input_tokens_cost = input_tokens_cost;
        turn.cached_read_tokens_cost = cached_read_tokens_cost;
        turn.cached_write_tokens_cost = cached_write_tokens_cost;
        turn.output_tokens_cost = output_tokens_cost;
        turn.total_tokens_cost = total_tokens_cost;
    }
}
