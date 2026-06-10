use anyhow::Result;
use rustic_core::Tool;
use rustic_ml::EmbeddingClient;
use rustic_providers::finance::service::ProviderService;
use std::sync::Arc;

#[cfg(feature = "writer")]
use crate::{
    domain::dto::ticker_seed::TickerSeed, storage::mongo::writer::FinanceMongoStorageWriter,
};
#[cfg(feature = "reader")]
use crate::{
    domain::{
        TickerEntity, TickerGroup, TickerNewsEntity,
        dto::{ticker_chart_entity::TickerChartEntity, ticker_search_param::TickerSearchParam},
    },
    storage::mongo::reader::FinanceMongoStorageReader,
};

use crate::{
    storage::mongo::manager::FinanceMongoStorageManager,
    tools::{
        ticker_indicator::TickerIndicatorTool, ticker_peers::TickerPeersTool,
        ticker_price_history::TickerPriceHistoryTool, ticker_screening::TickerScreeningTool,
        ticker_sentiment::TickerSentimentTool, ticker_snapshot::TickerSnapshotTool,
        ticker_taxonomy::TickerTaxonomyTool,
    },
};

pub struct FinanceService {
    reader: Option<Arc<FinanceMongoStorageReader>>,
    writer: Option<Arc<FinanceMongoStorageWriter>>,
    embedding_client: Arc<dyn EmbeddingClient>,
    provider_service: Option<Arc<ProviderService>>,
}

impl FinanceService {
    pub async fn new_reader(
        mongo_uri: &str,
        mongo_db: &str,
        embedding_client: Arc<dyn EmbeddingClient>,
    ) -> Result<Self> {
        let storage = FinanceMongoStorageManager::new(mongo_uri, mongo_db).await?;
        let reader = FinanceMongoStorageReader::new(storage);
        Ok(Self {
            reader: Some(Arc::new(reader)),
            writer: None,
            embedding_client,
            provider_service: None,
        })
    }

    pub async fn new(
        mongo_uri: &str,
        mongo_db: &str,
        embedding_client: Arc<dyn EmbeddingClient>,
        alpha_key: Option<String>,
        tiingo_token: Option<String>,
        coinmarketcap_key: Option<String>,
    ) -> Result<Self> {
        let storage = FinanceMongoStorageManager::new(mongo_uri, mongo_db).await?;
        let alpha_key = alpha_key.expect("alpha key is not set");
        let tiingo_token = tiingo_token.expect("tiingo token is not set");
        let coinmarketcap_key = coinmarketcap_key.expect("coinmarketcap key is not set");
        let provider_service = ProviderService::new(&alpha_key, &tiingo_token, &coinmarketcap_key)?;

        Ok(Self {
            reader: Some(Arc::new(FinanceMongoStorageReader::new(storage.clone()))),
            writer: Some(Arc::new(FinanceMongoStorageWriter::new(storage))),
            embedding_client,
            provider_service: Some(Arc::new(provider_service)),
        })
    }

    #[cfg(feature = "reader")]
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        let reader = self.reader.as_ref().expect("reader not initialized");

