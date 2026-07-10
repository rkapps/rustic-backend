use crate::domain::config::EconomicConfig;
use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde_json::{Value, json};
use tracing::{info};

#[derive(Debug)]
pub struct EconomicTaxonomyTool {
    config: EconomicConfig
}
impl EconomicTaxonomyTool {
    pub fn new(config: EconomicConfig) -> EconomicTaxonomyTool {
        Self { config }
    }
}

#[async_trait]
impl Tool for EconomicTaxonomyTool {
    fn name(&self) -> String {
        "economic_taxonomy".to_string()
    }

    fn description(&self) -> String {
        "Returns the economic data available for the application. This includes the fred series, bea nipa table names, bea regional codes and census variables"
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
        })
    }

    async fn execute(&self, _value: serde_json::Value) -> Result<Value> {
        info!(
            target: "economic-tool",
            "Economic Config: {:?}", self.config
        );
        Ok(json!({"taxonomy": &self.config }))
    }
}
