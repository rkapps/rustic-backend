use crate::services::config::agent::{AgentConfig, ExecutionType};

/// In-memory store of all [`AgentConfig`] entries loaded at startup.
///
/// Agents are registered once from `agents.json` and then looked up by ID at
/// request time. The registry is read-only after startup.
#[derive(Clone, Debug)]
pub struct AgentRegistry {
    pub agents: Vec<AgentConfig>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRegistry {
    /// Create an empty registry.
    pub fn new() -> AgentRegistry {
        Self { agents: Vec::new() }
    }

    /// Add an agent configuration to the registry.
    pub fn register_agent(&mut self, agent: AgentConfig) {
        self.agents.push(agent);
    }

    /// Return all registered agent configurations.
    pub fn all(&self) -> &Vec<AgentConfig> {
        &self.agents
    }

    /// Return agents that are visible in the public catalog.
    ///
    /// An agent appears in the catalog when it is `standalone` and its
    /// `execution` type is `SingleAgent` or `Pipeline` — these are the
    /// agents users can start conversations with directly.
    pub fn catalog(&self) -> Vec<AgentConfig> {
        self.agents
            .iter()
            .filter(|a| {
                a.standalone
                    && matches!(
                        a.execution,
                        ExecutionType::SingleAgent | ExecutionType::Pipeline
                    )
            })
            .cloned()
            .collect()
    }

    /// Look up an agent by its unique ID. Returns `None` if not found.
    pub fn find(&self, id: &str) -> Option<&AgentConfig> {
        self.agents.iter().find(|a| a.id == id)
    }

    /// Return the sub-agent configs listed in `agent_id`'s pipeline `available_agents`.
    ///
    /// Returns an empty vec if the agent is not a pipeline or has no available agents configured.
    pub fn sub_agents(&self, agent_id: &str) -> Vec<AgentConfig> {
        let available_ids: Vec<&str> = self
            .find(agent_id)
            .and_then(|a| a.pipeline.as_ref())
            .map(|p| p.available_agents.iter().map(|a| a.id.as_str()).collect())
            .unwrap_or_default();

        self.agents
            .iter()
            .filter(|a| available_ids.contains(&a.id.as_str()))
            .cloned()
            .collect()
    }
}
