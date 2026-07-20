use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::domain::Ticker;

#[derive(Debug, Serialize, Deserialize)]
pub struct TickerPerformance {
    pub symbol: String,
    pub diffs: HashMap<String, HashMap<String, f64>>,
}

impl From<Ticker> for TickerPerformance {
    fn from(ticker: Ticker) -> Self {
        TickerPerformance {
            symbol: ticker.symbol,
            diffs: ticker.performance_search,
        }
    }
}
