use crate::domain::tickers::decimal_serde;
use crate::domain::tickers::deserialize_flexible_datetime;
use crate::domain::tickers::serialize_as_bson_datetime;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rustic_providers::finance::tiingo::model::TiingoTickerHistory;
use rustic_storage::core::repository::RepoModel;
use serde::{Deserialize, Serialize};

use crate::domain::tickers::TICKER_HISTORY_COLLECTION_NAME;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TickerHistory {
    pub id: String,

    #[serde(
        deserialize_with = "deserialize_flexible_datetime",
        serialize_with = "serialize_as_bson_datetime"
    )]
    pub date: DateTime<Utc>,

    #[serde(rename = "metadata")]
    pub metadata: TickerHistoryMetaData,
    #[serde(with = "decimal_serde")]
    pub open: Decimal,

    #[serde(with = "decimal_serde")]
    pub high: Decimal,

    #[serde(with = "decimal_serde")]
    pub low: Decimal,

    #[serde(with = "decimal_serde")]
    pub close: Decimal,

    #[serde(with = "decimal_serde")]
    pub volume: Decimal,

    #[serde(with = "decimal_serde")]
    pub adj_close: Decimal,

    #[serde(with = "decimal_serde")]
    pub adj_high: Decimal,

    #[serde(with = "decimal_serde")]
    pub adj_low: Decimal,

    #[serde(with = "decimal_serde")]
    pub adj_open: Decimal,

    #[serde(with = "decimal_serde")]
    pub adj_volume: Decimal,

    #[serde(with = "decimal_serde")]
    pub split_factor: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TickerHistoryMetaData {
    pub symbol: String,
    pub exchange: String,
    pub granularity: String,
}

// Constants
pub mod granularity {
    pub const DAILY: &str = "1d";
    pub const _HOURLY: &str = "1h";
    pub const _FIVE_MIN: &str = "5m";
    pub const _ONE_MIN: &str = "1m";
}

impl RepoModel<String> for TickerHistory {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn collection(&self) -> &'static str {
        TICKER_HISTORY_COLLECTION_NAME
    }
}

impl TickerHistory {
    /// Create from Tiingo provider data
    pub fn from_tiingo(symbol: &str, exchange: &str, tiingo: TiingoTickerHistory) -> Result<Self> {
        Ok(Self {
            id: format!("{}_{}", symbol, tiingo.date.format("%Y%m%d")),
            date: tiingo.date,
            metadata: TickerHistoryMetaData {
                symbol: symbol.to_string(),
                exchange: exchange.to_string(),
                granularity: granularity::DAILY.to_string(),
            },
            open: tiingo.open,
            high: tiingo.high,
            low: tiingo.low,
            close: tiingo.close,
            volume: tiingo.volume,
            adj_close: tiingo.adj_close,
            adj_high: tiingo.adj_high,
            adj_low: tiingo.adj_low,
            adj_open: tiingo.adj_open,
            adj_volume: tiingo.adj_volume,
            split_factor: tiingo.split_factor,
        })
    }

    /// Batch convert from Tiingo data
    pub fn from_tiingo_batch(
        symbol: &str,
        exchange: &str,
        tiingo_data: Vec<TiingoTickerHistory>,
    ) -> Result<Vec<Self>> {
        tiingo_data
            .into_iter()
            .map(|e| Self::from_tiingo(symbol, exchange, e))
            .collect()
    }
}
