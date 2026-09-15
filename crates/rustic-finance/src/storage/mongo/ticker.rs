use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rustic_storage::{Repository, SearchCriteria};
use serde_json::json;
use tracing::debug;

use crate::{
    domain::{Ticker, TickerGroup, TickerPeer, dto::ticker_filter::TickerFilter},
    storage::{
        mongo::{reader::FinanceMongoStorageReader, writer::FinanceMongoStorageWriter},
        reader::TickerStorageReader,
        writer::TickerStorageWriter,
    },
    util::data_utils::assets_cap_label_range,
};

#[async_trait]
impl TickerStorageReader for FinanceMongoStorageReader {
    async fn get_ticker_groups(&self) -> Result<Vec<TickerGroup>> {
        match self.manager.tickers().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;

                let pipeline = vec![
                    json!({ "$group": { "_id": { "sector": "$sector", "industry": "$industry" } } }),
                    json!({ "$match": { "_id.sector": { "$ne": null }, "_id.industry": { "$ne": null } } }),
                ];

                let results = repo.aggregate(pipeline).await?;

                let groups: Vec<TickerGroup> = results
                    .iter()
                    .filter_map(|v| {
                        serde_json::from_value(json!({
                            "sector": v["_id"]["sector"],
                            "industry": v["_id"]["industry"]
                        }))
                        .ok()
                    })
                    .collect();

                Ok(groups)
            }
            Err(e) => Err(anyhow::anyhow!("Error getting Ticker: {}", e)),
        }
    }

    async fn get_tickers_by_total_assets(&self) -> Result<Vec<Ticker>> {
        let criteria = SearchCriteria::new().sort_desc("total_assets");
        self.manager.get_ticker_by_criteria(&criteria).await
    }

    async fn get_tickers_by_symbols(&self, symbols: Vec<String>) -> Result<Vec<Ticker>> {
        let criteria = SearchCriteria::new()
            .in_values("symbol", symbols)
            .sort_asc("symbol");
        debug!("get_tickers_by_symbols: {:#?}", criteria);
        self.manager.get_ticker_by_criteria(&criteria).await
    }

    async fn get_tickers_by_top_gainers(&self, asset_type: Option<String>) -> Result<Vec<Ticker>> {
        let mut criteria = SearchCriteria::new().sort_desc("pr_diff_perc").limit(20);
        if let Some(asset_type) = asset_type {
            criteria = criteria.eq("asset_type", asset_type.to_uppercase());
        }
        self.manager.get_ticker_by_criteria(&criteria).await
    }
    async fn get_tickers_by_top_gainers_ytd(
        &self,
        asset_type: Option<String>,
    ) -> Result<Vec<Ticker>> {
        let mut criteria = SearchCriteria::new()
            .sort_desc("performance_search.Ytd.perc")
            .limit(20);
        if let Some(asset_type) = asset_type {
            criteria = criteria.eq("asset_type", asset_type.to_uppercase());
        }
        self.manager.get_ticker_by_criteria(&criteria).await
    }

    async fn get_tickers_by_top_losers(&self, asset_type: Option<String>) -> Result<Vec<Ticker>> {
        let mut criteria = SearchCriteria::new().sort_asc("pr_diff_perc").limit(20);
        if let Some(asset_type) = asset_type {
            criteria = criteria.eq("asset_type", asset_type.to_uppercase());
        }
        self.manager.get_ticker_by_criteria(&criteria).await
    }
    async fn get_tickers_by_top_losers_ytd(
        &self,
        asset_type: Option<String>,
    ) -> Result<Vec<Ticker>> {
        let mut criteria = SearchCriteria::new()
            .sort_asc("performance_search.Ytd.perc")
            .limit(20);
        if let Some(asset_type) = asset_type {
            criteria = criteria.eq("asset_type", asset_type.to_uppercase());
        }
        self.manager.get_ticker_by_criteria(&criteria).await
    }

    async fn get_ticker_peers_by_symbols(
        &self,
        symbols: Vec<String>,
        limit: usize,
    ) -> Result<Vec<TickerPeer>> {
        debug!("symbols: {:?}", symbols);
        match self.manager.tickers().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;

                let pipeline = vec![
                    json!({ "$match": { "symbol": { "$in": &symbols } } }),
                    json!({ "$lookup": {
                        "from": "ticker",
                        "let": { "sec": "$sector", "ind": "$industry", "sym": "$symbol" },
                        "pipeline": [
                            { "$match": { "$expr": { "$and": [
                                { "$eq": ["$sector", "$$sec"] },
                                { "$ne": ["$symbol", "$$sym"] }
                            ]}}},
                            { "$addFields": {
                                "score": { "$cond": [
                                    { "$eq": ["$industry", "$$ind"] },
                                    2,
                                    1
                                ]}
                            }},
                            { "$sort": { "score": -1 } },
                            { "$limit": limit }
                        ],
                        "as": "peer_docs"
                    }}),
                    json!({ "$project": {
                        "symbol": 1,
                        "peers": {
                            "$map": {
                                "input": "$peer_docs",
                                "as": "peer",
                                "in": {
                                    "symbol": "$$peer.symbol",
                                    "score": "$$peer.score"
                                }
                            }
                        },
                        "_id": 0
                    }}),
                ];

                let results = repo.aggregate(pipeline).await?;
                let peers: Vec<TickerPeer> = results
                    .iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect();
                Ok(peers)
            }
            Err(e) => Err(anyhow::anyhow!("Error getting Ticker: {}", e)),
        }
    }

    async fn search_tickers(&self, filter: TickerFilter) -> Result<Vec<Ticker>> {
        let mut criteria = SearchCriteria::new();
        if let Some(industries) = filter.industries {
            criteria = criteria.in_values("industry", industries);
        }
    
        let new_asset_type = filter
            .asset_type
            .unwrap_or_else(|| "stock".to_string())
            .to_uppercase();
    
        criteria = criteria.eq("asset_type", new_asset_type);
        if let Some(range) = filter.assets_cap_range {
            let (min_cap, max_cap) = assets_cap_label_range(Some(range));
            criteria = criteria.gte("total_assets", min_cap);
            criteria = criteria.lte("total_assets", max_cap);
        }
        if let Some(signals) = filter.signals {
            criteria = criteria.all_of("signals", signals);
        }
    
    
        // --- new: generic metric filters ---
        if let Some(metric_filters) = filter.metric_filters {
            for mf in metric_filters {
                let field = map_metric_field(&mf.field)?;
        
                let dec_value = if field == "yield" {
                    let dec: Decimal = Decimal::from_f64_retain(mf.value)
                        .ok_or_else(|| anyhow::anyhow!("Invalid metric_filter value: {}", mf.value))?;
                    dec / Decimal::from(100)
                } else {
                    Decimal::from_f64_retain(mf.value)
                        .ok_or_else(|| anyhow::anyhow!("Invalid metric_filter value: {}", mf.value))?
                };
        
                match mf.op.as_str() {
                    "gt" | "gte" => criteria = criteria.gte(&field, dec_value),
                    "lt" | "lte" => criteria = criteria.lte(&field, dec_value),
                    _ => return Err(anyhow::anyhow!("Unsupported metric_filter op: {}", mf.op)),
                }
            }
        }
    
        if let Some(limit) = filter.limit {
            criteria = criteria.limit(limit);
        }
    
        // --- new: real sort, falling back to existing default ---
        match (filter.sort_by.as_deref(), filter.sort_dir.as_deref()) {
            (Some(field), dir) => {
                let mapped = map_sort_field(field)?; // validate against allow-list
                if dir == Some("asc") {
                    criteria = criteria.sort_asc(&mapped);
                } else {
                    criteria = criteria.sort_desc(&mapped);
                }
            }
            (None, _) => {
                criteria = criteria.sort_desc("total_assets");
            }
        }
    
        debug!("search_tickers criteria: {:#?}", criteria);
    
        self.manager.get_ticker_by_criteria(&criteria).await
    }
    
}

#[async_trait]
impl TickerStorageWriter for FinanceMongoStorageWriter {
    async fn save_tickers(&self, tickers: Vec<Ticker>) -> Result<()> {
        match self.manager.tickers().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.bulk_update(tickers).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!(format!("Error saving Ticker: {}", e)));
            }
        }
    }
}


fn map_metric_field(field: &str) -> Result<String> {
    match field {
        "pe_ratio" | "forward_pe" | "peg_ratio" | "pb_ratio" | "ps_ratio"
        | "eps" | "beta" | "ev_to_ebitda" | "profit_margin" | "return_on_equity_ttm"
        | "yield" => {
            Ok(field.to_string())
        }
        _ => Err(anyhow::anyhow!("Unsupported metric_filter field: {}", field)),
    }
}

fn map_sort_field(field: &str) -> Result<String> {
    match field {
        "performance_1w" | "performance_1m" | "performance_6m" | "performance_ytd"
        | "pe_ratio" | "eps" | "beta" | "total_assets" | "yield" | "change_perc" => {
            Ok(field.to_string())
        }
        _ => Err(anyhow::anyhow!("Unsupported sort_by field: {}", field)),
    }
}
