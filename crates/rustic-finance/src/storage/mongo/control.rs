use anyhow::Result;
use async_trait::async_trait;
use rustic_storage::Repository;

use crate::{
    domain::TickerControl,
    storage::{
        FinanceMongoStorageReader, mongo::writer::FinanceMongoStorageWriter,
        reader::TickerControlStorageReader, writer::TickerControlStorageWriter,
    },
};

#[async_trait]
impl TickerControlStorageReader for FinanceMongoStorageReader {
    async fn get_ticker_control(&self, symbol: &str) -> Result<TickerControl> {
        match self.manager.ticker_controls().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find_by_id(symbol.to_string()).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting TickerControl: {}", e));
            }
        }
    }

    async fn get_ticker_controls(&self) -> Result<Vec<TickerControl>> {
        match self.manager.ticker_controls().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find_all().await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting TickerControl: {}", e));
            }
        }
    }
}

#[async_trait]
impl TickerControlStorageWriter for FinanceMongoStorageWriter {
    async fn save_ticker_control(&self, tc: TickerControl) -> Result<()> {
        match self.manager.ticker_controls().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.update(tc).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!(format!(
                    "Error saving TickerControl for '{}' error: {}",
                    tc.symbol, e
                )));
            }
        }
    }

    async fn save_ticker_controls(&self, tcs: Vec<TickerControl>) -> Result<()> {
        match self.manager.ticker_controls().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.bulk_update(tcs).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!(format!(
                    "Error saving TickerControls: {}",
                    e
                )));
            }
        }
    }
}
