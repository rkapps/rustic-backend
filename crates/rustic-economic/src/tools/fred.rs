use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::debug;

use crate::{core::fred::get_fred_series, storage::mongo::reader::EconomicMongoStorageReader};
#[derive(Debug, Clone)]
pub struct FredDataTool {
    reader: Arc<EconomicMongoStorageReader>,
}

impl FredDataTool {
    pub fn new(reader: Arc<EconomicMongoStorageReader>) -> Self {
        Self { reader }
    }
}

#[async_trait]
impl Tool for FredDataTool {
    fn name(&self) -> String {
        "fred_series".to_string()
    }

    fn description(&self) -> String {
        "Fetch Federal Reserve economic time series data. Use for consumer spending, sentiment, unemployment, housing starts, CPI and other macro indicators.".to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "series_id": {
                    "type": "string",
                    "description": "FRED series ID e.g. CPIAUCSL, UMCSENT, UNRATE, HOUST"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of observations to return. Default 3.",
                    "default": 3
                }
            },
            "required": ["series_id"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let series_id = params["series_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("series_id required"))?;

        let limit = params["limit"].as_u64().map(|n| n as usize);
        let data_points = get_fred_series(self.reader.clone(), series_id, limit).await?;

        debug!("series_id: {} observations: {:?}", series_id, data_points);
        Ok(json!({
            "series_id": series_id,
            "observations": data_points
        }))
    }
}
