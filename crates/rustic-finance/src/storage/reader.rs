use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::fmt::Debug;

use async_trait::async_trait;

use crate::domain::{
    Ticker, TickerControl, TickerEmbedding, TickerGroup, TickerHistory, TickerIndicator,
    TickerNews, TickerPeer, TickerSentiment,
    dto::{ticker_filter::TickerFilter, ticker_indicator_entity::TickerIndicatorEntity},
};

#[async_trait]
pub trait StorageReader:
    TickerControlStorageReader
    + TickerStorageReader
    + TickerHistoryStorageReader
    + TickerIndicatorStorageReader
    + TickerSentimentStorageReader
    + TickerEmbeddingStorageReader
    + TickerNewsStorageReader
    + Send
    + Sync
    + Debug
{
}

#[async_trait]
pub trait TickerControlStorageReader: Send + Sync + Debug {
    async fn get_ticker_controls(&self) -> Result<Vec<TickerControl>>;
    async fn get_ticker_control(&self, symbol: &str) -> Result<TickerControl>;
}

#[async_trait]
pub trait TickerStorageReader: Send + Sync + Debug {
    async fn get_ticker_groups(&self) -> Result<Vec<TickerGroup>>;
    async fn get_ticker_peers_by_symbols(
        &self,
        symbols: Vec<String>,
        limit: usize,
    ) -> Result<Vec<TickerPeer>>;

    async fn get_tickers_by_total_assets(&self) -> Result<Vec<Ticker>>;
    async fn get_tickers_by_symbols(&self, symbols: Vec<String>) -> Result<Vec<Ticker>>;
    async fn get_tickers_by_top_gainers(&self, asset_type: Option<String>) -> Result<Vec<Ticker>>;
    async fn get_tickers_by_top_gainers_ytd(
        &self,
        asset_type: Option<String>,
    ) -> Result<Vec<Ticker>>;
    async fn get_tickers_by_top_losers(&self, asset_type: Option<String>) -> Result<Vec<Ticker>>;
    async fn get_tickers_by_top_losers_ytd(
        &self,
        asset_type: Option<String>,
    ) -> Result<Vec<Ticker>>;

    async fn search_tickers(&self, param: TickerFilter) -> Result<Vec<Ticker>>;
}

#[async_trait]
pub trait TickerHistoryStorageReader: Send + Sync + Debug {
    async fn get_ticker_history(&self, symbol: &str) -> Result<Vec<TickerHistory>>;

    async fn get_ticker_history_by_date(
        &self,
        symbol: &str,
        from_date: DateTime<Utc>,
    ) -> Result<Vec<TickerHistory>>;
}

#[async_trait]
pub trait TickerIndicatorStorageReader: Send + Sync + Debug {
    async fn get_ticker_indicators(&self, symbol: &str) -> Result<Vec<TickerIndicator>>;
    async fn get_ticker_indicators_by_symbol(
        &self,
        symbol: &str,
        from_date: DateTime<Utc>,
    ) -> Result<Vec<TickerIndicator>>;

    async fn get_ticker_indicators_by_symbols(
        &self,
        symbols: Vec<String>,
        n: Option<usize>,
    ) -> Result<Vec<TickerIndicatorEntity>>;
}

#[async_trait]
pub trait TickerSentimentStorageReader: Send + Sync + Debug {
    async fn get_ticker_sentiments_by_ids(&self, ids: Vec<String>) -> Result<Vec<TickerSentiment>>;

    async fn get_ticker_sentiments_with_score(
        &self,
        symbols: Vec<String>,
        score: &Decimal,
    ) -> Result<Vec<TickerSentiment>>;
}

#[async_trait]
pub trait TickerEmbeddingStorageReader: Send + Sync + Debug {
    async fn get_ticker_embeddings(&self, symbols: Vec<String>) -> Result<Vec<TickerEmbedding>>;
}

#[async_trait]
pub trait TickerNewsStorageReader: Send + Sync + Debug {
    async fn get_ticker_news(&self, symbol: &str) -> Result<Vec<TickerNews>>;
}
