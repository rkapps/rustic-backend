use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tracing::{debug, info, trace};

use tokio::sync::RwLock;

use crate::{
    Agent,
    agents::{
        domain::AgentInput,
        runner::{PipeLineAgent, Runnable, SingleAgent},
    },
    client::{llm::LlmClient, preset::Preset, provider::Provider},
    services::{
        builder::AgentBuilder,
        config::agent::{AgentConfig, CompletionStrategy, ExecutionType},
        registry::{agent::AgentRegistry, provider::ProviderRegistry},
    },
    tools::{mcp::MCPRegistry, tool::ToolRegistry},
};

/// Central service for constructing [`Agent`] and [`Runnable`] instances at request time.
///
/// `AgentService` holds shared references to all registries and a cache of
/// live [`LlmClient`] instances keyed by `"{provider}:{model}"`. Clients are
/// created on first use and reused across subsequent builds, avoiding repeated
/// initialisation overhead.
///
/// ## Building agents
///
/// | Goal | Method |
/// |------|--------|
/// | Ad-hoc chat agent (no registry entry) | [`build_chat_agent`](Self::build_chat_agent) |
/// | Registered agent by ID | [`build_agent_for_id`](Self::build_agent_for_id) |
/// | Runnable (handles both single and pipeline topologies) | [`build_runnable`](Self::build_runnable) |
/// | Low-level builder | [`builder`](Self::builder) |
///
/// Pipeline topologies are built recursively by [`build_runnable_agent`](Self::build_runnable_agent),
/// which detects cycles via a `visited` set and wraps leaf agents in [`SingleAgent`] and
/// orchestrators in [`PipeLineAgent`].
#[derive(Clone)]
pub struct AgentService {
    /// Live LLM client cache keyed by `"{LLM}:{model}"`.
    pub clients: Arc<RwLock<HashMap<String, Arc<dyn LlmClient>>>>,
    pub agent_registry: Arc<AgentRegistry>,
    pub provider_registry: Arc<ProviderRegistry>,
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
    pub mcp_registry: Arc<RwLock<MCPRegistry>>,
}

impl AgentService {
    /// Construct the service from pre-built registries.
    ///
    /// The client cache starts empty and is populated lazily as agents are built.
    pub fn from_registry(
        provider_registry: Arc<ProviderRegistry>,
        agent_registry: Arc<AgentRegistry>,
        tool_registry: Arc<RwLock<ToolRegistry>>,
        mcp_registry: Arc<RwLock<MCPRegistry>>,
    ) -> Self {
        Self {
            agent_registry,
            provider_registry,
            clients: Arc::new(RwLock::new(HashMap::new())),
            tool_registry,
            mcp_registry,
        }
    }

