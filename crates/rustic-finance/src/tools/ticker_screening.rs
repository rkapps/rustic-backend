use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use rustic_ml::{EmbeddingClient, search};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, info};

use crate::{
    domain::{Ticker, dto::ticker_filter::TickerFilter},
    storage::reader::StorageReader,
    util::data_utils::get_overview_embeddings,
};

#[derive(Debug)]
pub struct TickerScreeningTool {
    storage_service: Arc<dyn StorageReader>,
    embedding_client: Arc<dyn EmbeddingClient>,
}
impl TickerScreeningTool {
    pub fn new(
        storage_service: Arc<dyn StorageReader>,
        embedding_client: Arc<dyn EmbeddingClient>,
    ) -> TickerScreeningTool {
        Self {
            storage_service,
            embedding_client,
        }
    }
}

#[async_trait]
impl Tool for TickerScreeningTool {
    fn name(&self) -> String {
        "ticker_screening".to_string()
    }

    fn description(&self) -> String {
        "Finds and filters stocks or ETFs based on any combination of semantic query, technical signals, industry, market cap, and asset type. \
            ALWAYS use this tool when the user asks to find, compare, or screen stocks by theme, category, or condition. \
            Do not use ticker_peers for theme or category queries — use this tool instead. \
        \
            Always call ticker_taxonomy before this tool when the query mentions a specific industry or sector. \
        \
            Query guidelines: \
                - Pass the user's original query text unchanged. Never replace with generic terms like 'find stocks'. \
                - Only include total_assets_range if the user explicitly mentions a market cap size or total assets size. Never infer it. \
                - When the query implies a specific industry, populate both query and industry fields. \
        \
        Examples: \
            'software infrastructure mid cap stocks' → query: 'software infrastructure', industry: 'Software - Infrastructure', assets_cap_range: 'mid' \
            'find oversold cloud security companies' → query: 'cloud security', signals: ['RSI Oversold'] \
            'mostly oversold' or 'heavily oversold'  → signals: ['Deeply Oversold'] \
            'defensive buy rated stocks' → signals: ['Low Beta', 'Analyst Buy'] \
            'compare spider ETFs' → query: 'SPDR ETFs', asset_type: 'etf' \
            'find golden cross stocks' or 'stocks in a golden cross' → signals: ['Golden Cross Active'] \
            'stocks that just crossed golden cross' → signals: ['Golden Cross'] \
            'find low P/E semiconductor stocks' → industries: ['Semiconductors'], metric_filters: [{field: 'pe_ratio', op: 'lt', value: 15}] \
                'stocks with a large dividend' → metric_filters: [{field: 'yield', op: 'gt', value: 4}] \
            \
            Signal rules: \
                Pass ONE signal per category. Results must match ALL signals provided. \
                Categories: \
                    Trend: 'Golden Cross', 'Death Cross' (crossover just happened), \
                    'Golden Cross Active', 'Death Cross Active' (currently in that trend, may have crossed days/weeks ago), \
                    'Above SMA50', 'Below SMA50'. \
                    Momentum: 'MACD Bullish Crossover', 'MACD Bearish Crossover'. \
                    RSI: 'RSI Oversold', 'RSI Overbought', 'Deeply Oversold', 'Moderately Oversold', 'Deeply Overbought', 'Moderately Overbought'. \
                    Bands: 'BB Breakout Upper', 'BB Breakout Lower', 'BB Squeeze'. \
                    Stochastic: 'Stochastic Bullish', 'Stochastic Bearish'. \
                    Analyst: 'Analyst Strong Buy', 'Analyst Buy', 'Analyst Hold', 'Analyst Sell', 'Analyst Strong Sell'. \
                    Beta: 'Low Beta', 'Market Beta', 'High Beta', 'Very High Beta'."        
                .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Semantic search query for business theme or category. Example: 'software infrastructure', 'cloud security', 'payments processing'."
                },
                "signals": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": [
                            "SMA Stack Bullish", "SMA Stack Bearish",
                            "Bullish Pullback", "Bearish Rally",
                            "Above SMA50", "Below SMA50",
                            "Golden Cross", "Death Cross",
                            "Golden Cross Active", "Death Cross Active",
                            "MACD Bullish Crossover", "MACD Bearish Crossover",
                            "MACD Histogram Expanding", "MACD Histogram Weakening",
                            "BB Breakout Upper", "BB Breakout Lower", "BB Squeeze",
                            "Stochastic Bullish", "Stochastic Bearish",
                            "RSI Oversold", "RSI Multi-period Oversold",
                            "RSI Overbought", "RSI Multi-period Overbought",
                            "Oversold Confluence", "Oversold Reversal Setup",
                            "Overbought Confluence", "Overbought Reversal Setup",
                            "Deeply Oversold", "Moderately Oversold",
                            "Deeply Overbought", "Moderately Overbought",
                            "Mean Reversion Candidate",
                            "Momentum Breakout", "Bullish Trend Exhaustion", "Bearish Trend Exhaustion",
                            "Volatility Expanding", "Volatility Contracting",
                            "Analyst Strong Buy", "Analyst Buy", "Analyst Hold",
                            "Analyst Sell", "Analyst Strong Sell",
                            "Low Beta", "Market Beta", "High Beta", "Very High Beta"

