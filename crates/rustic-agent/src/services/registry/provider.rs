use crate::{client::llm::LlmProvider, services::config::provider::ResolvedProvider};

/// In-memory store of all [`ResolvedProvider`] entries loaded at startup.
///
/// Providers are registered once from `providers.json` with environment variables
/// already resolved to their values. The registry is read-only after startup and
/// consulted by [`AgentService`](super::super::agent::AgentService) on every agent build.
#[derive(Clone)]
pub struct ProviderRegistry {
    providers: Vec<ResolvedProvider>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    /// Create an empty registry.
    pub fn new() -> ProviderRegistry {
        Self {
            providers: Vec::new(),
        }
    }

    /// Add a resolved provider to the registry.
    pub fn add_provider(&mut self, provider: ResolvedProvider) {
        self.providers.push(provider);
    }

    /// Return all registered providers.
    pub fn all(&self) -> &Vec<ResolvedProvider> {
        &self.providers
    }

    pub fn llm_providers(&self) -> Vec<LlmProvider> {
        self.all()
            .iter()
            .map(|p| {
                let rp = p.clone();
                let models = rp.models.iter().map(|m| m.id.clone()).collect();
                LlmProvider {
                    default_model: rp.default_model,
                    id: rp.id,
                    llm: rp.llm,
                    models,
                }
            })
            .collect()
    }

    /// Return the resolved API key for `provider_id`, or `None` if not configured.
    pub fn get_api_key(&self, provider_id: &str) -> Option<&str> {
        self.providers
            .iter()
            .find(|p| p.id == provider_id)
            .and_then(|p| p.api_key.as_deref())
    }

    /// Return the resolved base URL for `provider_id`, or `None` if not configured.
    pub fn get_base_url(&self, provider_id: &str) -> Option<&str> {
        self.providers
            .iter()
            .find(|p| p.id == provider_id)
            .and_then(|p| p.base_url.as_deref())
    }

    /// Look up a provider by its unique ID. Returns `None` if not found.
    pub fn find(&self, provider_id: &str) -> Option<&ResolvedProvider> {
        self.providers.iter().find(|p| p.id == provider_id)
    }
}
