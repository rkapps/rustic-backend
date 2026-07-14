use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{debug, info};

use crate::{domain::TickerPerformance, storage::reader::StorageReader};


#[derive(Debug)]
pub struct TickerPerformanceTool {
    storage_service: Arc<dyn StorageReader>,
}
impl TickerPerformanceTool {
    pub fn new(storage_service: Arc<dyn StorageReader>) -> TickerPerformanceTool {
        Self { storage_service }
    }
}

#[async_trait]
impl Tool for TickerPerformanceTool {
    fn name(&self) -> String {
        "ticker_performance".to_string()
    }

    fn description(&self) -> String {
        "Returns the performance of the stock in percentage over 1W, 1M, 3M, 6M, Ytd, 1Y...etc".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "symbols": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of stock ticker symbols to find peers for"
                },
            },
            "required": ["symbols"]
        })
    }

    async fn execute(&self, value: serde_json::Value) -> Result<Value> {
        #[derive(Debug, Deserialize)]
        struct Params {
            symbols: Vec<String>,
        }
        let start = std::time::Instant::now();
        let params: Params = serde_json::from_value(value.clone())
            .map_err(|e| anyhow::anyhow!("Failed to deserialize params: {:?} — {:?}", value, e))?;

        info!("Ticker params {:?}", params.symbols);
        let tickers = match self
            .storage_service
            .get_tickers_by_symbols(params.symbols.clone())
            .await
        {
            Ok(t) => t,
            Err(_) => {
                return Ok(json!({
                    "symbol": params.symbols,
                    "error": "Ticker not found in database"
                }));
            }
        };

        let performances: Vec<TickerPerformance> =
            tickers.into_iter().map(TickerPerformance::from).collect();

        debug!("Performances: {:#?}", performances);

        let elapsed = start.elapsed();
        info!(
            "Performances: {:?}  {:.1}s",
            performances.len(),
            elapsed.as_secs_f32()
        );
        Ok(json!({"performances": performances }))
    }
}
