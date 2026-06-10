use anyhow::Result;
use async_trait::async_trait;
use rustic_storage::{Repository, SearchCriteria};
use tracing::debug;

use crate::{
    domain::TickerNews,
    storage::{
        FinanceMongoStorageReader, mongo::writer::FinanceMongoStorageWriter,
        reader::TickerNewsStorageReader, writer::TickerNewsStorageWriter,
    },
};

#[async_trait]
impl TickerNewsStorageReader for FinanceMongoStorageReader {
    async fn get_ticker_news(&self, symbol: &str) -> Result<Vec<TickerNews>> {
        let criteria = SearchCriteria::new()
            .eq("symbol", symbol.to_uppercase())
            .limit(50)
            .sort_desc("date");
        match self.manager.ticker_news().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                debug!("Criteria: {:?}", criteria);
                repo.find(Some(criteria)).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting TickerNews: {}", e));
            }
        }
    }
}

#[async_trait]
impl TickerNewsStorageWriter for FinanceMongoStorageWriter {
    async fn save_ticker_news(&self, symbol: &str, embeddings: Vec<TickerNews>) -> Result<()> {
        match self.manager.ticker_news().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.bulk_update(embeddings).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!(format!(
                    "Error saving TickerNewss for {}: {}",
                    symbol, e
                )));
            }
        }
    }
}
