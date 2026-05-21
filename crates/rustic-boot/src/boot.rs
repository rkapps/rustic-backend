use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{Router, extract::FromRef, http::HeaderValue};
use reqwest::{Method, StatusCode, header};
use rustic_agent::{
    client::{mcp::MCPServerAdapter, tools::Tool},
    services::{
        agent::AgentService,
        config::{
            mcp::MCPServerConfig,
            provider::{ProviderConfig, ResolvedProvider},
        },
        registry::{agent::AgentRegistry, provider::ProviderRegistry},
    },
    tools::{mcp::MCPRegistry, tool::ToolRegistry},
};
use tokio::{net::TcpListener, sync::RwLock};
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

use crate::{
    auth::firebase::{FirebaseKeyCache, fetch_firebase_keys},
    config::load::{
        ChatTemplate, load_agents_config, load_chat_templates, load_mcp_config,
        load_provider_config,
    },
    conversation::service::ConversationService,
    storage::manager::BootStorageManager,
};

// agentic-boot
#[derive(Clone)]
pub struct BootState {
    pub agent_service: Arc<AgentService>,
    pub conversation_service: Option<Arc<ConversationService>>,
    pub chat_templates: Vec<ChatTemplate>,
    pub firebase_keys: Arc<RwLock<FirebaseKeyCache>>,
    pub firebase_project_id: String,
}

impl BootState {
    pub fn conversation_service(&self) -> Result<&Arc<ConversationService>, (StatusCode, String)> {
        self.conversation_service.as_ref().ok_or_else(|| {
            (
                StatusCode::NOT_IMPLEMENTED,
                "Conversation storage not configured".to_string(),
            )
        })
    }
}

pub struct McpServerEntry {
    pub config: MCPServerConfig,
    pub adapter: Box<dyn MCPServerAdapter>,
    pub enabled_tools: Vec<String>, // optional - which tools to expose
}

pub struct AgenticBootBuilder {
    agents_config_path: Option<String>,
    config_dir: Option<String>,
    providers_path: Option<String>,
    chat_templates_path: Option<String>,
    mcp_servers_config_path: Option<String>,
    mongo_uri: Option<String>,
    mongo_db: Option<String>,
    cors_origins: Vec<String>,
    firebase_project_id: Option<String>,
    tools: Vec<Arc<dyn Tool>>,
}

impl Default for AgenticBootBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AgenticBootBuilder {
    pub fn new() -> Self {
        Self {
            agents_config_path: None,
            config_dir: None,
            providers_path: None,
            chat_templates_path: None,
            mcp_servers_config_path: None,
            mongo_uri: None,
            mongo_db: None,
            cors_origins: Vec::new(),
            firebase_project_id: None,
            tools: Vec::new(),
            // mcp_servers: Vec::new(),
        }
    }