        vec![
            Arc::new(TickerPeersTool::new(reader.clone())),
            Arc::new(TickerIndicatorTool::new(reader.clone())),
            Arc::new(TickerScreeningTool::new(
                reader.clone(),
                self.embedding_client.clone(),
            )),
            Arc::new(TickerSnapshotTool::new(reader.clone())),
            Arc::new(TickerTaxonomyTool::new(reader.clone())),
            Arc::new(TickerSentimentTool::new(
                reader.clone(),
                self.embedding_client.clone(),
            )),
            Arc::new(TickerPriceHistoryTool::new(reader.clone())),
        ]
    }

    #[cfg(feature = "reader")]
    pub async fn get_ticker_groups(&self) -> Result<Vec<TickerGroup>> {
        use crate::storage::TickerStorageReader;

        let reader = self.reader.as_ref().expect("reader not initialized");
        reader
            .get_ticker_groups()
            .await
            .map_err(|e| anyhow::anyhow!(format!("Get Ticker Groups error: {}", e)))
    }

    #[cfg(feature = "reader")]
    pub async fn get_ticker_charts(&self, symbol: &str) -> Result<Vec<TickerChartEntity>> {
        use crate::core::tickers::charts::get_ticker_charts;
        let reader = self.reader.as_ref().expect("reader not initialized");
        get_ticker_charts(reader.clone(), symbol).await
    }

    #[cfg(feature = "reader")]
    pub async fn get_ticker_news(&self, symbol: &str) -> Result<Vec<TickerNewsEntity>> {
        use crate::core::tickers::news::get_ticker_news;
        let reader = self.reader.as_ref().expect("reader not initialized");
        get_ticker_news(reader.clone(), symbol).await
    }
    #[cfg(feature = "reader")]
    pub async fn search_tickers(&self, param: TickerSearchParam) -> Result<Vec<TickerEntity>> {
        use crate::core::tickers::search::search_tickers;

        let reader = self.reader.as_ref().expect("reader not initialized");
        search_tickers(reader.clone(), self.embedding_client.clone(), param).await
    }

    // pipeline methods only available with writer
    #[cfg(feature = "writer")]
    pub async fn load_tickers(&self, ticker_seeds: &[TickerSeed], update: bool) -> Result<()> {
        use crate::core::pipeline::load_tickers;

        let reader = self.reader.as_ref().expect("reader not initialized");
        let writer = self.writer.as_ref().expect("writer not initialized");
        let provider_service = self
            .provider_service
            .as_ref()
            .expect("provider service not initialized");

        load_tickers(
            reader.clone(),
            writer.clone(),
            provider_service.clone(),
            self.embedding_client.clone(),
            ticker_seeds,
            update,
        )
        .await
    }

    #[cfg(feature = "writer")]
    pub async fn update_eod_tickers(&self, symbols: &str, update: bool) -> Result<()> {
        use crate::core::pipeline::update_eod_tickers_pipeline;
        let reader = self.reader.as_ref().expect("reader not initialized");
        let writer = self.writer.as_ref().expect("writer not initialized");
        let provider_service = self
            .provider_service
            .as_ref()
            .expect("provider service not initialized");

        update_eod_tickers_pipeline(
            reader.clone(),
            writer.clone(),
            provider_service.clone(),
            self.embedding_client.clone(),
            symbols,
            update,
        )
        .await
    }

    #[cfg(feature = "writer")]
    pub async fn update_ticker_eod_prediction_signals(&self, _symbols: &str) -> Result<()> {
        Ok(())
    }

    #[cfg(feature = "writer")]
    pub async fn update_realtime_stocks_etfs(&self, symbols: &str, update: bool) -> Result<()> {
        use crate::core::pipeline::update_realtime_stocks_etfs_pipeline;

        let reader = self.reader.as_ref().expect("reader not initialized");
        let writer = self.writer.as_ref().expect("writer not initialized");
        let provider_service = self
            .provider_service
            .as_ref()
            .expect("provider service not initialized");

        update_realtime_stocks_etfs_pipeline(
            reader.clone(),
            writer.clone(),
            provider_service.clone(),
            symbols,
            update,
        )
        .await
    }

    #[cfg(feature = "writer")]
    pub async fn update_realtime_cryptos(&self, symbols: &str, update: bool) -> Result<()> {
        use crate::core::pipeline::update_realtime_cryptos_pipeline;
        let reader = self.reader.as_ref().expect("reader not initialized");
        let writer = self.writer.as_ref().expect("writer not initialized");
        let provider_service = self
            .provider_service
            .as_ref()
            .expect("provider service not initialized");

        update_realtime_cryptos_pipeline(
            reader.clone(),
            writer.clone(),
            provider_service.clone(),
            symbols,
            update,
        )
        .await
    }

    #[cfg(feature = "writer")]
    pub async fn update_tickers_news(&self) -> Result<()> {
        use crate::{core::tickers::news::update_tickers_news, storage::TickerStorageReader};

        let writer = self.writer.as_ref().expect("writer not initialized");
        let reader = self.reader.as_ref().expect("reader not initialized");
        let provider_service = self
            .provider_service
            .as_ref()
            .expect("provider service not initialized");

        let all_tickers = reader.get_tickers_by_total_assets().await?;
        update_tickers_news(writer.clone(), provider_service.clone(), all_tickers).await?;
        Ok(())
    }
}
