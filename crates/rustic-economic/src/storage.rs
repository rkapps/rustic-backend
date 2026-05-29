use std::sync::Arc;

use anyhow::Result;
use rustic_storage::{
    MongoDatabase, Repository, SearchCriteria, mongo::repository::MongoRepository,
};
use tokio::sync::Mutex;
use crate::domain::{BEA_NIPA_COLLECTION, BEA_REGIONAL_COLLECTION, BeaNipaData, BeaRegionalData, CENSUS_COLLECTION, CensusData, ECONOMIC_SERIES_COLLECTION, EconomicSeries};


#[derive(Debug)]
pub struct EconomicStorageManager {
    db: MongoDatabase,
}

impl EconomicStorageManager {
    pub async fn new(uri: &str, name: &str) -> Result<Self> {
        let mut mdb = MongoDatabase::new(uri, name).await?;
        mdb.register_collection::<String, EconomicSeries>(ECONOMIC_SERIES_COLLECTION.to_owned()).await?;
        mdb.register_collection::<String, BeaNipaData>(BEA_NIPA_COLLECTION.to_owned()).await?;
        mdb.register_collection::<String, BeaRegionalData>(BEA_REGIONAL_COLLECTION.to_owned()).await?;
        mdb.register_collection::<String, CensusData>(CENSUS_COLLECTION.to_owned()).await?;
        Ok(Self { db: mdb })
    }

    // FRED
    pub async fn get_series(&self, series_id: &str) -> Result<Option<EconomicSeries>> {
        let Ok(repo) = self.economic_series().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        Ok(repo.find_by_id(series_id.to_owned()).await.ok())
    }

    pub async fn upsert_series(&self, series: EconomicSeries) -> Result<()> {
        let Ok(repo) = self.economic_series().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(series).await
    }

    pub async fn list_active(&self) -> Result<Vec<EconomicSeries>> {
        let Ok(repo) = self.economic_series().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        let criteria = SearchCriteria::new().eq("active", true);
        repo.find(Some(criteria)).await
    }

    // BEA NIPA
    pub async fn get_bea_nipa(&self, id: &str) -> Result<Option<BeaNipaData>> {
        let Ok(repo) = self.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;
        Ok(repo.find_by_id(id.to_owned()).await.ok())
    }

    pub async fn upsert_bea_nipa(&self, data: BeaNipaData) -> Result<()> {
        let Ok(repo) = self.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(data).await
    }

    // BEA Regional
    pub async fn get_bea_regional(&self, id: &str) -> Result<Option<BeaRegionalData>> {
        let Ok(repo) = self.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaRegional Repository"));
        };
        let mut repo = repo.lock().await;
        Ok(repo.find_by_id(id.to_owned()).await.ok())
    }

    pub async fn upsert_bea_regional(&self, data: BeaRegionalData) -> Result<()> {
        let Ok(repo) = self.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaRegional Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(data).await
    }

    // Census
    pub async fn get_census(&self, id: &str) -> Result<Option<CensusData>> {
        let Ok(repo) = self.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;
        Ok(repo.find_by_id(id.to_owned()).await.ok())
    }

    pub async fn upsert_census(&self, data: CensusData) -> Result<()> {
        let Ok(repo) = self.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(data).await
    }

    // private collection accessors
    async fn economic_series(&self) -> Result<Arc<Mutex<MongoRepository<String, EconomicSeries>>>> {
        self.db.collection::<String, EconomicSeries>(ECONOMIC_SERIES_COLLECTION.to_string()).await
    }

    async fn bea_nipa(&self) -> Result<Arc<Mutex<MongoRepository<String, BeaNipaData>>>> {
        self.db.collection::<String, BeaNipaData>(BEA_NIPA_COLLECTION.to_string()).await
    }

    async fn bea_regional(&self) -> Result<Arc<Mutex<MongoRepository<String, BeaRegionalData>>>> {
        self.db.collection::<String, BeaRegionalData>(BEA_REGIONAL_COLLECTION.to_string()).await
    }

    async fn census(&self) -> Result<Arc<Mutex<MongoRepository<String, CensusData>>>> {
        self.db.collection::<String, CensusData>(CENSUS_COLLECTION.to_string()).await
    }
}