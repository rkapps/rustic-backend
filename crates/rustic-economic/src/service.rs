use anyhow::Result;
use rustic_core::Tool;
#[cfg(feature = "writer")]
use rustic_providers::economic::bea::model::BeaParamValue;
use std::sync::Arc;

use rustic_providers::{BeaClient, CensusClient, FredClient};

use crate::domain::config::EconomicConfig;
use crate::storage::mongo::reader::EconomicMongoStorageReader;
use crate::storage::mongo::writer::EconomicMongoStorageWriter;
use crate::{
    core::{bea::update_bea_nipa, census::update_census, fred::update_fred_series},
    storage::mongo::manager::EconomicMongoStorageManager,
};

#[derive(Debug, Clone)]
pub struct EconomicService {
    reader: Option<Arc<EconomicMongoStorageReader>>,
    writer: Option<Arc<EconomicMongoStorageWriter>>,
    fred: Option<Arc<FredClient>>,
    bea: Option<Arc<BeaClient>>,
    census: Option<Arc<CensusClient>>,
    pub config: EconomicConfig
}

impl EconomicService {
    pub async fn new_reader(mongo_uri: &str, mongo_db: &str, config: EconomicConfig) -> Result<Self> {
        let storage = EconomicMongoStorageManager::new(mongo_uri, mongo_db).await?;
        Ok(Self {
            reader: Some(Arc::new(EconomicMongoStorageReader::new(storage))),
            writer: None,
            fred: None,
            bea: None,
            census: None,
            config
        })
    }

    pub async fn new(
        mongo_uri: &str,
        mongo_db: &str,
        fred_api_key: Option<String>,
        bea_api_key: Option<String>,
        census_api_key: Option<String>,
        config: EconomicConfig
    ) -> Result<Self> {
        let storage = EconomicMongoStorageManager::new(mongo_uri, mongo_db).await?;
        let fred_api_key = fred_api_key.expect("fred api key is not set");
        let bea_api_key = bea_api_key.expect("bea api key is not set");
        let census_api_key = census_api_key.expect("census api key is not set");
        Ok(Self {
            reader: Some(Arc::new(EconomicMongoStorageReader::new(storage.clone()))),
            writer: Some(Arc::new(EconomicMongoStorageWriter::new(storage))),
            fred: Some(Arc::new(FredClient::new(fred_api_key)?)),
            bea: Some(Arc::new(BeaClient::new(bea_api_key)?)),
            census: Some(Arc::new(CensusClient::new(census_api_key)?)),
            config
        })
    }

    #[cfg(feature = "reader")]
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        use crate::tools::{bea_nipa::BeaNipaDataTool, bea_regional::BeaRegionalDataTool, census::CensusDataTool, fred::FredDataTool, taxonomy::EconomicTaxonomyTool};
        let reader = self.reader.as_ref().expect("reader not initialized");

        vec![
            Arc::new(EconomicTaxonomyTool::new(self.config.clone())),
            Arc::new(BeaNipaDataTool::new(reader.clone())),
            Arc::new(BeaRegionalDataTool::new(reader.clone())),
            Arc::new(CensusDataTool::new(reader.clone())),
            Arc::new(FredDataTool::new(reader.clone())),
        ]
    }

    // ── CLEAN ──────────────────────────────────────────────────────────────
    #[cfg(feature = "writer")]
    pub async fn clean_fred(&self) -> Result<()> {
        use crate::storage::writer::FredStorageWriter;

        let writer = self.writer.as_ref().expect("writer not initialized");
        writer.delete_all_fred_series().await
    }

    #[cfg(feature = "writer")]
    pub async fn clean_bea(&self) -> Result<()> {
        use crate::storage::writer::BeaStorageWriter;

        let writer = self.writer.as_ref().expect("writer not initialized");
        writer.delete_all_bea_nipa().await?;
        writer.delete_all_bea_regional().await?;
        Ok(())
    }

    #[cfg(feature = "writer")]
    pub async fn clean_census(&self) -> Result<()> {
        use crate::storage::writer::CensusStorageWriter;

        let writer = self.writer.as_ref().expect("writer not initialized");
        writer.delete_all_census().await
    }

    #[cfg(feature = "writer")]
    pub async fn get_geo_fips(&self) -> Result<Vec<BeaParamValue>> {
        let bea = self.bea.as_ref().expect("bea client not initialized");
        bea.get_geo_fips().await
    }

    #[cfg(feature = "writer")]
    pub async fn update_fred_series(
        &self,
        series_id: &str,
        frequency: &str,
        limit: usize,
        name: &str,
        category: &str,
    ) -> Result<()> {
        let writer = self.writer.as_ref().expect("writer not initialized");
        let fred = self.fred.as_ref().expect("fred client not initialized");

        update_fred_series(
            writer.clone(),
            fred.clone(),
            series_id,
            frequency,
            limit,
            name,
            category,
        )
        .await
    }

    #[cfg(feature = "writer")]
    pub async fn update_bea_nipa(
        &self,
        table_name: &str,
        frequency: &str,
        year: &str,
    ) -> Result<()> {
        let writer = self.writer.as_ref().expect("writer not initialized");
        let bea = self.bea.as_ref().expect("bea client not initialized");

        update_bea_nipa(writer.clone(), bea.clone(), table_name, frequency, year).await
    }

    #[cfg(feature = "writer")]
    pub async fn update_bea_regional(
        &self,
        code: &str,
        line_code: &str,
        geo_fips: &str,
        year: &str,
    ) -> Result<()> {
        use crate::core::bea::update_bea_regional;

        let writer = self.writer.as_ref().expect("writer not initialized");
        let bea = self.bea.as_ref().expect("bea client not initialized");

        update_bea_regional(writer.clone(), bea.clone(), code, line_code, geo_fips, year).await
    }

    #[cfg(feature = "writer")]
    pub async fn update_census(
        &self,
        dataset: &str,
        variables: &[&str],
        years: &[String],
    ) -> Result<()> {
        let writer = self.writer.as_ref().expect("writer not initialized");
        let census = self.census.as_ref().expect("census client not initialized");

        update_census(writer.clone(), census.clone(), dataset, variables, years).await
    }
}
