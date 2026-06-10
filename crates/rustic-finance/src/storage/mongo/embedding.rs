use async_trait::async_trait;
use rustic_storage::core::{repository::Repository, search::SearchCriteria};

use anyhow::Result;

use crate::{
    domain::TickerEmbedding,
    storage::{
        mongo::{reader::FinanceMongoStorageReader, writer::FinanceMongoStorageWriter},
        reader::TickerEmbeddingStorageReader,
        writer::TickerEmbeddingStorageWriter,
    },
};

#[async_trait]
impl TickerEmbeddingStorageReader for FinanceMongoStorageReader {
    async fn get_ticker_embeddings(&self, symbols: Vec<String>) -> Result<Vec<TickerEmbedding>> {
        let criteria = SearchCriteria::new().in_values("symbol", symbols);
        match self.manager.ticker_embeddings().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find(Some(criteria)).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting TickerEmbedding: {}", e));
            }
        }
    }
}

#[async_trait]
impl TickerEmbeddingStorageWriter for FinanceMongoStorageWriter {
    async fn save_ticker_embeddings(
        &self,
        symbol: &str,
        embeddings: Vec<TickerEmbedding>,
    ) -> Result<()> {
        match self.manager.ticker_embeddings().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.bulk_update(embeddings).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!(format!(
                    "Error saving TickerEmbeddings for {}: {}",
                    symbol, e
                )));
            }
        }
    }
}
