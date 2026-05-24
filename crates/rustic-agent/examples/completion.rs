//! Multi-turn chat example using Gemini without tools.
//!
//! Demonstrates a two-turn conversation where the second [`Message::User`]
//! carries the `response_id` from the first reply so Gemini can continue
//! the same interaction thread.
//!
//! ```bash
//! GEMINI_API_KEY=<key> cargo run --example completion
//! ```

use std::{env, sync::Arc};

use anyhow::{Context, Result};
use rustic_agent::client::request::ReasoningEffort;
use rustic_agent::{
    agents::Agent,
    client::{message::Message, response::CompletionResponseContent},
    providers::gemini::{self, completion::GeminiClient},
    tools::{mcp::MCPRegistry, tool::ToolRegistry},
};

#[tokio::main]
async fn main() -> Result<()> {
    let mut messages = vec![];
    let system_prompt = Some("You are an elementary quiz coordinator. Design a multiple choise quiz after asking them about the grade, subject and difficult level. Provide 20 questions and rate them at the end.".to_string());
    let content = "Start the quiz";
    let mut message = Message::User {
        content: content.to_string(),
        response_id: None,
    };
    messages.push(message);

    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY envrionment variable not set");

    let client = GeminiClient::new(api_key.to_string())
        .with_context(|| anyhow::anyhow!("Error creating Gemini client"))?;

    let agent = Agent {
        id: "".to_string(),
        client: Arc::new(client),
        enable_cache: true,
        llm: "gemini".to_string(),
        max_tokens: 4098,
        mcp_registry: Arc::new(MCPRegistry::new()),
        model: gemini::MODEL_GEMINI_3_FLASH_PREVIEW.to_string(),
        reasoning_effort: ReasoningEffort::Medium,
        system_prompt,
        temperature: 0.7,
        tool_registry: Arc::new(ToolRegistry::new()),
    };

    // Turn 1: ask Gemini to start the quiz
    let response = agent.complete(&messages).await?;
    let response_id = response.response_id;
    let content = response.contents.get(0).unwrap();
    if let CompletionResponseContent::Text(val) = content {
        message = Message::Assistant {
            content: val.to_string(),
            response_id: Some(response_id.clone()),
        };
        messages.push(message);
    }

    // Turn 2: supply grade level, threading the same response_id so Gemini
    // continues the existing interaction rather than starting a new one.
    message = Message::User {
        content: "1st Grade".to_string(),
        response_id: Some(response_id),
    };
    messages.push(message);

    println!("complete start---");
    let response = agent.complete(&messages).await?;
    println!("Response: {:#?}", response);

    Ok(())
}
