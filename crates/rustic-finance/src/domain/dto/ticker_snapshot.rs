use crate::domain::Ticker;
use rust_decimal::prelude::ToPrimitive;
use rustic_core::serialize_vec_or_null;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TickerSnapshot {
    pub symbol: String,
    pub name: String,
    pub sector: String,
    pub industry: String,
    pub last: f64,
    pub prev: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub change_amt: f64,
    pub change_perc: f64,
    pub wk52_high: f64,
    pub wk52_low: f64,
    pub total_assets: Option<i64>,
    pub eps: Option<f64>,
    pub pe_ratio: Option<f64>,
    pub peg_ratio: Option<f64>,
    pub pb_ratio: Option<f64>,
    pub ps_ratio: Option<f64>,
    pub forward_pe: Option<f64>,
    pub beta: Option<f64>,
    pub analyst_target_price: Option<f64>,
    pub analyst_consensus: Option<String>,

    #[serde(serialize_with = "serialize_vec_or_null")]
    pub technical_signals: Vec<String>,
    #[serde(serialize_with = "serialize_vec_or_null")]
    pub mlp_signals: Vec<String>,
    #[serde(serialize_with = "serialize_vec_or_null")]
    pub ml_signals: Vec<String>,
}

impl From<Ticker> for TickerSnapshot {
    fn from(ticker: Ticker) -> Self {
        let mut technical_signals = Vec::new();
        let mut mlp_signals = Vec::new();
        let mut ml_signals = Vec::new();

        for signal in ticker.signals {
            if signal.starts_with("MLP") {
                mlp_signals.push(signal);
            } else if signal.starts_with("ML") {
                ml_signals.push(signal);
            } else {
                technical_signals.push(signal);
            }
        }

        TickerSnapshot {
            symbol: ticker.symbol,
            name: ticker.name,
            sector: ticker.sector.unwrap_or_default(),
            industry: ticker.industry.unwrap_or_default(),
            last: ticker.pr_last.to_f64().unwrap_or_default(),
            prev: ticker.pr_prev.to_f64().unwrap_or_default(),
            open: ticker.pr_open.to_f64().unwrap_or_default(),
            high: ticker.pr_high.to_f64().unwrap_or_default(),
            low: ticker.pr_low.to_f64().unwrap_or_default(),
            change_amt: ticker.pr_diff_amt.to_f64().unwrap_or_default(),
            change_perc: ticker.pr_diff_perc.to_f64().unwrap_or_default(),
            wk52_high: ticker.pr_52_wk_high.to_f64().unwrap_or_default(),
            wk52_low: ticker.pr_52_wk_low.to_f64().unwrap_or_default(),
            analyst_consensus: ticker.analyst_consensus,
            analyst_target_price: ticker.analyst_target_price,
            beta: ticker.beta,
            eps: ticker.eps,
            forward_pe: ticker.forward_pe,
            pb_ratio: ticker.pb_ratio,
            pe_ratio: ticker.pe_ratio,
            peg_ratio: ticker.peg_ratio,
            ps_ratio: ticker.ps_ratio,
            total_assets: ticker.total_assets,
            technical_signals,
            mlp_signals,
            ml_signals,
        }
    }
}
