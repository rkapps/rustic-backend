use std::sync::Arc;
use anyhow::Result;
use tracing::debug;
use crate::{domain::TickerNewsEntity, storage::reader::StorageReader};

pub async fn get_ticker_news(
    reader: Arc<dyn StorageReader>,
    symbol: &str,
) -> Result<Vec<TickerNewsEntity>> {
    let news = reader
        .get_ticker_news(symbol)
        .await
        .map_err(|e| anyhow::anyhow!(format!("Get Ticker Groups error: {}", e)))?;

    debug!("Ticker {} news: {}", symbol, news.len());
    let news_entity: Vec<TickerNewsEntity> = news
        .iter()
        .map(|n| {
            let entity = n.clone();
            TickerNewsEntity {
                date: entity.date,
                description: entity.description,
                source: entity.source,
                symbol: entity.symbol,
                title: entity.title,
                url: entity.url,
            }
        })
        .collect();
    Ok(news_entity)
}
