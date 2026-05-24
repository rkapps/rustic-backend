use anyhow::{Context, Result};
use rustic_core::Tool;
use std::sync::Arc;

use crate::{
    MCPRegistry, ToolRegistry,
    agents::Agent,
    client::{
        llm::LlmClient, mcp::MCPServerAdapter, preset::Preset, provider::Provider,
        request::ReasoningEffort,
    },
    providers::{
        anthropic::{self, completion::AnthropicClient},
        gemini::{self, completion::GeminiClient},
        local::completion::LocalClient,
        openai::{self, completion::OpenAIClient},
    },
    services::agent::AgentService,
    tools::mcp::MCPServerSetting,
};

const MODEL_TEMPERATURE: f32 = 0.5;
const MODEL_MAX_TOKENS: i32 = 5000;

/// Fluent builder for constructing a configured [`Agent`].
///
/// Obtained via [`AgentService::builder`]. `AgentBuilder` borrows `AgentService`
/// immutably for its lifetime, so multiple builders can coexist concurrently.
/// Shared state (client cache, tool/MCP registries) is mutated safely through
/// the service's interior `RwLock`s.
///
/// The minimum viable call sequence is:
/// ```text
/// service.builder()
///     .with_provider(provider).await?
///     .with_preset(Preset::Balanced)
///     .build().await?
/// ```
pub struct AgentBuilder<'a> {
    id: String,
    service: &'a AgentService,
    llm: Option<String>,
    model: Option<String>,
    system_prompt: Option<String>,
    client: Option<Arc<dyn LlmClient>>,
    temperature: Option<f32>,
    max_tokens: Option<i32>,
    enable_cache: bool,
    reasoning_effort: ReasoningEffort,
    /// Tools accumulated via `with_tool*` — registered into the shared registry on `build`.
    pending_tools: Vec<Arc<dyn Tool>>,
    filtered_mcp: Option<MCPRegistry>,
}

impl<'a> AgentBuilder<'a> {
    /// Create a builder bound to `service`. Prefer [`AgentService::builder`] over calling this directly.
    pub fn new(service: &'a AgentService, id: &str) -> Self {
        Self {
            id: id.to_string(),
            service,
            llm: None,
            model: None,
            system_prompt: None,
            client: None,
            temperature: None,
            max_tokens: None,
            enable_cache: false,
            reasoning_effort: ReasoningEffort::None,
            pending_tools: Vec::new(),
            filtered_mcp: None,
        }
    }

    /// Set the system prompt prepended before every conversation.
    pub fn with_system_prompt(mut self, system_prompt: String) -> Self {
        self.system_prompt = Some(system_prompt);
        self
    }

    /// Select provider via enum — cleanest API for library consumers
    pub async fn with_provider(self, provider: Provider) -> Result<Self> {
        match provider {
            Provider::OpenAI { api_key, model } => self.with_openai(&api_key, &model).await,
            Provider::Gemini { api_key, model } => self.with_gemini(&api_key, &model).await,
            Provider::Anthropic { api_key, model } => self.with_anthropic(&api_key, &model).await,
            Provider::Local { model, base_url } => {
                self.with_local("local", &model, &base_url).await
            }
        }
    }

    /// Configure the builder to use Anthropic, reusing a cached client if one exists for this model.
    pub async fn with_anthropic(mut self, api_key: &str, model: &str) -> Result<Self> {
        let mut clients = self.service.clients.write().await;
        self.llm = Some(anthropic::LLM.to_string());
        self.model = Some(model.to_string());
        let client_key = format! {"{}:{}", anthropic::LLM, model};
        let client = clients
            .entry(client_key)
            .or_insert(self.anthropic_client(api_key)?);
        self.client = Some(client.clone());
        Ok(self)
    }

    /// Configure the builder to use OpenAI, reusing a cached client if one exists for this model.
    pub async fn with_openai(mut self, api_key: &str, model: &str) -> Result<Self> {
        let mut clients = self.service.clients.write().await;
        self.llm = Some(openai::LLM.to_string());
        self.model = Some(model.to_string());
        let client_key = format! {"{}:{}", openai::LLM, model};
        let client = clients
            .entry(client_key)
            .or_insert(self.openai_client(api_key)?);
        self.client = Some(client.clone());
        Ok(self)
    }

