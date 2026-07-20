use std::sync::Arc;

use anyhow::Result;
use rustic_storage::{MongoDatabase, mongo::repository::MongoRepository};
use tokio::sync::Mutex;

use crate::domain::{
    BEA_NIPA_COLLECTION, BEA_REGIONAL_COLLECTION, CENSUS_COLLECTION, FRED_SERIES_COLLECTION,
    bea::{BeaNipa, BeaRegional},
    census::Census,
    fred::FredSeries,
};

#[derive(Debug, Clone)]
pub struct EconomicMongoStorageManager {
    db: MongoDatabase,
}

impl EconomicMongoStorageManager {
    pub async fn new(uri: &str, name: &str) -> Result<Self> {
        let mut mdb = MongoDatabase::new(uri, name).await?;
        mdb.register_collection::<String, FredSeries>(FRED_SERIES_COLLECTION.to_owned())
            .await?;
        mdb.register_collection::<String, BeaNipa>(BEA_NIPA_COLLECTION.to_owned())
            .await?;
        mdb.register_collection::<String, BeaRegional>(BEA_REGIONAL_COLLECTION.to_owned())
            .await?;
        mdb.register_collection::<String, Census>(CENSUS_COLLECTION.to_owned())
            .await?;
        Ok(Self { db: mdb })
    }

    pub async fn economic_series(&self) -> Result<Arc<Mutex<MongoRepository<String, FredSeries>>>> {
        self.db
            .collection::<String, FredSeries>(FRED_SERIES_COLLECTION.to_string())
            .await
    }

    pub async fn bea_nipa(&self) -> Result<Arc<Mutex<MongoRepository<String, BeaNipa>>>> {
        self.db
            .collection::<String, BeaNipa>(BEA_NIPA_COLLECTION.to_string())
            .await
    }

    pub async fn bea_regional(&self) -> Result<Arc<Mutex<MongoRepository<String, BeaRegional>>>> {
        self.db
            .collection::<String, BeaRegional>(BEA_REGIONAL_COLLECTION.to_string())
            .await
    }

    pub async fn census(&self) -> Result<Arc<Mutex<MongoRepository<String, Census>>>> {
        self.db
            .collection::<String, Census>(CENSUS_COLLECTION.to_string())
            .await
    }
}
