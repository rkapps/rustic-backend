use anyhow::Result;
use async_trait::async_trait;
use rustic_storage::{Repository, SearchCriteria};

use crate::{
    domain::fred::FredSeries,
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::FredStorageReader,
        writer::FredStorageWriter,
    },
};

#[async_trait]
impl FredStorageReader for EconomicMongoStorageReader {
    async fn get_series(&self, series_id: &str) -> Result<FredSeries> {
        match self.manager.economic_series().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find_by_id(series_id.to_string()).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting CensusData: {}", e));
            }
        }
    }

    async fn list_active(&self) -> Result<Vec<FredSeries>> {
        let Ok(repo) = self.manager.economic_series().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        let criteria = SearchCriteria::new().eq("active", true);

        repo.find(Some(criteria)).await
    }
}

#[async_trait]
impl FredStorageWriter for EconomicMongoStorageWriter {
    async fn delete_all_fred_series(&self) -> Result<()> {
        let Ok(repo) = self.manager.economic_series().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.delete_many(Some(SearchCriteria::new())).await?;
        Ok(())
    }
    async fn upsert_fred_series(&self, series: FredSeries) -> Result<()> {
        let Ok(repo) = self.manager.economic_series().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(series).await
    }
}
