use crate::{
    domain::{
        TickerHistory,
        dto::ticker_seed::TickerSeed,
        tickers::{AssetType, decimal_serde, performance_serde},
    },
    util::{
        date_utils::same_date,
        string_utils::{
            string_to_decimal, string_to_float, string_to_int32, string_to_int64,
            string_to_utc_datetime,
        },
    },
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use rust_decimal_macros::dec;
use rustic_providers::finance::{
    alpha::model::{AlphaEtf, AlphaTicker},
    cmc::model::{CmcCryptoData, CmcCryptoQuote},
    tiingo::model::TiingoTickerRealtime,
};
use rustic_storage::core::repository::RepoModel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Ticker {
    pub id: String,
    pub asset_type: AssetType,
    pub active: bool,
    pub symbol: String,
    pub exchange: String,
    pub name: String,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub overview: String,

    pub total_assets: Option<i64>,

    pub pr_date: Option<DateTime<Utc>>,
    #[serde(with = "decimal_serde")]
    pub pr_last: Decimal,
    #[serde(with = "decimal_serde")]
    pub pr_prev: Decimal,
    #[serde(with = "decimal_serde")]
    pub pr_diff_amt: Decimal,
    #[serde(with = "decimal_serde")]
    pub pr_diff_perc: Decimal,

    #[serde(with = "decimal_serde")]
    pub pr_open: Decimal,
    #[serde(with = "decimal_serde")]
    pub pr_high: Decimal,
    #[serde(with = "decimal_serde")]
    pub pr_low: Decimal,
    #[serde(with = "decimal_serde")]
    pub pr_close: Decimal,
    pub pr_diff_perc_search: f64,
    #[serde(with = "decimal_serde")]
    pub pr_52_wk_high: Decimal,
    #[serde(with = "decimal_serde")]
    pub pr_52_wk_low: Decimal,

    #[serde(with = "performance_serde")]
    pub performance: HashMap<String, HashMap<String, Decimal>>,

    pub dividend_amt: f64,
    pub r#yield: f64,
    pub ex_div_date: Option<DateTime<Utc>>,
    pub pay_date: Option<DateTime<Utc>>,
    pub pay_ratio: f64,
    pub avg_volume: i32,
    pub volume: i32,

    #[serde(default)]
    pub signals: Vec<String>,

    #[serde(default)]
    pub lr_returns: HashMap<String, f64>, // LinearRegression returns

    #[serde(default)]
    pub rf_returns: HashMap<String, f64>, // RandomForst returns

    #[serde(default)]
    pub mlp_returns: HashMap<String, f64>, // MLP returns

    // pub market_cap: Option<i64>,
    pub expense_ratio: Option<f64>, // etfs
    pub eps: Option<f64>,
    pub pe_ratio: Option<f64>,
    pub peg_ratio: Option<f64>,
    pub pb_ratio: Option<f64>,
    pub ps_ratio: Option<f64>,
    pub forward_pe: Option<f64>,
    pub beta: Option<f64>,
    pub profit_margin: Option<f64>,
    pub operating_margin_ttm: Option<f64>,
    pub return_on_equity_ttm: Option<f64>,
    pub return_on_asset_ttm: Option<f64>,
    pub ebitda: Option<i64>,
    pub ev_to_revenue: Option<f64>,
    pub ev_to_ebitda: Option<f64>,
    pub shares_outstanding: Option<i64>,
    pub shares_float: Option<i64>,
    pub percent_insiders: Option<f64>,
    pub percent_institutions: Option<f64>,

    pub quarterly_earnings_growth_yoy: Option<f64>,
    pub quarterly_revenue_growth_yoy: Option<f64>,
    pub analyst_target_price: Option<f64>,
    pub analyst_rating_strong_buy: Option<i32>,
    pub analyst_rating_buy: Option<i32>,
    pub analyst_rating_hold: Option<i32>,
    pub analyst_rating_sell: Option<i32>,
    pub analyst_rating_strong_sell: Option<i32>,
    pub analyst_consensus: Option<String>,

    // The "Search" version (Hidden from JSON, used for Atlas)
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")] // 2. Keeps Mongo clean
    pub performance_search: HashMap<String, HashMap<String, f64>>,

    #[serde(default)]
    pub indicators_search: HashMap<String, f64>,

    pub overview_text: Option<String>,
    pub overview_embedding: Option<Vec<f32>>,

    pub industry_text: Option<String>,
    pub industry_embedding: Option<Vec<f32>>,

    pub country: String,
    pub currency: String,
}

impl RepoModel<String> for Ticker {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn collection(&self) -> &'static str {
        "ticker"
    }
}

