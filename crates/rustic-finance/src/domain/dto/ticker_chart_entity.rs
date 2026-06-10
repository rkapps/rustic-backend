use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub struct TickerChartEntity {
    pub symbol: String,
    pub date: DateTime<Utc>,
    pub close: f64,
    pub sma_50: f64,
    pub sma_200: f64,
}
