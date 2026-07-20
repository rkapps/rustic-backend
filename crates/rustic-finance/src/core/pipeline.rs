use crate::{
    core::{
        helper::get_tickers_for_symbols,
        tickers::update::{
            update_all_ticker_overview_embeddings, update_all_ticker_sentiments_embeddings,
            update_all_tickers, update_cryptos_realtime, update_stocks_etfs_realtime,
        },
    },
    domain::{Ticker, TickerControl, dto::ticker_seed::TickerSeed, tickers::AssetType},
    storage::{
        FinanceMongoStorageReader, mongo::writer::FinanceMongoStorageWriter,
        reader::TickerControlStorageReader,
    },
};
use anyhow::Result;
use rustic_ml::EmbeddingClient;
use rustic_providers::finance::service::ProviderService;
use std::{collections::HashMap, sync::Arc};
use tracing::{error, info};

pub async fn load_tickers(
    reader: Arc<FinanceMongoStorageReader>,
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    embedding_client: Arc<dyn EmbeddingClient>,
    ticker_seeds: &[TickerSeed],
    update: bool,
) -> Result<()> {
    info!("Loading tickers: {} update: {}", ticker_seeds.len(), update);

    let all_controls = reader.get_ticker_controls().await?;
    let mut control_map: HashMap<String, TickerControl> = all_controls
        .into_iter()
        .map(|c| (c.symbol.clone(), c))
        .collect();

    let mut all_tickers = Vec::new();
    let mut all_new_controls = Vec::new();
    for seed in ticker_seeds.iter() {
        let tc = control_map
            .remove(&seed.symbol)
            .unwrap_or_else(|| TickerControl::new(seed.clone()));
        let ticker = Ticker::new(seed.clone());
        all_tickers.push(ticker);
        all_new_controls.push(tc);
    }

    update_all_tickers(
        writer.clone(),
        provider_service,
        all_new_controls,
        all_tickers.clone(),
        update,
    )
    .await?;

    // update ticker overview embeddings
    update_all_ticker_overview_embeddings(writer.clone(), embedding_client.clone(), all_tickers)
        .await?;

    Ok(())
}

pub async fn update_eod_tickers_pipeline(
    reader: Arc<FinanceMongoStorageReader>,
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    symbols: &str,
    update: bool,
) -> Result<()> {
    let all_tickers = get_tickers_for_symbols(&reader, symbols).await?;
    let all_controls = reader.get_ticker_controls().await?;
    update_all_tickers(
        writer.clone(),
        provider_service,
        all_controls,
        all_tickers.clone(),
        update,
    )
    .await?;
    Ok(())
}

// update sentiments and embeddings
pub async fn update_eod_tickers_sentiments_embeddings_pipeline(
    reader: Arc<FinanceMongoStorageReader>,
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    embedding_client: Arc<dyn EmbeddingClient>,
    symbols: &str,
    update: bool,
) -> Result<()> {
    let all_tickers = get_tickers_for_symbols(&reader, symbols).await?;
    let all_controls = reader.get_ticker_controls().await?;
    update_all_ticker_sentiments_embeddings(
        reader.clone(),
        writer.clone(),
        provider_service,
        embedding_client.clone(),
        all_controls,
        all_tickers.clone(),
        update,
    )
    .await?;
    Ok(())
}

pub async fn update_realtime_stocks_etfs_pipeline(
    reader: Arc<FinanceMongoStorageReader>,
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    symbols: &str,
    update: bool,
) -> Result<()> {
    let mut all_tickers = get_tickers_for_symbols(&reader, symbols).await?;
    all_tickers.retain(|t| t.asset_type == AssetType::Stock || t.asset_type == AssetType::Etf);

    match update_stocks_etfs_realtime(
        writer.clone(),
        provider_service.clone(),
        all_tickers,
        update,
    )
    .await
    {
        Ok(_) => {}
        Err(e) => error!("Ticker Realtime error: {}", e),
    }

    Ok(())
}

pub async fn update_realtime_cryptos_pipeline(
    reader: Arc<FinanceMongoStorageReader>,
    writer: Arc<FinanceMongoStorageWriter>,
    provider_service: Arc<ProviderService>,
    symbols: &str,
    update: bool,
) -> Result<()> {
    let mut all_tickers = get_tickers_for_symbols(&reader, symbols).await?;
    all_tickers.retain(|t| t.asset_type == AssetType::Crypto);

    update_cryptos_realtime(
        writer.clone(),
        provider_service.clone(),
        all_tickers,
        update,
    )
    .await?;
    Ok(())
}
