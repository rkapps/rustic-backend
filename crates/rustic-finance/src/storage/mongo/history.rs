use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustic_storage::{Repository, core::search::SearchCriteria};
use tracing::{debug, error};

use crate::{
    domain::TickerHistory,
    storage::{
        mongo::{reader::FinanceMongoStorageReader, writer::FinanceMongoStorageWriter},
        reader::TickerHistoryStorageReader,
        writer::TickerHistoryStorageWriter,
    },
};

#[async_trait]
impl TickerHistoryStorageReader for FinanceMongoStorageReader {
    async fn get_ticker_history(&self, symbol: &str) -> Result<Vec<TickerHistory>> {
        let criteria = SearchCriteria::new().eq("metadata.symbol", symbol.to_uppercase());
        self.manager.get_ticker_history_by_criteria(&criteria).await
    }

    async fn get_ticker_history_by_date(
        &self,
        symbol: &str,
        from_date: DateTime<Utc>,
    ) -> Result<Vec<TickerHistory>> {
        let criteria = SearchCriteria::new()
            .eq("metadata.symbol", symbol.to_uppercase())
            .gte("date", from_date);
        self.manager.get_ticker_history_by_criteria(&criteria).await
    }
}

#[async_trait]
impl TickerHistoryStorageWriter for FinanceMongoStorageWriter {
    async fn save_ticker_history(&self, symbol: &str, hist: Vec<TickerHistory>) -> Result<()> {
        debug!("history: {}", hist.len());
        match self.manager.ticker_history().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.insert_many(hist).await
            }
            Err(e) => {
                let mesg = format!("Error saving TickerHistory for {}: {}", symbol, e);
                error!(mesg);
                return Err(anyhow::anyhow!(mesg));
            }
        }
    }
}