    pub fn cors_origins(mut self, origins: Vec<&str>) -> Self {
        self.cors_origins = origins.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn config_dir(mut self, config_dir: String) -> Self {
        self.config_dir = Some(config_dir);
        self
    }

    pub fn providers(mut self, providers_path: String) -> Self {
        self.providers_path = Some(providers_path);
        self
    }

    pub fn agents_config(mut self, agents_path: String) -> Self {
        self.agents_config_path = Some(agents_path);
        self
    }

    pub fn mcp_config(mut self, mcp_servers_config_path: String) -> Self {
        self.mcp_servers_config_path = Some(mcp_servers_config_path);
        self
    }

    pub fn chat_templates(mut self, chat_templates_path: String) -> Self {
        self.chat_templates_path = Some(chat_templates_path);
        self
    }

    pub fn firebase_project_id(mut self, project_id: &str) -> Self {
        self.firebase_project_id = Some(project_id.to_string());
        self
    }

    pub fn mongo_database(mut self, uri: String, db: String) -> Self {
        self.mongo_uri = Some(uri);
        self.mongo_db = Some(db);
        self
    }

    pub fn tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    pub async fn build(self) -> Result<BootState> {
        // add chat templates
        info!("AgenticBootBuilder build...");

        let mut chat_templates = Vec::new();
        let config_dir = self.config_dir.unwrap_or_default();
        let chat_templates_path = self.chat_templates_path;
        let providers_path = self.providers_path;
        let agents_config_path = self.agents_config_path;
        let mcp_servers_config_path = self.mcp_servers_config_path;

        let mongo_uri = self.mongo_uri;
        let mongo_db = self.mongo_db;

        let firebase_project_id = self.firebase_project_id.unwrap_or_default();

        // register tools
        let mut tool_registry = ToolRegistry::new();
        for tool in self.tools {
            info!("Tool: {:?}", tool.name());
            tool_registry.register_tool_arc(tool);
        }

        info!("Config directory: {:?}", config_dir);
        info!("Firebase project: {:}", firebase_project_id);
        info!("Mongo Uri: {:?} db: {:?}", mongo_uri, mongo_db);

        if let Some(path) = chat_templates_path {
            let full_path = format!("{}/{}", config_dir, path);
            info!("ChatTemplate path: {}", full_path);
            chat_templates = load_chat_templates(full_path).await?;
        }

        // add the provider registry
        let mut provider_registry = ProviderRegistry::new();
        if let Some(path) = providers_path {
            let full_path = format!("{}/{}", config_dir, path);
            info!("ProviderConfig path: {}", full_path);

            let provider_configs = load_provider_config(full_path).await?;
            let resolved_providers = build_resolved_providers(&provider_configs)?;
            for provider in resolved_providers {
                info!(
                    "  Provider: {}-{} {}",
                    provider.id, provider.llm, provider.default_model
                );
                provider_registry.add_provider(provider);
            }
        }

        // load mcp servers
        let mut mcp_registry = MCPRegistry::new();

        if let Some(path) = mcp_servers_config_path {
            let full_path = format!("{}/{}", config_dir, path);
            info!("MCPServer config path: {}", full_path);
            let mcp_server_configs = load_mcp_config(full_path).await?;
            for server in mcp_server_configs {
                info!("McpServerConfig: {}", server.name);
                let mcp_server_config = match server.to_core_config() {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("Skipping MCP server '{}': {}", server.name, e);
                        continue;
                    }
                };
                let definitions = mcp_registry
                    .register_server(mcp_server_config.clone())
                    .await?;

                if server.enabled_tools.is_empty() {
                    // expose all
                    let tools: Vec<String> = definitions.iter().map(|t| t.name.clone()).collect();
                    info!("Tools: {:#?}", tools);

                    mcp_registry.add_definitions(&server.name, definitions);
                } else {
                    // expose only selected
                    let filtered: Vec<_> = definitions
                        .into_iter()
                        .filter(|d| {
                            // match by tool name (strip server prefix)
                            server.enabled_tools.iter().any(|t| d.name.ends_with(t))
                        })
                        .collect();
                    info!(
                        "McpServerSettings: {} filtered tools: {:?}",
                        server.name, filtered
                    );
                    mcp_registry.add_definitions(&server.name, filtered);
                }
            }
            info!("MCPServers configured");
        }

        let defs: Vec<String> = mcp_registry
            .definitions
            .iter()
            .map(|d| d.0.clone())
            .collect();
        info!("Mcp Definitions: {:?}", defs);

        let mut agent_registry = AgentRegistry::new();
        if let Some(path) = agents_config_path {
            let full_path = format!("{}/{}", config_dir, path);
            info!("AgentConfig path: {}", full_path);
            let agents = load_agents_config(config_dir.clone(), full_path).await?;
            for agent in agents {
                // validate — only borrows agent
                info!("  agent: {} Tools: {:#?}", agent.id, agent.tools);
                let missing_tool = agent.tools.iter().find(|tool_id| {
                    tool_registry.get_tool(tool_id).is_none()
                        && mcp_registry.get_tool(tool_id).is_none()
                });

                match missing_tool {
                    Some(tool_id) => {
                        error!(
                            "Agent '{}' references unknown tool '{}' — skipping",
                            agent.id, tool_id
                        );
                    }
                    None => {
                        info!("Registered agent: {}", agent.id);
                        agent_registry.register_agent(agent); // ← moved here, no borrow conflict
                    }
                }
            }
        }

        //firebase keys
        let keys = fetch_firebase_keys().await?;
        let firebase_keys = Arc::new(RwLock::new(FirebaseKeyCache {
            keys,
            fetched_at: std::time::Instant::now(),
        }));

        let agent_service = AgentService::from_registry(
            Arc::new(provider_registry),
            Arc::new(agent_registry),
            Arc::new(RwLock::new(tool_registry)),
            Arc::new(RwLock::new(mcp_registry)),
        );

        let conversation_service = match (mongo_uri, mongo_db) {
            (Some(mongo_uri), Some(mongo_db)) => {
                info!("Connecting to MongoDB: {}/{}", mongo_uri, mongo_db);
                let storage_manager = BootStorageManager::new(&mongo_uri, &mongo_db).await?;

                Some(Arc::new(ConversationService::new(
                    Arc::new(agent_service.clone()),
                    Arc::new(storage_manager),
                )))
            }
            _ => {
                info!("No MongoDB configured — storage disabled");
                None
            }
        };

        info!("Configuring boot state");
        Ok(BootState {
            agent_service: Arc::new(agent_service.clone()),
            conversation_service,
            chat_templates,
            firebase_keys,
            firebase_project_id,
        })
    }

