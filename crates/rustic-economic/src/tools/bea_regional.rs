use crate::{
    core::bea::get_bea_regional,
    storage::mongo::reader::EconomicMongoStorageReader,
};
use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, info};

#[derive(Debug)]
pub struct BeaRegionalDataTool {
    reader: Arc<EconomicMongoStorageReader>,
}

impl BeaRegionalDataTool {
    pub fn new(reader: Arc<EconomicMongoStorageReader>) -> Self {
        Self { reader }
    }
}

#[async_trait]
impl Tool for BeaRegionalDataTool {
    fn name(&self) -> String {
        "bea_regional_data".to_string()
    }

    fn description(&self) -> String {
        "Fetch BEA state-level regional economic data. Use code and line_codes from the taxonomy. Tables: CAINC1 (personal income by state), CAINC5N (earnings by industry by state), CAGDP1 (GDP by state).".to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "BEA regional table code e.g. CAINC1, CAINC5N, CAGDP1"
                },
                "line_codes": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Line codes to fetch e.g. [\"1\"], [\"700\", \"702\", \"521\"]"
                },
                "year": {
                    "type": "string",
                    "description": "Year or range. Examples: 2025,2024,2023,2022, LAST5, LAST3, LAST2"
                },
                "geo_type": {
                    "type": "string",
                    "enum": ["US", "STATE", "COUNTY"],
                    "description": "Filter by geography type"
                },
                "geo_fips": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "FIPS codes for filtering. States: 06000=California, 48000=Texas, 04000=Arizona. Counties: 06075=San Francisco, 04013=Maricopa. When geo_type=county and a state FIPS is provided, returns all counties in that state."
                },
                "state_prefix": {
                    "type": "string",
                    "description": "2-digit state code to return all counties within that state. 06=California, 48=Texas, 04=Arizona. Use with geo_type=county."
                }                    
            },
            "required": ["code", "line_codes", "year"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let code = params["code"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("code required"))?;

        let line_codes: Vec<String> = params["line_codes"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("line_codes required"))?
            .iter()
            // Extract the inner string slice instead of converting the JSON block to a string
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        let year = params["year"].as_str().unwrap_or("LAST5");

        let geo_fips: Vec<String> = params["geo_fips"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str()) // Safely extracts Option<&str>
                    .map(String::from) // Converts &str to owned String
                    .collect()
            })
            .unwrap_or_default(); // Returns empty Vec if "geo_fips" is missing or not an array

        let geo_type = params["geo_type"].as_str();
        let state_prefix = params["state_prefix"].as_str();

        info!(
            target: "economic-tool",
            "bea regional code {} line_codes: {:?} geo_fips: {:?} geo_type: {:?} state_prefix: {:?} year: {}",
            code,
            line_codes,
            geo_fips,
            geo_type,
            state_prefix,
            year,
        );

        let rows = get_bea_regional(
            self.reader.clone(),
            code,
            line_codes,
            geo_fips,
            geo_type,
            state_prefix,
            year,
        )
        .await?;
        debug!(
            target: "economic-tool",
            "Bea: {}", rows.len()
        );

        Ok(json!({
            "bea_regional":   if rows.is_empty() {Value::Null} else {json!(rows)},
            "provider":   "bea"
        }))
    }
}
