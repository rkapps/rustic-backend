use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub struct TickerNewsEntity {
    pub date: DateTime<Utc>,
    pub symbol: String,
    pub url: String,
    pub title: String,
    pub description: String,
    pub source: String,
}