    /// Return a new [`AgentBuilder`] borrowing from this service.
    pub fn builder(&self, id: &str) -> AgentBuilder<'_> {
        AgentBuilder::new(self, id)
    }

    /// Build a general-purpose chat agent for the given provider and model.
    ///
    /// Intended for ad-hoc conversations that do not correspond to a registered
    /// [`AgentConfig`]. The agent is assigned no ID and no tools. Preset defaults
    /// to [`Preset::Local`] for local providers and [`Preset::Balanced`] otherwise.
    /// `system_prompt` defaults to an empty string when `None`.
    pub async fn build_chat_agent(
        &self,
        llm: &str,
        model: &str,
        system_prompt: &Option<String>,
        strategy: &CompletionStrategy,
    ) -> Result<Agent> {
        let provider = self.resolve_provider("", llm, Some(model))?;

        let preset = match &provider {
            Provider::Local { .. } => Preset::Local,
            _ => Preset::Balanced,
        };
        let system_prompt = system_prompt.clone().unwrap_or_default();
        debug!(
            "Conversation strategy: {:#?} Preset: {:?}",
            preset, strategy
        );
        debug!("System Prompt: {}", system_prompt);

        // chat does not have an id
        let id = String::new();
        let agent = self
            .builder(id.as_str())
            .with_strategy(strategy.clone())
            .with_system_prompt(system_prompt)
            .with_preset(preset)
            .with_provider(provider)
            .await?
            .with_filtered_mcp(MCPRegistry::new())
            .build()
            .await?;

        Ok(agent)
    }

    /// Build a single [`Agent`] from a registered [`AgentConfig`].
    ///
    /// Preset resolution order (first wins): caller-supplied `preset` → agent config →
    /// parent agent config → provider default (`Local` or `Balanced`).
    ///
    /// Tool and MCP registries are filtered down to only the IDs listed in the agent's
    /// config; tools not present in the global registry are silently skipped.
    ///
    /// `system_prompt` overrides the config's own prompt; pass `None` to fall back to
    /// an empty string (the config prompt is not used automatically — callers should
    /// supply it from the config when needed).
    ///
    /// Returns an error if `agent_id` is not registered or the provider cannot be resolved.
    pub async fn build_agent_for_id(
        &self,
        parent_agent_id: Option<String>,
        agent_id: &str,
        llm: &str,
        model: &str,
        system_prompt: Option<String>,
        strategy: &CompletionStrategy,
        preset: Option<Preset>,
    ) -> Result<Agent> {
        let agent_config = self.find_agent_config(agent_id).await?;

        let provider = self.resolve_provider(agent_id, llm, Some(model))?;

        // default the system prompt from agent config
        let system_prompt = system_prompt.or(Some(agent_config.system_prompt));

        let mut dpreset = match &provider {
            Provider::Local { .. } => Preset::Local,
            _ => Preset::Balanced,
        };

        if let Some(preset) = preset {
            dpreset = preset;
        } else {
            // override from agent
            if let Some(agent_preset) = agent_config.preset {
                dpreset = agent_preset;
            } else {
                // override from parent
                if let Some(parent_agent_id) = parent_agent_id {
                    let parent_config = self.find_agent_config(&parent_agent_id).await?;
                    if let Some(parent_preset) = parent_config.preset {
                        dpreset = parent_preset;
                    }
                }
            }
        }

        let tool_registry = {
            let global = self.tool_registry.read().await;
            let mut filtered = ToolRegistry::new();
            for tool_id in &agent_config.tools {
                trace!("Tool: {:?}", tool_id);
                if let Some(tool) = global.get_tool(tool_id) {
                    filtered.register_tool_arc(tool);
                }
            }
            filtered
        };

        let mcp_registry = {
            let global = self.mcp_registry.read().await;

            if agent_config.mcp_tools.is_empty() {
                MCPRegistry::new()
            } else {
                let mut filtered = MCPRegistry::new();
                for tool_id in &agent_config.mcp_tools {
                    if let Some(def) = global.get_tool(tool_id) {
                        filtered.definitions.insert(tool_id.clone(), def);
                    }
                }

                // clone the registry
                filtered.registry = global.registry.clone();
                filtered
            }
        };

        // info!("System Prompt: {}", agent_config.system_prompt);
        info!(
            strategy= ?strategy,
            preset= ?dpreset,
            tools= ?tool_registry.get_tools().len(),
            // system_prompt= ?agent_config.system_prompt,
           "Agent: {} - build_agent_handle", agent_config.id
        );

        let agent = self
            .builder(&agent_config.id)
            .with_strategy(strategy.clone())
            .with_system_prompt(system_prompt.unwrap_or_default())
            .with_tools(tool_registry.get_tools())
            .with_filtered_mcp(mcp_registry)
            .with_preset(dpreset)
            .with_provider(provider)
            .await?
            .build()
            .await?;

        Ok(agent)
    }

    /// Look up an [`AgentConfig`] by ID.  Returns an error if not registered.
    pub async fn find_agent_config(&self, agent_id: &str) -> Result<AgentConfig> {
        self.agent_registry
            .find(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", agent_id))
            .cloned()
    }

    /// Build a [`Runnable`] from an [`AgentInput`], handling both single-agent and
    /// pipeline topologies.
    ///
    /// Entry point for request handling. Delegates to [`build_runnable_agent`](Self::build_runnable_agent)
    /// with a fresh cycle-detection set.
    pub async fn build_runnable(&self, input: &AgentInput) -> Result<Arc<dyn Runnable>> {
        info!("Agent: {} - Build Runnable", input.agent_id);

        self.build_runnable_agent(input, &mut HashSet::new()).await
    }

    /// Recursively build a [`Runnable`] for `input`, dispatching on [`ExecutionType`]:
    ///
    /// - `SingleAgent` / `PipelineAgent` → wrapped in [`SingleAgent`].
    /// - `Pipeline` → each sub-agent in the config's `available_agents` list is built
    ///   recursively and collected into a [`PipeLineAgent`]. Each sub-agent's strategy
    ///   and system prompt come from its own config; LLM and model are inherited from
    ///   the top-level input.
    ///
    /// `visited` tracks agent IDs seen in the current recursion path; a repeated ID
    /// causes an immediate error to break cycles.
    pub async fn build_runnable_agent(
        &self,
        input: &AgentInput,
        visited: &mut HashSet<String>,
    ) -> Result<Arc<dyn Runnable>> {
        let config = self.find_agent_config(&input.agent_id).await?;
        debug!(
            execution= ?config.execution,
           "Agent: {} - Build Runnable Agent", config.id
        );

        if !visited.insert(config.id.to_string()) {
            return Err(anyhow::anyhow!(
                "Circular pipeline reference detected: {}",
                config.id
            ));
        }

        let agent = self
            .build_agent_for_id(
                Some(input.agent_id.clone()),
                &input.agent_id,
                &input.llm,
                &input.model,
                input.system_prompt.clone(),
                &input.strategy,
                input.preset.clone(),
            )
            .await?;

        match config.execution {
            ExecutionType::SingleAgent | ExecutionType::PipelineAgent => {
                Ok(Arc::new(SingleAgent::new(agent, input.strategy.clone())))
            }
            ExecutionType::Pipeline => {
                let pipeline_config = config.pipeline.expect(&format!(
                    "Pipeline agent {} should have sub agents",
                    input.agent_id
                ));
                let mut subs = Vec::new();
                for sub_agent in pipeline_config.available_agents {
                    let config = self.find_agent_config(&sub_agent.id).await?;
                    let strategy = config.get_strategy();
                    let sub_input = AgentInput::new(
                        sub_agent.id.clone(),
                        input.llm.clone(),
                        input.model.clone(),
                        Some(config.system_prompt),
                        strategy,
                        sub_agent.preset.clone(),
                    );
                    let sub_agent = Box::pin(self.build_runnable_agent(&sub_input, visited))
                        .await
                        .context(format!("Sub Agent error: {}", sub_agent.id))?;
                    subs.push(sub_agent);
                }
                let pipeline = PipeLineAgent::new(agent, input.strategy.clone(), subs);
                Ok(Arc::new(pipeline) as Arc<dyn Runnable>)
            }
        }
    }

    /// Resolve a [`Provider`] enum variant from a provider ID and optional model override.
    ///
    /// Falls back to the provider's `default_model` when `model` is `None`.
    /// Unknown provider IDs are treated as local/Ollama servers and require `base_url` to be set.
    ///
    /// Returns an error if the provider is not registered, the required API key is missing,
    /// or a local provider has no `base_url` configured.
    pub fn resolve_provider(
        &self,
        agent_id: &str,
        id: &str,
        model: Option<&str>,
    ) -> anyhow::Result<Provider> {
        debug!(
            llm= %id,
            model= ?model,
           "Agent: {} - Resolve Provider", agent_id
        );

        let provider = self
            .provider_registry
            .find(id)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' not found", id))?;

        let model = model.unwrap_or(&provider.default_model);

        match id {
            "openai" => Ok(Provider::openai(
                self.provider_registry
                    .get_api_key("openai")
                    .ok_or_else(|| anyhow::anyhow!("OpenAI API key not configured"))?,
                model,
            )),
            "gemini" => Ok(Provider::gemini(
                self.provider_registry
                    .get_api_key("gemini")
                    .ok_or_else(|| anyhow::anyhow!("Gemini API key not configured"))?,
                model,
            )),
            "anthropic" => Ok(Provider::anthropic(
                provider
                    .api_key
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Anthropic API key not configured"))?,
                model,
            )),
            _ => {
                let base_url = provider.base_url.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("Provider '{}' has no base_url configured", id)
                })?;

                Ok(Provider::local(model, base_url))
            }
        }
    }
}