    /// Configure the builder to use Gemini, reusing a cached client if one exists for this model.
    pub async fn with_gemini(mut self, api_key: &str, model: &str) -> Result<Self> {
        let mut clients = self.service.clients.write().await;
        self.llm = Some(gemini::LLM.to_string());
        self.model = Some(model.to_string());
        let client_key = format! {"{}:{}", gemini::LLM, model};
        let client = clients
            .entry(client_key)
            .or_insert(self.gemini_client(api_key)?);
        self.client = Some(client.clone());
        Ok(self)
    }

    /// Configure the builder to use a local Anthropic-compatible server, reusing a cached client if one exists.
    pub async fn with_local(mut self, llm: &str, model: &str, base_url: &str) -> Result<Self> {
        let mut clients = self.service.clients.write().await;
        self.llm = Some(llm.to_string());
        self.model = Some(model.to_string());
        let client_key = format! {"{}:{}", llm, model};
        let client = clients
            .entry(client_key)
            .or_insert(self.local_client(base_url)?);
        self.client = Some(client.clone());
        Ok(self)
    }

    /// Add a statically-typed tool. Stored in `pending_tools` until `build` registers it.
    pub fn with_tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.pending_tools.push(Arc::new(tool));
        self
    }

    /// Add a trait-object tool. Stored in `pending_tools` until `build` registers it.
    pub fn with_tool_boxed(mut self, tool: Arc<dyn Tool>) -> Self {
        self.pending_tools.push(tool);
        self
    }
    /// Add multiple tools in one call.
    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.pending_tools.extend(tools);
        self
    }

    /// Register an MCP server into the shared [`MCPRegistry`] using a custom adapter.
    pub async fn with_mcp_registry<T: MCPServerAdapter + 'static>(
        self,
        setting: MCPServerSetting,
        adapter: T,
    ) -> Result<Self> {
        let mut registry = self.service.mcp_registry.write().await;
        let _ = registry
            .register_server_with_adapter(setting, Box::new(adapter))
            .await?;
        Ok(self)
    }

    pub fn with_filtered_mcp(mut self, registry: MCPRegistry) -> Self {
        self.filtered_mcp = Some(registry);
        self
    }

    /// Register a single tool from an already-registered MCP server into the shared registry.
    pub async fn with_mcp_tool(self, server_name: &str, tool_name: &str) -> Result<Self> {
        let mut registry = self.service.mcp_registry.write().await;
        let _ = registry.register_tool(server_name, tool_name).await?;
        Ok(self)
    }

    /// Apply a named preset, setting temperature, max tokens, cache, and reasoning effort together.
    pub fn with_preset(self, preset: Preset) -> Self {
        match preset {
            Preset::Fast => self.with_preset_fast(),
            Preset::Balanced => self.with_preset_balanced(),
            Preset::Precise => self.with_preset_precise(),
            Preset::Thorough => self.with_preset_thorough(),
            Preset::Local => self.with_preset_local(),
        }
    }

    /// `Fast` — no cache, no reasoning, 0.7 temperature, 1 024 max tokens.
    pub fn with_preset_fast(mut self) -> Self {
        self.enable_cache = false;
        self.reasoning_effort = ReasoningEffort::None;
        self.with_temperature(0.7).with_max_tokens(1024)
    }

    /// `Balanced` — cache enabled, medium reasoning, 0.5 temperature, 2 048 max tokens.
    pub fn with_preset_balanced(mut self) -> Self {
        self.enable_cache = true;
        self.reasoning_effort = ReasoningEffort::Medium;
        self.with_temperature(0.5).with_max_tokens(2048)
    }

    /// `Precise` — cache enabled, high reasoning, 0.2 temperature, 4 096 max tokens.
    pub fn with_preset_precise(mut self) -> Self {
        self.enable_cache = true;
        self.reasoning_effort = ReasoningEffort::High;
        self.with_temperature(0.2).with_max_tokens(4096)
    }

    /// `Thorough` — cache enabled, high reasoning, 0.1 temperature, 8 192 max tokens.
    pub fn with_preset_thorough(mut self) -> Self {
        self.enable_cache = true;
        self.reasoning_effort = ReasoningEffort::High;
        self.with_temperature(0.1).with_max_tokens(8192)
    }

    /// `Local` — no cache, no reasoning, 0.7 temperature, 4 096 max tokens. Tuned for Ollama.
    pub fn with_preset_local(mut self) -> Self {
        self.enable_cache = false;
        self.reasoning_effort = ReasoningEffort::None;
        self.with_temperature(0.7).with_max_tokens(4096)
    }
    /// Override the sampling temperature directly, bypassing preset defaults.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Override the max output token limit directly, bypassing preset defaults.
    pub fn with_max_tokens(mut self, max_tokens: i32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    fn anthropic_client(&self, api_key: &str) -> Result<Arc<dyn LlmClient>> {
        let client = AnthropicClient::new(api_key.to_string())
            .with_context(|| anyhow::anyhow!("Error creating Anthropic client"))?;
        Ok(Arc::new(client))
    }

    fn openai_client(&self, api_key: &str) -> Result<Arc<dyn LlmClient>> {
        let client = OpenAIClient::new(api_key.to_string())
            .with_context(|| anyhow::anyhow!("Error creating Anthropic client"))?;
        Ok(Arc::new(client))
    }

    fn gemini_client(&self, api_key: &str) -> Result<Arc<dyn LlmClient>> {
        let client = GeminiClient::new(api_key.to_string())
            .with_context(|| anyhow::anyhow!("Error creating Anthropic client"))?;
        Ok(Arc::new(client))
    }

    fn local_client(&self, base_url: &str) -> Result<Arc<dyn LlmClient>> {
        let client = LocalClient::anthropic_compat(base_url.to_string())
            .with_context(|| anyhow::anyhow!("Error creating Local Anthropic Compatible client"))?;
        Ok(Arc::new(client))
    }

    /// Consume the builder and produce a fully configured [`Agent`].
    ///
    /// Any tools accumulated via `with_tool*` are registered into the shared
    /// `ToolRegistry` before the agent takes a snapshot read of it. The write
    /// lock is dropped before the read lock is acquired.
    ///
    /// Returns an error if `client`, `llm`, or `model` were never set.
    pub async fn build(self) -> Result<Agent> {
        let client = self
            .client
            .ok_or_else(|| anyhow::anyhow!("Client is required"))?;
        let llm = self.llm.ok_or_else(|| anyhow::anyhow!("LLM is required"))?;
        let model = self
            .model
            .ok_or_else(|| anyhow::anyhow!("Model is required"))?;
        let temperature: f32 = self.temperature.unwrap_or(MODEL_TEMPERATURE);
        let max_tokens = self.max_tokens.unwrap_or(MODEL_MAX_TOKENS);
        let system_prompt = self.system_prompt;

        let tool_registry = {
            let mut registry = ToolRegistry::new();
            for tool in self.pending_tools {
                registry.register_tool_boxed(tool);
            }
            Arc::new(registry)
        };
        // let mcp_tool_guard = self.service.mcp_registry.read().await;
        // let mcp_registry = Arc::new(mcp_tool_guard.clone());

        // use filtered MCP if provided, otherwise use full shared registry
        let mcp_registry = if let Some(filtered) = self.filtered_mcp {
            Arc::new(filtered)
        } else {
            let mcp_tool_guard = self.service.mcp_registry.read().await;
            Arc::new(mcp_tool_guard.clone())
        };

        let reasoning_effort = self.reasoning_effort;
        let enable_cache = self.enable_cache;

        Ok(Agent {
            id: self.id,
            llm,
            model,
            system_prompt,
            client,
            temperature,
            max_tokens,
            reasoning_effort,
            enable_cache,
            tool_registry,
            mcp_registry,
        })
    }
}