    pub async fn serve<F, S, P, R>(
        self,
        addr: &str,
        extend_state: F,
        public: P,
        protected: R,
    ) -> Result<()>
    where
        F: FnOnce(BootState) -> S,
        S: Clone + Send + Sync + 'static,
        Arc<BootState>: FromRef<S>,
        P: FnOnce(Router<S>, S) -> Router<S>,
        R: FnOnce(Router<S>, S) -> Router<S>,
    {
        let cors = build_cors(&self.cors_origins);
        let boot_state = Arc::new(self.build().await?);
        let app_state = extend_state((*boot_state).clone());

        let public_router = public(Router::new(), app_state.clone());

        let protected_router = protected(Router::new(), app_state.clone());

        let app = Router::new()
            .merge(public_router)
            .merge(protected_router)
            .layer(cors)
            .with_state(app_state);

        let listener = TcpListener::bind(addr).await?;
        info!("Listening on {}", addr);
        axum::serve(listener, app).await?;

        Ok(())
    }
}

pub fn build_resolved_providers(
    provider_configs: &[ProviderConfig],
) -> Result<Vec<ResolvedProvider>> {
    let mut resolved_providers: Vec<ResolvedProvider> = Vec::new();

    for config in provider_configs {
        if !config.enabled {
            continue;
        }

        let api_key = match &config.api_key_env {
            Some(env_var) => {
                let key = std::env::var(env_var).with_context(|| {
                    format!(
                        "Provider '{}' requires env var '{}' which is not set",
                        config.id, env_var
                    )
                })?;
                Some(key)
            }
            None => None,
        };

        let base_url = match &config.base_url_env {
            Some(env_var) => {
                let url = std::env::var(env_var).with_context(|| {
                    format!(
                        "Provider '{}' requires env var '{}' which is not set",
                        config.id, env_var
                    )
                })?;
                Some(url)
            }
            None => None,
        };

        let cconfig = config.clone();
        resolved_providers.push(ResolvedProvider {
            id: cconfig.id,
            llm: cconfig.llm,
            api_key,
            base_url,
            models: cconfig.models,
            default_model: cconfig.default_model,
        });
    }

    Ok(resolved_providers)
}

pub fn build_cors(origins: &[String]) -> CorsLayer {
    if origins.is_empty() {
        // allow all — development only
        return CorsLayer::permissive();
    }

    let allowed: Vec<HeaderValue> = origins.iter().filter_map(|o| o.parse().ok()).collect();

    CorsLayer::new()
        .allow_origin(allowed)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
        .allow_credentials(true)
}
