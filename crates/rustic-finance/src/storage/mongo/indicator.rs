use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustic_storage::{SearchCriteria, core::repository::Repository};
use serde_json::json;
use tracing::{debug, warn};

use anyhow::Result;

use crate::{
    domain::{TickerIndicator, dto::ticker_indicator_entity::TickerIndicatorEntity},
    storage::{
        mongo::{reader::FinanceMongoStorageReader, writer::FinanceMongoStorageWriter},
        reader::TickerIndicatorStorageReader,
        writer::TickerIndicatorStorageWriter,
    },
};

#[async_trait]
impl TickerIndicatorStorageReader for FinanceMongoStorageReader {
    async fn get_ticker_indicators(&self, symbol: &str) -> Result<Vec<TickerIndicator>> {
        let criteria = SearchCriteria::new()
            .eq("symbol", symbol.to_uppercase())
            .sort_asc("date");
        self.manager
            .get_ticker_indicators_by_criteria(&criteria)
            .await
    }

    async fn get_ticker_indicators_by_symbol(
        &self,
        symbol: &str,
        from_date: DateTime<Utc>,
    ) -> Result<Vec<TickerIndicator>> {
        let criteria = SearchCriteria::new()
            .eq("symbol", symbol.to_uppercase())
            .gte("date", from_date)
            .sort_asc("date");
        self.manager
            .get_ticker_indicators_by_criteria(&criteria)
            .await
    }

    async fn get_ticker_indicators_by_symbols(
        &self,
        symbols: Vec<String>,
        n: Option<usize>,
    ) -> Result<Vec<TickerIndicatorEntity>> {
        debug!("symbols: {:?}", symbols);

        let n = n.unwrap_or(0);

        match self.manager.ticker_indicators().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;

                let pipeline = vec![
                    json!({ "$match": { "symbol": { "$in": &symbols } } }),
                    json!({ "$sort": { "symbol": 1, "date": -1 } }),
                    json!({ "$group": {
                        "_id": "$symbol",
                        "docs": { "$push": "$$ROOT" }
                    }}),
                    match n {
                        0 => json!({ "$project": {
                            "symbol": "$_id",
                            "records": "$docs",
                            "_id": 0
                        }}),
                        _ => json!({ "$project": {
                            "symbol": "$_id",
                            "records": { "$slice": ["$docs", n as i64] },
                            "_id": 0
                        }}),
                    },
                    json!({ "$unwind": "$records" }),
                    json!({ "$replaceRoot": { "newRoot": "$records" } }),
                    json!({ "$addFields": {
                        "rsi_14": { "$toDouble": "$values.rsi_14" },
                        "sma_50": { "$toDouble": "$values.sma_50" },
                        "sma_200": { "$toDouble": "$values.sma_200" },
                        "macd": { "$toDouble": "$values.macd" },
                        "macd_signal": { "$toDouble": "$values.macd_signal" },
                        "bb_upper": { "$toDouble": "$values.bb_upper" },
                        "bb_lower": { "$toDouble": "$values.bb_lower" },
                    }}),
                    json!({ "$project": {
                        "id" : 1,
                        "symbol": 1,
                        "date": 1,
                        "rsi_14": 1,
                        "sma_50": 1,
                        "sma_200": 1,
                        "macd": 1,
                        "macd_signal": 1,
                        "bb_upper": 1,
                        "bb_lower": 1,
                        "_id": 0
                    }}),
                ];
                let results = repo.aggregate(pipeline).await?;
                debug!("results: {:?}", results);

                let indicators: Vec<TickerIndicatorEntity> = results
                    .iter()
                    .filter_map(|v| match serde_json::from_value(v.clone()) {
                        Ok(i) => Some(i),
                        Err(e) => {
                            warn!(
                                "Failed to deserialize TickerIndicator: {} value: {:#?}",
                                e, v
                            );
                            None
                        }
                    })
                    .collect();
                debug!("results: {:?}", indicators);
                Ok(indicators)
            }
            Err(e) => Err(anyhow::anyhow!("Error getting TickerIndicator: {}", e)),
        }
    }
}

#[async_trait]
impl TickerIndicatorStorageWriter for FinanceMongoStorageWriter {
    async fn save_ticker_indicators(
        &self,
        symbol: &str,
        indicators: Vec<TickerIndicator>,
    ) -> Result<()> {
        match self.manager.ticker_indicators().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.bulk_update(indicators).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!(format!(
                    "Error saving TickerIndicators for {}: {}",
                    symbol, e
                )));
            }
        }
    }
}
