use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::domain::tickers::indicator::TickerIndicator;
use crate::domain::tickers::indicator::indicator_type::{
    BB_LOWER, BB_UPPER, MACD, MACD_SIGNAL, RSI, SMA,
};
use crate::domain::tickers::signals::{Direction, OverallDirection};

#[derive(Debug, Serialize, Deserialize)]
pub struct TickerIndicatorEntity {
    pub symbol: String,
    pub price: f64,

    pub rsi_14: f64,
    pub rsi_band: String, // "oversold" | "neutral" | "overbought"

    pub sma_50_distance_pct: f64, // (price - sma_50) / sma_50 * 100
    pub sma_trend: String,        // "golden_cross" | "death_cross" | "no_cross"

    pub macd_histogram: f64, // macd - macd_signal
    pub macd_trend: String,  // "expanding" | "weakening" | "flat"

    pub bb_percent: f64, // (price - bb_lower) / (bb_upper - bb_lower)

    pub signals: Vec<SignalEntity>,
    pub overall: Option<OverallSignalEntity>,
    pub model_outlook: Option<ModelOutlookEntity>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignalEntity {
    pub name: String,
    pub direction: String, // "Bullish" | "Bearish" | "Neutral"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OverallSignalEntity {
    pub direction: String, // "Bullish" | "Bearish" | "Neutral" | "Mixed"
    pub summary: String,
    pub conflicting: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelOutlookEntity {
    pub lr_30d: Option<f64>,
    pub rf_30d: Option<f64>,
    pub mlp_30d: Option<f64>,
    pub divergence: bool,
}

fn direction_str(d: Direction) -> String {
    match d {
        Direction::Bullish => "Bullish",
        Direction::Bearish => "Bearish",
        Direction::Neutral => "Neutral",
    }
    .to_string()
}

fn overall_direction_str(d: OverallDirection) -> String {
    match d {
        OverallDirection::Bullish => "Bullish",
        OverallDirection::Bearish => "Bearish",
        OverallDirection::Neutral => "Neutral",
        OverallDirection::Mixed => "Mixed",
    }
    .to_string()
}

impl From<&TickerIndicator> for TickerIndicatorEntity {
    fn from(ind: &TickerIndicator) -> Self {
        let to_f64 = |k: &str| ind.values.get(k).and_then(|d| d.to_f64());

        let price = to_f64("price").unwrap_or_default();
        let rsi_14 = to_f64(&format!("{}_14", RSI)).unwrap_or_default();
        let sma_50 = to_f64(&format!("{}_50", SMA));
        let sma_200 = to_f64(&format!("{}_200", SMA));
        let macd = to_f64(MACD);
        let macd_signal = to_f64(MACD_SIGNAL);
        let bb_upper = to_f64(BB_UPPER);
        let bb_lower = to_f64(BB_LOWER);

        let rsi_band = match rsi_14 {
            r if r < 30.0 => "oversold",
            r if r > 70.0 => "overbought",
            _ => "neutral",
        }
        .to_string();

        let sma_50_distance_pct = match sma_50 {
            Some(s) if s != 0.0 => (price - s) / s * 100.0,
            _ => 0.0,
        };

        let sma_trend = match (sma_50, sma_200) {
            (Some(s50), Some(s200)) if s50 > s200 => "golden_cross",
            (Some(s50), Some(s200)) if s50 < s200 => "death_cross",
            _ => "no_cross",
        }
        .to_string();

        let macd_histogram = match (macd, macd_signal) {
            (Some(m), Some(s)) => m - s,
            _ => 0.0,
        };

        // Trend derived from whether this is expanding/weakening signal is present;
        // fallback to flat if neither fired (e.g. first row, no prev).
        let macd_trend = if ind
            .signals
            .iter()
            .any(|s| s.name == "MACD Histogram Expanding")
        {
            "expanding"
        } else if ind
            .signals
            .iter()
            .any(|s| s.name == "MACD Histogram Weakening")
        {
            "weakening"
        } else {
            "flat"
        }
        .to_string();

        let bb_percent = match (bb_upper, bb_lower) {
            (Some(u), Some(l)) if u != l => (price - l) / (u - l),
            _ => 0.0,
        };

        let signals = ind
            .signals
            .iter()
            .map(|s| SignalEntity {
                name: s.name.clone(),
                direction: direction_str(s.direction),
            })
            .collect();

        let overall = ind.overall.as_ref().map(|o| OverallSignalEntity {
            direction: overall_direction_str(o.direction),
            summary: o.summary.clone(),
            conflicting: o.conflicting,
        });

        let model_outlook = ind.model_returns.as_ref().map(|mr| {
            let lr_30d = mr.lr_returns.get("30d").copied();
            let rf_30d = mr.rf_returns.get("30d").copied();
            let mlp_30d = mr.mlp_returns.get("30d").copied();

            let divergence = [lr_30d, rf_30d, mlp_30d]
                .iter()
                .flatten()
                .any(|v| v.signum() != lr_30d.unwrap_or(0.0).signum())
                && lr_30d.is_some()
                && rf_30d.is_some()
                && mlp_30d.is_some();

            ModelOutlookEntity {
                lr_30d,
                rf_30d,
                mlp_30d,
                divergence,
            }
        });

        TickerIndicatorEntity {
            symbol: ind.symbol.clone(),
            price,
            rsi_14,
            rsi_band,
            sma_50_distance_pct,
            sma_trend,
            macd_histogram,
            macd_trend,
            bb_percent,
            signals,
            overall,
            model_outlook,
        }
    }
}
