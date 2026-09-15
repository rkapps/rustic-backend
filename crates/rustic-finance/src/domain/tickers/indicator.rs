use std::collections::HashMap;

use crate::domain::tickers::deserialize_flexible_datetime;
use crate::domain::tickers::indicator_serde;
use crate::domain::tickers::serialize_as_bson_datetime;
use crate::domain::tickers::signals::OverallSignal;
use crate::domain::tickers::signals::Signal;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rustic_storage::core::repository::RepoModel;
use serde::{Deserialize, Serialize};

use crate::domain::tickers::TICKER_INDICATOR_COLLECTION_NAME;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerIndicator {
    pub id: String,

    pub symbol: String,
    #[serde(
        deserialize_with = "deserialize_flexible_datetime",
        serialize_with = "serialize_as_bson_datetime"
    )]
    pub date: DateTime<Utc>,

    #[serde(with = "indicator_serde")]
    pub values: HashMap<String, Decimal>, // "sma_20": 182.3, "rsi_14": 68.4 etc

    #[serde(default)]
    pub signals: Vec<Signal>, // tier 1 + tier 2, technical

    #[serde(default)]
    pub overall: Option<OverallSignal>, // tier 3 rollup — Option since it needs enough signals to mean anything

    #[serde(default)]
    pub model_signals: Vec<Signal>, // written later by predictions job

    #[serde(default)]
    pub model_returns: Option<ModelReturns>,
}

// Indicator type constants
pub mod indicator_type {
    pub const SMA: &str = "sma";
    pub const EMA: &str = "ema";
    pub const RSI: &str = "rsi";
    pub const MACD: &str = "macd";
    pub const MACD_SIGNAL: &str = "macd_signal";
    pub const MACD_HISTOGRAM: &str = "macd_histogram";
    pub const BB_UPPER: &str = "bb_upper";
    pub const BB_MIDDLE: &str = "bb_middle";
    pub const BB_LOWER: &str = "bb_lower";
    pub const ATR: &str = "atr";
    pub const STOCHASTIC_K: &str = "stochastic_k"; // %K (fast line)
    pub const STOCHASTIC_D: &str = "stochastic_d"; // %D (slow/signal line)
    pub const VOLUME_RATIO: &str = "volume_ratio";
}

impl RepoModel<String> for TickerIndicator {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn collection(&self) -> &'static str {
        TICKER_INDICATOR_COLLECTION_NAME
    }
}

impl TickerIndicator {
    pub fn new(
        date: DateTime<Utc>,
        symbol: &str,
        values: HashMap<String, Decimal>,
        signals: Vec<Signal>,
        overall: Option<OverallSignal>,
    ) -> TickerIndicator {
        Self {
            id: format!("{}_{}", symbol, date.format("%Y%m%d")),
            symbol: symbol.to_string(),
            date,
            values,
            signals,
            overall,
            model_signals: Vec::new(),
            model_returns: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelReturns {
    pub lr_returns: HashMap<String, f64>,
    pub rf_returns: HashMap<String, f64>,
    pub mlp_returns: HashMap<String, f64>,
}

#[derive(Debug, Clone)]
pub struct IndicatorSnapshot {
    pub price: Option<Decimal>,
    pub rsi_10: Option<Decimal>,
    pub rsi_14: Option<Decimal>,
    pub rsi_26: Option<Decimal>,
    pub sma_20: Option<Decimal>,
    pub sma_50: Option<Decimal>,
    pub sma_100: Option<Decimal>,
    pub sma_200: Option<Decimal>,
    pub macd: Option<Decimal>,
    pub macd_signal: Option<Decimal>,
    pub macd_histogram: Option<Decimal>,
    pub bb_upper: Option<Decimal>,
    pub bb_middle: Option<Decimal>,
    pub bb_lower: Option<Decimal>,
    pub stochastic_k_14: Option<Decimal>,
    pub stochastic_d: Option<Decimal>,
    pub atr: Option<Decimal>,
    pub volume_ratio: Option<Decimal>,
}

impl From<&TickerIndicator> for IndicatorSnapshot {
    fn from(value: &TickerIndicator) -> Self {
        let values = value.values.clone();
        IndicatorSnapshot {
            price: values.get("price").copied(),
            sma_20: values.get("sma_20").copied(),
            sma_50: values.get("sma_50").copied(),
            sma_100: values.get("sma_100").copied(),
            sma_200: values.get("sma_200").copied(),
            macd: values.get("macd").copied(),
            macd_signal: values.get("macd_signal").copied(),
            macd_histogram: values.get("macd_histogram").copied(),
            bb_upper: values.get("bb_upper").copied(),
            bb_middle: values.get("bb_middle").copied(),
            bb_lower: values.get("bb_lower").copied(),
            stochastic_k_14: values.get("stochastic_k_14").copied(),
            stochastic_d: values.get("stochastic_d").copied(),
            rsi_10: values.get("rsi_10").copied(),
            rsi_14: values.get("rsi_14").copied(),
            rsi_26: values.get("rsi_26").copied(),
            atr: values.get("atr").copied(),
            volume_ratio: values.get("volume_ratio").copied(),
        }
    }
}

pub struct IndicatorWindow {
    pub curr: IndicatorSnapshot,
    pub prev: IndicatorSnapshot,
}

impl IndicatorWindow {
    pub fn new(curr: IndicatorSnapshot, prev: IndicatorSnapshot) -> Self {
        Self { curr, prev }
    }
}
