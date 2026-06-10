use serde::{Deserialize, Serialize};

use crate::domain::dto::ticker_search_param::TickerSearchParam;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TickerFilter {
    // search
    pub asset_type: Option<String>,       // "stock", "etf"
    pub query: Option<String>,            // semantic: "cloud security", "payments infrastructure"
    pub signals: Option<Vec<String>>,     // ["RSI Oversold", "MACD Bullish Crossover"]
    pub industry: Option<String>,         // regex match
    pub assets_cap_range: Option<String>, // "mega", "large", "mid", "small"
    pub r#yield: Option<f32>,

    // sorting
    pub sort_by: Option<String>, // "price", "change_pct", "volume", "market_cap"
    pub sort_dir: Option<String>, // "asc", "desc"

    // pagination
    pub limit: Option<usize>,
}

impl From<TickerSearchParam> for TickerFilter {
    fn from(param: TickerSearchParam) -> TickerFilter {
        TickerFilter {
            asset_type: param.asset_type,
            query: param.query,
            signals: param.signals,
            industry: param.industry,
            assets_cap_range: param.assets_cap_range,
            r#yield: param.r#yield,
            sort_by: param.sort_by,
            sort_dir: param.sort_dir,
            limit: param.limit,
        }
    }
}
