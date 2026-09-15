use serde::{Deserialize, Serialize};

use crate::domain::dto::ticker_search_param::TickerSearchParam;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TickerFilter {
    // search
    pub asset_type: Option<String>,       // "stock", "etf"
    pub query: Option<String>,            // semantic: "cloud security", "payments infrastructure"
    pub signals: Option<Vec<String>>,     // ["RSI Oversold", "MACD Bullish Crossover"]
    pub industries: Option<Vec<String>>,         
    pub assets_cap_range: Option<String>, // "mega", "large", "mid", "small"
    pub metric_filters: Option<Vec<MetricFilter>>,

    // sorting
    pub sort_by: Option<String>, // "price", "change_pct", "volume", "market_cap"
    pub sort_dir: Option<String>, // "asc", "desc"

    // pagination
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetricFilter {
    pub field: String,      // "pe_ratio", "eps", "beta", "performance_3m", etc — enum-constrained
    pub op: String,         // "gt", "lt", "gte", "lte"
    pub value: f64,
}

impl From<TickerSearchParam> for TickerFilter {
    fn from(param: TickerSearchParam) -> TickerFilter {
        TickerFilter {
            asset_type: param.asset_type,
            query: param.query,
            signals: param.signals,
            industries: param.industries,
            assets_cap_range: param.assets_cap_range,
            sort_by: param.sort_by,
            sort_dir: param.sort_dir,
            limit: param.limit,
            metric_filters: None
        }
    }
}
