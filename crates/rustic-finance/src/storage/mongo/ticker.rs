use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rustic_storage::{Repository, SearchCriteria};
use serde_json::json;
use tracing::debug;

use crate::{
    domain::{Ticker, TickerGroup, TickerPeer, dto::ticker_filter::TickerFilter},
    storage::{mongo::{reader::FinanceMongoStorageReader, writer::FinanceMongoStorageWriter}, reader::TickerStorageReader, writer::TickerStorageWriter},
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
        if let Some(industry) = filter.industry {
            criteria = criteria.contains("industry", industry);
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
            criteria = criteria.gte("signals", signals);
        }

        if let Some(cyield) = filter.r#yield
            && cyield > 0.0
        {
            let dec_yield: Decimal = Decimal::from_f32_retain(cyield).unwrap();
            let dec_yield = dec_yield / Decimal::from(100);

            criteria = criteria.gte("yield", dec_yield);
        }

        if let Some(limit) = filter.limit {
            criteria = criteria.limit(limit);
        }

        criteria = criteria.sort_desc("total_assets");

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