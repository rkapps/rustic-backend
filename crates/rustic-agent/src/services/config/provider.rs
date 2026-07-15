use serde::{Deserialize, Serialize};

/// Raw provider entry as deserialised from `providers.json`.
///
/// API keys and base URLs are referenced by environment variable name rather than
/// stored directly. Call `ProviderConfig → ResolvedProvider` at startup to read the
/// actual values from the environment.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderConfig {
    /// Unique provider identifier (e.g. `"anthropic"`, `"openai"`, `"gemini"`).
    pub id: String,
    /// Provider label forwarded to the agent (e.g. `"Anthropic"`).
    pub llm: String,
    /// When `false` the provider is skipped during startup.
    pub enabled: bool,
    /// Name of the environment variable holding the API key. `None` for local/unauthenticated providers.
    pub api_key_env: Option<String>,
    /// Name of the environment variable holding the base URL override. `None` for hosted providers.
    pub base_url_env: Option<String>,
    pub models: Vec<ModelConfig>,
    pub default_model: String,
}

/// A provider with environment variables already resolved to their values.
///
/// This is what [`ProviderRegistry`](super::super::registry::provider::ProviderRegistry) stores
/// after startup resolves `api_key_env` and `base_url_env`.
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    pub id: String,
    pub llm: String,
    /// Resolved API key value; `None` for providers that don't require authentication.
    pub api_key: Option<String>,
    /// Resolved base URL; `None` for providers that use their standard endpoint.
    pub base_url: Option<String>,
    pub models: Vec<ModelConfig>,
    pub default_model: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ModelConfig {
    pub id: String,
    /// When `false` the config is skipped during startup.
    pub enabled: bool,
    pub input_cost_per_1k: f64,
    pub cached_read_cost_per_1k: f64,
    pub cached_write_cost_per_1k: f64,
    pub output_cost_per_1k: f64,
}

impl ModelConfig {
    pub fn from_id(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Default::default()
        }
    }
}
