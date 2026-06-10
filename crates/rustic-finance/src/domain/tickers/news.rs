use crate::domain::tickers::TICKER_NEWS_COLLECTION_NAME;
use crate::domain::tickers::deserialize_flexible_datetime;
use crate::domain::tickers::serialize_as_bson_datetime;
use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use rustic_providers::finance::tiingo::model::TiingoTickerNews;
use rustic_storage::core::repository::RepoModel;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TickerNews {
    pub id: String,

    #[serde(
        deserialize_with = "deserialize_flexible_datetime",
        serialize_with = "serialize_as_bson_datetime"
    )]
    pub date: DateTime<Utc>,
    pub symbol: String,
    pub url: String,
    pub title: String,
    pub description: String,
    pub source: String,
}

impl RepoModel<String> for TickerNews {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn collection(&self) -> &'static str {
        TICKER_NEWS_COLLECTION_NAME
    }
}

impl TickerNews {
    /// Create from Tiingo provider data
    pub fn from_tiingo(symbol: &str, tiingo: TiingoTickerNews) -> Result<Self> {
        Ok(Self {
            id: format!("{}_{}", symbol, tiingo.date.format("%Y%m%d")),
            date: tiingo.date,
            description: tiingo.description.unwrap(),
            source: tiingo.source,
            symbol: symbol.to_string(),
            title: tiingo.title,
            url: tiingo.url,
        })
    }

    /// Batch convert from Tiingo data
    pub fn from_tiingo_batch(
        symbol: &str,
        tiingo_data: Vec<TiingoTickerNews>,
    ) -> Result<Vec<Self>> {
        tiingo_data
            .into_iter()
            .filter(|e| e.description.is_some())
            .map(|e| Self::from_tiingo(symbol, e))
            .collect()
    }
}
