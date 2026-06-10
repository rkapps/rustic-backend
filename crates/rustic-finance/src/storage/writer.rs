use anyhow::Result;
use std::fmt::Debug;

use async_trait::async_trait;

use crate::domain::{
    Ticker, TickerControl, TickerEmbedding, TickerHistory, TickerIndicator, TickerNews,
    TickerSentiment,
};

#[async_trait]
pub trait StorageWriter:
    TickerControlStorageWriter
    + TickerStorageWriter
    + TickerHistoryStorageWriter
    + TickerIndicatorStorageWriter
    + TickerSentimentStorageWriter
    + TickerEmbeddingStorageWriter
    + TickerNewsStorageWriter
    + Send
    + Sync
    + Debug
{
}

#[async_trait]
pub trait TickerControlStorageWriter: Send + Sync + Debug {
    async fn save_ticker_control(&self, tc: TickerControl) -> Result<()>;
    async fn save_ticker_controls(&self, tcs: Vec<TickerControl>) -> Result<()>;
}

#[async_trait]
pub trait TickerStorageWriter: Send + Sync + Debug {
    // async fn save_ticker(&self, ticker: Ticker) -> Result<()>;
    async fn save_tickers(&self, tickers: Vec<Ticker>) -> Result<()>;
}

#[async_trait]
pub trait TickerHistoryStorageWriter: Send + Sync + Debug {
    // async fn delete_ticker_history(&self, symbol: &str) -> Result<()>;
    async fn save_ticker_history(&self, symbol: &str, hist: Vec<TickerHistory>) -> Result<()>;
}

#[async_trait]
pub trait TickerIndicatorStorageWriter: Send + Sync + Debug {
    // async fn delete_ticker_indicators(&self, symbol: &str) -> Result<()>;
    // async fn delete_ticker_indicators_before(&self, date: DateTime<Utc>) -> Result<()>;
    async fn save_ticker_indicators(
        &self,
        symbol: &str,
        indicators: Vec<TickerIndicator>,
    ) -> Result<()>;
}

#[async_trait]
pub trait TickerSentimentStorageWriter: Send + Sync + Debug {
    // async fn delete_ticker_sentiments_before(&self, date: DateTime<Utc>) -> Result<()>;
    async fn save_ticker_sentiments(
        &self,
        symbol: &str,
        sentiments: Vec<TickerSentiment>,
    ) -> Result<()>;
}

#[async_trait]
pub trait TickerEmbeddingStorageWriter: Send + Sync + Debug {
    // async fn delete_ticker_embeddings_before(&self, date: DateTime<Utc>) -> Result<()>;
    async fn save_ticker_embeddings(
        &self,
        symbol: &str,
        sentiments: Vec<TickerEmbedding>,
    ) -> Result<()>;
}

#[async_trait]
pub trait TickerNewsStorageWriter: Send + Sync + Debug {
    // async fn delete_ticker_news_before(&self, date: DateTime<Utc>) -> Result<()>;
    async fn save_ticker_news(&self, symbol: &str, news: Vec<TickerNews>) -> Result<()>;
}
