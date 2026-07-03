use std::str::FromStr;

use anyhow::{Context, Result};
use rustic_agent::services::config::{
    agent::AgentConfig, mcp::MCPServerConfig, provider::ProviderConfig,
};
use rustic_core::load_content;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatTemplate {
    pub id: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub system_prompt: Option<String>,
    pub suggested_prompts: Vec<String>,
    pub recommended_llm: String,
    pub icon: String,
}

#[derive(Debug, Deserialize)]
struct TemplatesFile {
    templates: Vec<ChatTemplate>,
}

#[derive(Debug, Deserialize)]
struct ProviderConfigFile {
    llm_providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct McpConfigFile {
    pub mcp_servers: Vec<MCPServerConfig>,
}

#[derive(Debug, Deserialize)]
struct AgentConfigFile {
    agents: Vec<AgentConfig>,
}


pub async fn load_provider_config(providers_path: String) -> Result<Vec<ProviderConfig>> {
    let providers_content = load_content(providers_path).await?;
    let providers_file: ProviderConfigFile = serde_json::from_str(&providers_content)?;
    Ok(providers_file.llm_providers)
}

pub async fn load_mcp_config(mcp_config_path: String) -> Result<Vec<MCPServerConfig>> {
    let mcp_server_content = load_content(mcp_config_path).await?;
    let mcp_server_file: McpConfigFile = serde_json::from_str(&mcp_server_content)?;
    Ok(mcp_server_file.mcp_servers)
}

pub async fn load_agents_config(
    config_dir: String,
    agents_path: String,
) -> Result<Vec<AgentConfig>> {
    let agents_content = load_content(agents_path).await?;
    trace!("load_agents_config: {}", agents_content);
    let agents_file: AgentConfigFile = serde_json::from_str(&agents_content)?;

    let mut agents = agents_file.agents;
    // resolve system_prompt file paths to content
    for agent in &mut agents {

        // load description from .md if available — falls back to inline JSON description
        let desc_path = format!("{}/{}", config_dir, agent.description);
        debug!("description path: {:?}", desc_path);

        if let Ok(content) = load_content(desc_path).await {
            agent.description = content;
        }

        let prompt_path = format!("{}/{}", config_dir, agent.system_prompt);
        info!("System prompt path: {}", prompt_path);
        agent.system_prompt = load_content(prompt_path.clone()).await.with_context(|| {
            anyhow::anyhow!(
                "System prompt load for agent '{}' at '{}' error:",
                agent.id,
                prompt_path
            )
        })?;

        let response_format_schema_path = format!("{}/{}", config_dir, agent.response_format_schema_path);
        info!("Response format schema path: {}", response_format_schema_path);

        if let Ok(schema) = load_content(response_format_schema_path.clone()).await {
            agent.response_format_schema = Some(serde_json::Value::from_str(&schema)?);
            info!("response schema: {:?}", agent.response_format_schema);
        }

    }
    Ok(agents)
}

pub async fn load_chat_templates(chat_templates_path: String) -> Result<Vec<ChatTemplate>> {
    let chat_templates_content = load_content(chat_templates_path).await?;
    trace!("load_chat_templates: {}", chat_templates_content);
    let template_file: TemplatesFile = serde_json::from_str(&chat_templates_content)?;
    Ok(template_file.templates)
}