                            // --- ML signals (deferred, not yet computed) ---
                            // "ML Strong Bull — All Periods All Models Confirmed",
                            // "ML Strong Bull — All Periods Confirmed",
                            // "ML Strong Bull — All Periods LR+MLP Confirmed",
                            // "ML Strong Bear — All Periods All Models Confirmed",
                            // "ML Strong Bear — All Periods Confirmed",
                            // "ML Strong Bear — All Periods LR+MLP Confirmed",
                            // "ML5 Bullish — 2/2 Confirmed",  "ML5 Bullish — 3/3 Confirmed",
                            // "ML5 Bearish — 2/2 Confirmed",  "ML5 Bearish — 3/3 Confirmed",
                            // "ML10 Bullish — 2/2 Confirmed", "ML10 Bullish — 3/3 Confirmed",
                            // "ML10 Bearish — 2/2 Confirmed", "ML10 Bearish — 3/3 Confirmed",
                            // "ML20 Bullish — 2/2 Confirmed", "ML20 Bullish — 3/3 Confirmed",
                            // "ML20 Bearish — 2/2 Confirmed", "ML20 Bearish — 3/3 Confirmed",
                            // "ML60 Bullish — 2/2 Confirmed", "ML60 Bullish — 3/3 Confirmed",
                            // "ML60 Bearish — 2/2 Confirmed", "ML60 Bearish — 3/3 Confirmed",
                            // "MLP5 Bullish",  "MLP5 Bearish",
                            // "MLP10 Bullish", "MLP10 Bearish",
                            // "MLP20 Bullish", "MLP20 Bearish",
                            // "MLP60 Bullish", "MLP60 Bearish",
                        ]
                    },
                    "description": "Filter by active technical or analyst signals."
                },                
                "industries": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "One or more industries to filter by. Must be exact industry names as
                        returned by ticker_taxonomy — do not paraphrase or guess industry names. For a
                        specific sub-category, pass one industry. For a broad sector-level request, pass
                        every industry under that sector from the taxonomy result, all in one call."
                },
                "assets_cap_range": {
                    "type": "string",
                    "enum": ["mega", "large", "mid", "small"],
                    "description": "Filter by assets cap. mega: >$1T, large: $100B-$1T, mid: $2B-$100B, small: <$2B. \
                            Map: 'mid cap' → 'mid', 'large cap' → 'large', 'small cap' → 'small', 'mega cap' → 'mega'."
                },
                "asset_type": {
                    "type": "string",
                    "enum": ["stock", "etf"],
                    "description": "Filter by asset type. Defaults to 'stock'."
                },
                "metric_filters": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "field": {
                                "type": "string",
                                "enum": [
                                    "pe_ratio", "forward_pe", "peg_ratio", "pb_ratio", "ps_ratio",
                                    "eps", "beta", "ev_to_ebitda", "profit_margin",
                                    "return_on_equity_ttm", "yield"
                                ],
                                "description": "The metric to filter on."
                            },
                            "op": {
                                "type": "string",
                                "enum": ["gt", "gte", "lt", "lte"],
                                "description": "Comparison operator."
                            },
                            "value": {
                                "type": "number",
                                "description": "Threshold value. For 'yield', use human-scale percentage (e.g. 4 for 4%, not 0.04)."
                            }
                        },
                        "required": ["field", "op", "value"]
                    },
                    "description": "Filter by a numeric threshold on a fundamental or valuation metric. Use this for any 'high/low X' or 'X above/below N' language on a metric not otherwise covered by a dedicated field. Examples: 'low P/E' → {field: 'pe_ratio', op: 'lt', value: 15}, 'high EPS' → {field: 'eps', op: 'gt', value: 5}, 'beta above 2' → {field: 'beta', op: 'gt', value: 2}, 'large dividend' → {field: 'yield', op: 'gt', value: 4}. Infer a reasonable threshold when the user doesn't give an exact number; use their number exactly when they do."
                },               
                "limit": {
                    "type": "integer",
                    "description": "Max number of results to return. Defaults to 10."
                },
                "sort_by": {
                    "type": "string",
                    "enum": [
                        "performance_1w", "performance_1m", "performance_3m", "performance_6m",
                        "performance_ytd", "pe_ratio", "eps", "beta", "total_assets", "yield", "change_perc"
                    ],
                    "description": "Sort results by this field. Use for ranking/superlative language like \
                        'best performing', 'outperformed', 'biggest gainers', 'highest yield' — not for \
                        threshold filtering (use metric_filters for that)."
                },
                "sort_dir": {
                    "type": "string",
                    "enum": ["asc", "desc"],
                    "description": "Sort direction. 'desc' for 'best/highest/outperformed', 'asc' for \
                        'worst/lowest/underperformed'. Defaults to 'desc' if omitted."
                },                
            }
        })
    }

    async fn execute(&self, value: serde_json::Value) -> Result<Value> {
        let filter: TickerFilter = serde_json::from_value(value.clone())
            .map_err(|e| anyhow::anyhow!("Failed to deserialize params: {:?} — {:?}", value, e))?;

        let start = std::time::Instant::now();

        info!("Ticker screening filter: {:?}", filter);
        let tickers = self.storage_service.search_tickers(filter.clone()).await?;
        debug!("Screened stocks from initial search: {}", tickers.len());

        let overview_candidates: Vec<(Ticker, Vec<f32>)> = get_overview_embeddings(&tickers);
        debug!("Overview candidates: {}", overview_candidates.len());
        let limit = filter.limit.unwrap_or(10);

        let symbols: Vec<String> = if let Some(query) = filter.query {
            let vectors = self.embedding_client.embed_text(&query).await?.into_vec();

            let candidates: Vec<(String, Vec<f32>)> = overview_candidates
                .iter()
                .map(|(t, e)| (t.symbol.clone(), e.clone()))
                .collect();

            search(&vectors, &candidates, limit)
                .into_iter()
                .map(|(s, _)| s)
                .collect()
        } else {
            tickers.into_iter().take(limit).map(|t| t.symbol).collect()
        };
        let elapsed = start.elapsed();
        info!(
            "Symbols: {:?}  {:.1}s",
            symbols,
            elapsed.as_secs_f32()
        );
        Ok(if symbols.is_empty() {
            json!({ "symbols": null })
        } else {
            json!({ "symbols": symbols })
        })
    }
}
