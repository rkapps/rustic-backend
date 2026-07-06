use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::debug;

use crate::{
    core::fred::get_fred_series, storage::mongo::reader::EconomicMongoStorageReader,
    tools::domain::FredSeriesEntity,
};
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
                "series_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of Fred series e.g. CPIAUCSL, UMCSENT, UNRATE, HOUST"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of observations to return. Default 12.",
                    "default": 12
                }
            },
            "required": ["series_ids"]
        })
    }

    async fn execute(&self, value: Value) -> Result<Value> {
        #[derive(Debug, Deserialize)]
        struct Params {
            series_ids: Vec<String>,
            #[serde(default = "default_limit")]
            limit: Option<usize>,
        }
        fn default_limit() -> Option<usize> {
            Some(12)
        }

        let params: Params = serde_json::from_value(value.clone())
            .map_err(|e| anyhow::anyhow!("Failed to deserialize params: {:?} — {:?}", value, e))?;

        let series_ids = params.series_ids;
        let limit = params.limit;

        let mut fseriesa = Vec::new();
        for series_id in series_ids {
            let series = get_fred_series(self.reader.clone(), &series_id).await?;
            let obs = match limit {
                Some(n) => series.observations.into_iter().take(n).collect(),
                None => series.observations,
            };
            let fseries = FredSeriesEntity {
                series_id: series.series_id,
                name: series.name,
                category: series.category,
                observations: obs,
            };
            fseriesa.push(fseries);
        }

        debug!(
            target: "economic-tool",
            "Fred series: {:?}", fseriesa.len()
        );

        Ok(json!({"fred_series": fseriesa, "provider":   "fred"}))
    }
}
