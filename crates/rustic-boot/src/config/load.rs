use std::{io::Write, path::PathBuf};

use anyhow::{Context, Result};
use google_cloud_storage::{
    client::{Client, ClientConfig},
    http::objects::{download::Range, get::GetObjectRequest},
};
use rustic_agent::services::config::{
    agent::AgentConfig, mcp::MCPServerConfig, provider::ProviderConfig,
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use tracing::trace;

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
        let prompt_path = format!("{}/{}", config_dir, agent.system_prompt);
        agent.system_prompt = load_content(prompt_path.clone()).await.with_context(|| {
            anyhow::anyhow!(
                "System prompt load for agent '{}' at '{}' error:",
                agent.id,
                prompt_path
            )
        })?;
    }
    Ok(agents)
}

pub async fn load_chat_templates(chat_templates_path: String) -> Result<Vec<ChatTemplate>> {
    let chat_templates_content = load_content(chat_templates_path).await?;
    trace!("load_chat_templates: {}", chat_templates_content);
    let template_file: TemplatesFile = serde_json::from_str(&chat_templates_content)?;
    Ok(template_file.templates)
}

pub async fn load_content(path: String) -> Result<String> {
    let content = if path.starts_with("gs://") {
        download_gcs_string(&path).await?
    } else {
        std::fs::read_to_string(&path)?
    };
    Ok(content)
}

pub async fn download_gcs_bytes(gcs_path: &str) -> anyhow::Result<Vec<u8>> {
    let path = gcs_path
        .strip_prefix("gs://")
        .ok_or_else(|| anyhow::anyhow!("Invalid GCS path: {}", gcs_path))?;
    let (bucket, object) = path
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("Invalid GCS path: {}", gcs_path))?;

    let config = ClientConfig::default().with_auth().await?;
    let client = Client::new(config);

    let data = client
        .download_object(
            &GetObjectRequest {
                bucket: bucket.to_string(),
                object: object.to_string(),
                ..Default::default()
            },
            &Range::default(),
        )
        .await?;

    Ok(data)
}

// for files that need a temp path (xlsx, models)
pub async fn download_gcs_to_file(gcs_path: &str) -> anyhow::Result<PathBuf> {
    let data = download_gcs_bytes(gcs_path).await?;
    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(&data)?;
    Ok(tmp.into_temp_path().keep()?)
}

// for text content (config, json)
pub async fn download_gcs_string(gcs_path: &str) -> anyhow::Result<String> {
    let data = download_gcs_bytes(gcs_path).await?;
    Ok(String::from_utf8(data)?)
}
