// rustic-economic/src/tools/fred_series.rs

use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::service::EconomicDataService;
#[derive(Debug, Clone)]
pub struct FredSeriesTool {
    service: Arc<EconomicDataService>,
}

impl FredSeriesTool {
    pub fn new(service: Arc<EconomicDataService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl Tool for FredSeriesTool {
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
                "frequency": {
                    "type": "string",
                    "enum": ["m", "q", "a"],
                    "description": "m=monthly, q=quarterly, a=annual"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of observations to return. Default 3.",
                    "default": 3
                }
            },
            "required": ["series_id", "frequency"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let series_id = params["series_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("series_id required"))?;

        let frequency = params["frequency"].as_str();
        let limit = params["limit"].as_u64().map(|n| n as usize);

        let data_points = self
            .service
            .get_fred_series(series_id, frequency, limit)
            .await?;

        Ok(json!({
            "series_id": series_id,
            "observations": data_points
        }))
    }
}
