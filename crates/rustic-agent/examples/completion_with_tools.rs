//! Agentic tool-use example using Gemini with a local `GetWeatherTool`.
//!
//! Shows how to register a custom [`Tool`] implementation, then let the agent
//! drive a multi-step tool loop automatically via [`Agent::complete_with_tools`].
//! The agent will call `get_weather` for each city and synthesise the results
//! into a final text response.
//!
//! ```bash
//! GEMINI_API_KEY=<key> cargo run --example completion_with_tools
//! ```

pub mod tools;
use anyhow::{Context, Result};
use rustic_agent::client::request::ReasoningEffort;
use rustic_agent::{
    agents::Agent,
    client::message::Message,
    providers::gemini::{self, completion::GeminiClient},
    tools::{mcp::MCPRegistry, tool::ToolRegistry},
};
use rustic_core::set_logger;

use std::{env, sync::Arc};

use crate::tools::get_weather::GetWeatherTool;

#[tokio::main]
async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        "rustic_ai_api=debug,rustic_boot=info,rustic_agent=debug,fin_analyse=info".to_string()
    });
    set_logger(filter);

    let api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY envrionment variable not set");
    let client = GeminiClient::new(api_key.to_string())
        .with_context(|| anyhow::anyhow!("Error creating Anthropic client"))?;

    let tool = GetWeatherTool {};
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register_tool(tool);
    let system_prompt = Some("You are a weather expert".to_string());

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
        store: true,
        temperature: 0.7,
        tool_registry: Arc::new(tool_registry),
    };

    let content = "what is the weather in paris and San Fransicso".to_string();
    let message = Message::user(content.clone());
    println!("completion start---");
    let response = agent.complete(&vec![message], None).await?;
    println!("Response: {:#?}", response);

    Ok(())
}
