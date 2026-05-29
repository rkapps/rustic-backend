// rustic-economic/src/lib.rs
use anyhow::Result;
use rustic_core::Tool;
use rustic_providers::{BeaClient, CensusClient, FredClient};
use std::sync::Arc;

pub mod bea;
pub mod census;
pub mod fred;

use crate::{
    service::EconomicDataService,
    storage::EconomicStorageManager,
    tools::{bea::BeaDataTool, census::CensusDataTool, fred::FredSeriesTool},
};

pub struct EconomicTools {
    service: Arc<EconomicDataService>,
}

impl EconomicTools {
    pub async fn new(
        mongo_uri: &str,
        mongo_db: &str,
        fred_api_key: &str,
        bea_api_key: &str,
        census_api_key: &str,
    ) -> Result<Self> {
        let storage = EconomicStorageManager::new(mongo_uri, mongo_db).await?;
        let fred = FredClient::new(fred_api_key)?;
        let bea = BeaClient::new(bea_api_key)?;
        let census = CensusClient::new(census_api_key)?;
        let service = EconomicDataService::new(
            Arc::new(storage),
            Arc::new(fred),
            Arc::new(bea),
            Arc::new(census),
        );
        Ok(Self {
            service: Arc::new(service),
        })
    }

    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(FredSeriesTool::new(self.service.clone())),
            Arc::new(BeaDataTool::new(self.service.clone())),
            Arc::new(CensusDataTool::new(self.service.clone())),
        ]
    }
}