impl Ticker {
    pub fn new(seed: TickerSeed) -> Self {
        Ticker {
            id: seed.symbol.clone(),
            asset_type: seed.asset_type,
            active: true,
            symbol: seed.symbol,
            exchange: seed.exchange,
            name: seed.name,
            sector: Some(seed.sector),
            industry: Some(seed.industry),
            overview: seed.overview,
            ..Ticker::default()
        }
    }
}

impl Ticker {
    pub fn update_from_alpha(&mut self, value: AlphaTicker) {
        self.total_assets = Some(string_to_int64(value.market_capitalization));
        self.eps = Some(string_to_float(&value.eps));
        self.pe_ratio = Some(string_to_float(&value.peratio));
        self.peg_ratio = Some(string_to_float(&value.pegratio));
        self.pb_ratio = Some(string_to_float(&value.pbratio));
        self.ps_ratio = Some(string_to_float(&value.psratio));
        self.forward_pe = Some(string_to_float(&value.foward_pe));
        self.beta = Some(string_to_float(&value.beta));
        self.profit_margin = Some(string_to_float(&value.profit_margin));
        self.operating_margin_ttm = Some(string_to_float(&value.operating_margin_ttm));
        self.return_on_equity_ttm = Some(string_to_float(&value.return_on_equity_ttm));
        self.return_on_asset_ttm = Some(string_to_float(&value.return_on_asset_ttm));
        self.ebitda = Some(string_to_int64(value.ebitda));
        self.ev_to_revenue = Some(string_to_float(&value.ev_to_revenue));
        self.ev_to_ebitda = Some(string_to_float(&value.ev_to_ebitda));
        self.shares_outstanding = Some(string_to_int64(value.shares_outstanding));
        self.shares_float = Some(string_to_int64(value.shares_float));
        self.percent_insiders = Some(string_to_float(&value.percent_insiders));
        self.percent_institutions = Some(string_to_float(&value.percent_institutions));

        self.quarterly_earnings_growth_yoy =
            Some(string_to_float(&value.quarterly_earnings_growth_yoy));
        self.quarterly_revenue_growth_yoy =
            Some(string_to_float(&value.quarterly_revenue_growth_yoy));
        self.analyst_target_price = Some(string_to_float(&value.analyst_target_price));
        self.analyst_rating_strong_buy = Some(string_to_int32(value.analyst_rating_strong_buy));
        self.analyst_rating_buy = Some(string_to_int32(value.analyst_rating_buy));
        self.analyst_rating_hold = Some(string_to_int32(value.analyst_rating_hold));
        self.analyst_rating_sell = Some(string_to_int32(value.analyst_rating_sell));
        self.analyst_rating_strong_sell = Some(string_to_int32(value.analyst_rating_strong_sell));

        self.analyst_consensus = Some(self.set_analyst_consensus());

        self.dividend_amt = string_to_float(&value.dividend_per_share);
        self.r#yield = string_to_float(&value.dividend_yield);
        self.ex_div_date = string_to_utc_datetime(&value.ex_dividend_date);
        self.pay_date = string_to_utc_datetime(&value.dividend_date);
        self.pay_ratio = 0.0;

        self.pr_52_wk_high = string_to_decimal(&value.pr_52_wk_high);
        self.pr_52_wk_low = string_to_decimal(&value.pr_52_wk_low);
    }

    pub fn update_etf_from_alpha(&mut self, value: AlphaEtf) {
        debug!("value: {:?}", value);
        self.total_assets = Some(string_to_int64(value.net_assets));
        self.expense_ratio = Some(string_to_float(&value.net_expense_ratio) * 100.0);
        if !value.dividend_yield.is_empty() {
            self.r#yield = string_to_float(&value.dividend_yield) * 100.0;
        } else {
            self.r#yield = 0.0;
        }
    }

