use std::sync::Arc;

use anyhow::Result;
use rustic_storage::{
    MongoDatabase, Repository, SearchCriteria, mongo::repository::MongoRepository,
};
use tokio::sync::Mutex;

use crate::domain::{
    Ticker, TickerControl, TickerEmbedding, TickerHistory, TickerIndicator, TickerNews,
    TickerSentiment,
    tickers::{
        TICKER_COLLECTION_NAME, TICKER_CONTROL_COLLECTION_NAME, TICKER_EMBEDDING_COLLECTION_NAME,
        TICKER_HISTORY_COLLECTION_NAME, TICKER_INDICATOR_COLLECTION_NAME,
        TICKER_NEWS_COLLECTION_NAME, TICKER_SENTIMENT_COLLECTION_NAME,
    },
};

#[derive(Debug, Clone)]
pub struct FinanceMongoStorageManager {
    db: MongoDatabase,
}

impl FinanceMongoStorageManager {
    pub async fn new(uri: &str, name: &str) -> Result<Self> {
        let mut mdb = MongoDatabase::new(uri, name).await?;

        mdb.register_collection::<String, TickerControl>(
            TICKER_CONTROL_COLLECTION_NAME.to_string(),
        )
        .await?;

        mdb.register_collection::<String, Ticker>(TICKER_COLLECTION_NAME.to_string())
            .await?;

        mdb.register_collection::<String, TickerHistory>(
            TICKER_HISTORY_COLLECTION_NAME.to_string(),
        )
        .await?;
        mdb.register_collection::<String, TickerIndicator>(
            TICKER_INDICATOR_COLLECTION_NAME.to_string(),
        )
        .await?;

        mdb.register_collection::<String, TickerSentiment>(
            TICKER_SENTIMENT_COLLECTION_NAME.to_string(),
        )
        .await?;
        mdb.register_collection::<String, TickerEmbedding>(
            TICKER_EMBEDDING_COLLECTION_NAME.to_string(),
        )
        .await?;

        mdb.register_collection::<String, TickerNews>(TICKER_NEWS_COLLECTION_NAME.to_string())
            .await?;

        Ok(FinanceMongoStorageManager { db: mdb })
    }

    pub async fn ticker_controls(
        &self,
    ) -> Result<Arc<Mutex<MongoRepository<String, TickerControl>>>> {
        self.db
            .collection::<String, TickerControl>(TICKER_CONTROL_COLLECTION_NAME.to_string())
            .await
    }

    pub async fn tickers(&self) -> Result<Arc<Mutex<MongoRepository<String, Ticker>>>> {
        self.db
            .collection::<String, Ticker>(TICKER_COLLECTION_NAME.to_string())
            .await
    }

    pub async fn ticker_history(
        &self,
    ) -> Result<Arc<Mutex<MongoRepository<String, TickerHistory>>>> {
        self.db
            .collection::<String, TickerHistory>(TICKER_HISTORY_COLLECTION_NAME.to_string())
            .await
    }

    pub async fn ticker_indicators(
        &self,
    ) -> Result<Arc<Mutex<MongoRepository<String, TickerIndicator>>>> {
        self.db
            .collection::<String, TickerIndicator>(TICKER_INDICATOR_COLLECTION_NAME.to_string())
            .await
    }

    pub async fn ticker_sentiments(
        &self,
    ) -> Result<Arc<Mutex<MongoRepository<String, TickerSentiment>>>> {
        self.db
            .collection::<String, TickerSentiment>(TICKER_SENTIMENT_COLLECTION_NAME.to_string())
            .await
    }

    pub async fn ticker_embeddings(
        &self,
    ) -> Result<Arc<Mutex<MongoRepository<String, TickerEmbedding>>>> {
        self.db
            .collection::<String, TickerEmbedding>(TICKER_EMBEDDING_COLLECTION_NAME.to_string())
            .await
    }

    pub async fn ticker_news(&self) -> Result<Arc<Mutex<MongoRepository<String, TickerNews>>>> {
        self.db
            .collection::<String, TickerNews>(TICKER_NEWS_COLLECTION_NAME.to_string())
            .await
    }

    pub async fn get_ticker_by_criteria(&self, criteria: &SearchCriteria) -> Result<Vec<Ticker>> {
        match self.tickers().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find(Some(criteria.clone())).await
            }
            Err(e) => Err(anyhow::anyhow!("Error getting Ticker: {}", e)),
        }
    }

    pub async fn get_ticker_history_by_criteria(
        &self,
        criteria: &SearchCriteria,
    ) -> Result<Vec<TickerHistory>> {
        match self.ticker_history().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find(Some(criteria.clone())).await
            }
            Err(e) => Err(anyhow::anyhow!("Error getting TickerHistory: {}", e)),
        }
    }

    pub async fn get_ticker_indicators_by_criteria(
        &self,
        criteria: &SearchCriteria,
    ) -> Result<Vec<TickerIndicator>> {
        match self.ticker_indicators().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find(Some(criteria.clone())).await
            }
            Err(e) => Err(anyhow::anyhow!("Error getting TickerIndicator: {}", e)),
        }
    }
}
