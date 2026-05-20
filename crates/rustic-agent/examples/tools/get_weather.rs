//! Stub weather tool used to demonstrate custom [`Tool`] registration.

use anyhow::Result;
use async_trait::async_trait;
use rustic_agent::client::tools::Tool;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Example [`Tool`] that pretends to fetch the current temperature for a city.
///
/// `execute` always returns `20` — replace with a real weather API call in
/// production use.
#[derive(Debug, Serialize, Clone)]
pub struct GetWeatherTool {}

#[derive(Deserialize)]
struct Weather {
    #[serde(rename = "location")]
    _location: String,
}

#[async_trait]
impl Tool for GetWeatherTool {
    fn name(&self) -> String {
        "get_weather".to_string()
    }

    fn description(&self) -> String {
        "Get current temperatur for a given location".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        let parameters = json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City and country e.g. Bogotá, Colombia"
                }
            },
            "required": ["location"],
            "additionalProperties": false
        });
        parameters
    }

    async fn execute(&self, value: serde_json::Value) -> Result<Value> {
        let _: Weather = match serde_json::from_value(value.clone()) {
            Ok(c) => c,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Error dezerializing arguments {:#?}: {}",
                    value,
                    e
                ));
            }
        };
        let result = "20";
        let newvalue = serde_json::from_str(result)?;
        println!("Argements: {:#?}", newvalue);
        Ok(newvalue)
    }
}
