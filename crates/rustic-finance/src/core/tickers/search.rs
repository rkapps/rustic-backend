use std::sync::Arc;

use anyhow::Result;
use rustic_ml::{EmbeddingClient, search};
use tracing::debug;

use crate::{
    domain::{
        Ticker, TickerEntity,
        dto::{ticker_filter::TickerFilter, ticker_search_param::TickerSearchParam},
    },
    storage::reader::StorageReader,
    util::data_utils::get_overview_embeddings,
};

pub async fn search_tickers(
    reader: Arc<dyn StorageReader>,
    embedding_client: Arc<dyn EmbeddingClient>,
    param: TickerSearchParam,
) -> Result<Vec<TickerEntity>> {
    // let mut tickers = Vec::new();
    debug!("Search Param: {:#?}", param);
    let tickers: Vec<Ticker> = if let Some(symbols) = param.symbols {
        let list: Vec<String> = symbols.split(',').map(|s| s.to_string()).collect();
        debug!("List: {:?}", list);
        reader.get_tickers_by_symbols(list).await?
    } else if let Some(function) = param.function {
        match function.as_str() {
            "top_gainers" => reader.get_tickers_by_top_gainers(param.asset_type).await?,
            "top_gainers_ytd" => {
                reader
                    .get_tickers_by_top_gainers_ytd(param.asset_type)
                    .await?
            }
            "top_losers" => reader.get_tickers_by_top_losers(param.asset_type).await?,
            "top_losers_ytd" => {
                reader
                    .get_tickers_by_top_losers_ytd(param.asset_type)
                    .await?
            }
            _ => Vec::new(),
        }
    } else {
        let filter = TickerFilter::from(param.clone());
        let tickers = reader
            .search_tickers(filter)
            .await
            .map_err(|e| anyhow::anyhow!(format!("Get Ticker error: {}", e)))?;
        debug!("Tickers from storage: {}", tickers.len());
        search_tickers_by_overview_embedding(embedding_client, &tickers, param.query, param.score)
            .await?
    };

    debug!("Tickers: {}", tickers.len());
    let tentities = tickers
        .iter()
        .map(|t| TickerEntity::from(t.clone()))
        .collect();
    Ok(tentities)
}

pub async fn search_tickers_by_overview_embedding(
    embedding_client: Arc<dyn EmbeddingClient>,
    tickers: &[Ticker],
    query: Option<String>,
    score: Option<f32>,
) -> Result<Vec<Ticker>> {
    let tickers = if let Some(query) = query
        && !tickers.is_empty()
    {
        let score = score.unwrap_or_default();
        let overview_candidates: Vec<(Ticker, Vec<f32>)> = get_overview_embeddings(tickers);
        let vectors = embedding_client.embed_text(&query).await?.into_vec();

        debug!(
            "Overview candidates: {}, dimensions: {}",
            overview_candidates.len(),
            overview_candidates.first().unwrap().1.len()
        );
        debug!("Query: {:?} dimensions: {}", query, vectors.len());

        search(&vectors, &overview_candidates, 20)
            .into_iter()
            .filter_map(|(t, s)| {
                debug!("Ticker: {} score: {}", t.symbol, s);
                if s > score { Some(t.clone()) } else { None }
            })
            .collect()
    } else {
        tickers.to_vec()
    };
    Ok(tickers)
}