    pub fn update_crypto_from_cmc(&mut self, value: CmcCryptoData) {
        // info!("value: {:?}", value.data.get(&self.symbol).unwrap().first());

        self.total_assets = Some(0);
        if let Some(data) = value.data.get(&self.symbol)
            && let Some(sdata) = data.first()
            && let Some(quote) = sdata.quote.get("USD")
            && let Some(market_cap) = quote.market_cap
        {
            self.total_assets = Some(market_cap as i64);
        };
    }

    pub fn update_crypto_realtime(
        &mut self,
        last_updated: DateTime<Utc>,
        quote: CmcCryptoQuote,
    ) -> Result<()> {
        self.total_assets = Some(0);
        if let Some(price) = quote.price
            && let Some(market_cap) = quote.market_cap
        {
            self.total_assets = Some(market_cap as i64);

            if let Some(_pr_date) = self.pr_date {
                if same_date(last_updated, Utc::now()) {
                    self.pr_prev = self.pr_last;
                    self.pr_last = Decimal::from_f64_retain(price).unwrap();
                    self.pr_last = self.pr_last.round_dp(6);
                }
            } else {
                self.pr_date = Some(last_updated);
                self.pr_prev = self.pr_last;
            }
            self.calculate_price_diff()?;
        };
        Ok(())
    }

    // update realtime price for stocks and etfs
    pub fn update_stock_etf_price_realtime(
        &mut self,
        realtime: TiingoTickerRealtime,
    ) -> Result<()> {
        if let Some(_pr_date) = self.pr_date {
            if same_date(realtime.date, Utc::now()) {
                self.pr_prev = self.pr_close;
                self.pr_last = Decimal::from_f64_retain(realtime.tngo_last).unwrap();
                self.pr_last = self.pr_last.round_dp(6);
            }
        } else {
            self.pr_date = Some(realtime.date);
            self.pr_prev = self.pr_close;
        }
        self.calculate_price_diff()?;
        Ok(())
    }

    pub fn update_price_from_history(
        &mut self,
        last_history: TickerHistory,
        prev_history: Option<TickerHistory>,
    ) -> Result<()> {
        self.pr_date = Some(last_history.date);
        self.pr_close = last_history.close;
        self.pr_high = last_history.high;
        self.pr_last = last_history.close;
        self.pr_low = last_history.low;
        self.pr_open = last_history.open;
        if let Some(prev) = prev_history {
            self.pr_prev = prev.close;
        }

        self.calculate_price_diff()?;
        Ok(())
    }

    fn calculate_price_diff(&mut self) -> Result<()> {
        if self.pr_prev == Decimal::ZERO {
            self.pr_diff_amt = self.pr_last;
            self.pr_diff_perc = dec!(100);
        } else {
            self.pr_diff_amt = self.pr_last - self.pr_prev;
            self.pr_diff_perc = self.pr_diff_amt / self.pr_prev * dec!(100);
            self.pr_diff_perc = self.pr_diff_perc.round_dp(2);
        }
        self.pr_diff_amt = self.pr_diff_amt.round_dp(4);

        self.pr_diff_perc_search = self
            .pr_diff_perc
            .to_f64()
            .ok_or_else(|| anyhow::anyhow!("Error calculating price diff percentage:"))?;

        Ok(())
    }

    fn set_analyst_consensus(&mut self) -> String {
        let strong_buy = self.analyst_rating_strong_buy.unwrap_or(0);
        let buy = self.analyst_rating_buy.unwrap_or(0);
        let hold = self.analyst_rating_hold.unwrap_or(0);
        let sell = self.analyst_rating_sell.unwrap_or(0);
        let strong_sell = self.analyst_rating_strong_sell.unwrap_or(0);
        let total = strong_buy + buy + hold + sell + strong_sell;

        let score =
            (strong_buy * 5 + buy * 4 + hold * 3 + sell * 2 + strong_sell) as f32 / total as f32;
        match score {
            s if s >= 4.5 => "Strong Buy".to_string(),
            s if s >= 3.5 => "Buy".to_string(),
            s if s >= 2.5 => "Hold".to_string(),
            s if s >= 1.5 => "Sell".to_string(),
            s if s >= 0.5 => "Strong Sell".to_string(),
            _ => "NA".to_string(),
        }
    }
}
