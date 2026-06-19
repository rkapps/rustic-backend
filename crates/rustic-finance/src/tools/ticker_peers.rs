use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::info;

use crate::storage::reader::StorageReader;

#[derive(Debug)]
pub struct TickerPeersTool {
    storage_service: Arc<dyn StorageReader>,
}
impl TickerPeersTool {
    pub fn new(storage_service: Arc<dyn StorageReader>) -> TickerPeersTool {
        Self { storage_service }
    }
}

#[async_trait]
impl Tool for TickerPeersTool {
    fn name(&self) -> String {
        "ticker_peers".to_string()
    }

    fn description(&self) -> String {
        "Returns peer stocks for a given ticker across three dimensions: \
 industry peers (same industry), sector peers (same sector), \
 and similar stocks (pre-computed embedding similarity on business description). \
 ALWAYS call this tool first before fetching any peer data. \
 Never use training knowledge to assume peers. \
 Use the returned symbols to decide which stocks to analyse further."
            .to_string()
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
                "limit": {
                    "type": "integer",
                    "description": "Max number of peers to return.",
                    "default": 10
                }
            },
            "required": ["symbols"]
        })
    }

    async fn execute(&self, value: serde_json::Value) -> Result<Value> {
        #[derive(Debug, Deserialize)]
        struct Params {
            symbols: Vec<String>,
            #[serde(default = "default_limit")]
            limit: usize,
        }

        fn default_limit() -> usize {
            10
        }
        let start = std::time::Instant::now();

        let params: Params = serde_json::from_value(value.clone())
            .map_err(|e| anyhow::anyhow!("Failed to deserialize params: {:?} — {:?}", value, e))?;

        let peers = self
            .storage_service
            .get_ticker_peers_by_symbols(params.symbols, params.limit)
            .await?;

        let elapsed = start.elapsed();
        info!("Peers: {:?}  {:.1}s", peers.len(), elapsed.as_secs_f32());

        Ok(json!({ "peers": if peers.is_empty() {Value::Null} else { json!(peers)} }))

    }
}
