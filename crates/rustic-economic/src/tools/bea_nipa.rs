use crate::{core::bea::get_bea_nipa, storage::mongo::reader::EconomicMongoStorageReader};
use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, info};

#[derive(Debug)]
pub struct BeaNipaDataTool {
    reader: Arc<EconomicMongoStorageReader>,
}

impl BeaNipaDataTool {
    pub fn new(reader: Arc<EconomicMongoStorageReader>) -> Self {
        Self { reader }
    }
}

#[async_trait]
impl Tool for BeaNipaDataTool {
    fn name(&self) -> String {
        "bea_nipa_data".to_string()
    }

    fn description(&self) -> String {
        "Fetch BEA NIPA national spending and income data. Use table_name and series_codes from the taxonomy. Tables: T20100 (personal income and outlays), T20305 (PCE by product type).".to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
               "table_name": {
                    "type": "string",
                    "description": "NIPA table name: T20100 or T20305"
                },
                "series_codes": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Series codes from taxonomy e.g. [\"DPCERC\", \"A067RC\"]"
                },
                "frequency": {
                    "type": "string",
                    "enum": ["A", "Q"],
                    "description": "A = Annual, Q = Quarterly. Default: A"
                },
                "year": {
                    "type": "string",
                    "description": "Year or range. Examples: 2024, 2024,2023,2022, LAST5, LAST3, LAST2"
                },
            },
            "required": ["table_name", "series_codes"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let table_name = params["table_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("table_name required"))?;

        let year = params["year"].as_str().unwrap_or("LAST5");
        let series_codes: Vec<String> = params["series_codes"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("series_codes required"))?
            .iter()
            // Extract the inner string slice instead of converting the JSON block to a string
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        info!(
            target: "economic-tool",
            "Bea Nipa: Table Name: {:?} Series codes: {:?} year: {:?}",
            table_name, series_codes, year
        );

        let rows = get_bea_nipa(self.reader.clone(), table_name, series_codes, year).await?;
        debug!(
            target: "economic-tool",
            "Bea: {}", rows.len()
        );
        Ok(json!({
            "bea_nipa":   if rows.is_empty() {Value::Null} else {json!(rows)},
            "provider":   "bea"
        }))
    }
}
