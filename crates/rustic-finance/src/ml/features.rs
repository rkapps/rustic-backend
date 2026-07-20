use rust_decimal::Decimal;
use rustic_core::utils::{dec_utils::decimal_to_float, float_utils::round_to_precision_2};

use crate::domain::TickerIndicator;
const RSI_DEFAULT: f64 = 45.0;
// const STOCH_DEFAULT: f64 = 50.0;

pub struct Features {
    indicator: TickerIndicator,
    prev_indicator: TickerIndicator,
}

impl Features {
    pub fn new(indicator: TickerIndicator, prev_indicator: TickerIndicator) -> Self {
        Self {
            indicator,
            prev_indicator,
        }
    }

    /// Human-readable feature names — keep in sync with values() order.
    pub fn _feature_names() -> Vec<&'static str> {
        vec![
            "rsi_14",
            "rsi_divergence",
            "pr_sma20_pct",
            "pr_sma50_pct",
            "pr_sma200_pct",
            "bb_position_pct",
            "bb_width_pct",
            "macd_histogram",
            "macd_histogram_slope",
            "stochastic_k",
            "stoch_divergence",
            "atr_price_pct",
            "volume_ratio",
        ]
    }

    pub fn values(&self) -> Vec<f64> {
        vec![
            self.get_rsi_value("rsi_14"),
            round_to_precision_2(self.get_rsi_divergence()),
        ]
    }

    // get indicator values
    pub fn get_indicator_value(&self, value: &str) -> Option<Decimal> {
        self.indicator.values.get(value).copied()
    }

    // get prev indicator values
    pub fn _get_prev_indicator_value(&self, value: &str) -> Option<Decimal> {
        self.prev_indicator.values.get(value).copied()
    }

    // get rsi
    fn get_rsi_value(&self, value: &str) -> f64 {
        let value = self.get_indicator_value(value);
        decimal_to_float(value, RSI_DEFAULT)
    }

    // get rsi divergence
    fn get_rsi_divergence(&self) -> f64 {
        let rsi_10 = self.get_rsi_value("rsi_10");
        let rsi_26 = self.get_rsi_value("rsi_26");
        rsi_10 - rsi_26
    }
}
