//! Agentic tool-use example using OpenAI with an MCP server (Apify).
//!
//! Demonstrates the higher-level [`AgentService`] API: an [`MCPRegistry`] is
//! populated from a remote MCP endpoint and handed to the service, which then
//! exposes every registered tool to the agent automatically.
//!
//! The agent performs a multi-step research loop over Apify's web-scraping
//! actors to answer a question about furniture retail trends.
//!
//! ```bash
//! OPENAI_API_KEY=<key> APIFY_API_KEY=<key> cargo run --example completion_with_mcp
//! ```

use std::{env, sync::Arc, vec};

use anyhow::Result;
use rustic_agent::{
    client::{message::Message, preset::Preset},
    providers::openai,
    services::{
        agent::AgentService,
        registry::{agent::AgentRegistry, provider::ProviderRegistry},
    },
    tools::{
        mcp::{MCPRegistry, MCPServerSetting},
        tool::ToolRegistry,
    },
};
use rustic_core::logger::set_logger;
use tokio::sync::RwLock;

pub mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    set_logger("rustic_agent=info".to_string());
    let furniture_analyst_prompt = "You are a furniture industry market analyst. \
        Use Apify tools to research:\
        - Furniture retailer websites (IKEA, Wayfair, Williams Sonoma)\
        - Home decor trend sites\
        - Furniture review platforms\
        - Industry news sources\
        \
        For Reddit research focus on r/furniture, r/HomeDecorating, r/InteriorDesign\
        For e-commerce focus on Amazon furniture categories, Wayfair, IKEA";

    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY envrionment variable not set");
    let apify_key = env::var("APIFY_API_KEY").expect("APIFY_API_KEY environemtn variable not set");
    let model = openai::MODEL_GPT_5_4.to_string();

    let mut mcp_registry = MCPRegistry::new();
    let setting = MCPServerSetting {
        name: "Apify".to_string(),
        url: "https://mcp.apify.com".to_string(),
        api_key: apify_key,
    };

    let definitions = mcp_registry.register_server(setting.clone()).await?;
    mcp_registry.add_definitions(&setting.name, definitions);

    let agent_service = AgentService::from_registry(
        Arc::new(ProviderRegistry::new()),
        Arc::new(AgentRegistry::new()),
        Arc::new(RwLock::new(ToolRegistry::new())),
        Arc::new(RwLock::new(mcp_registry)),
    );

    let agent = agent_service
        .builder(&String::new())
        .with_system_prompt(furniture_analyst_prompt.to_string())
        .with_preset(Preset::Balanced)
        .with_openai(&api_key, &model)
        .await?
        .build()
        .await?;

    let content = "What are the top-rated sofas on Amazon right now?".to_string();
    let message = Message::user(content.clone());
    let response = agent.complete(&vec![message], None).await?;
    println!("Response: {:#?}", response);

    Ok(())
}
