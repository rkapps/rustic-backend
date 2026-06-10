use async_trait::async_trait;
use rust_decimal::Decimal;
use rustic_storage::core::{repository::Repository, search::SearchCriteria};

use anyhow::Result;

use crate::{
    domain::TickerSentiment,
    storage::{
        mongo::{reader::FinanceMongoStorageReader, writer::FinanceMongoStorageWriter},
        reader::TickerSentimentStorageReader,
        writer::TickerSentimentStorageWriter,
    },
};

#[async_trait]
impl TickerSentimentStorageReader for FinanceMongoStorageReader {
    async fn get_ticker_sentiments_by_ids(&self, ids: Vec<String>) -> Result<Vec<TickerSentiment>> {
        let criteria = SearchCriteria::new().in_values("id", ids);
        match self.manager.ticker_sentiments().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find(Some(criteria)).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting TickerSentiment: {}", e));
            }
        }
    }

    async fn get_ticker_sentiments_with_score(
        &self,
        symbols: Vec<String>,
        score: &Decimal,
    ) -> Result<Vec<TickerSentiment>> {
        let criteria = SearchCriteria::new()
            .in_values("symbol", symbols)
            .gte("relevance_score", *score);
        match self.manager.ticker_sentiments().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find(Some(criteria)).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting TickerSentiment: {}", e));
            }
        }
    }
}

#[async_trait]
impl TickerSentimentStorageWriter for FinanceMongoStorageWriter {
    async fn save_ticker_sentiments(
        &self,
        symbol: &str,
        sentiments: Vec<TickerSentiment>,
    ) -> Result<()> {
        match self.manager.ticker_sentiments().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.bulk_update(sentiments).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!(format!(
                    "Error saving TickerSentiments for {}: {}",
                    symbol, e
                )));
            }
        }
    }
}
