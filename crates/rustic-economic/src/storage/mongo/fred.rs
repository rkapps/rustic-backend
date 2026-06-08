use anyhow::Result;
use async_trait::async_trait;
use rustic_storage::{Repository, SearchCriteria};

use crate::{
    domain::EconomicSeries,
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::FredStorageReader,
        writer::FredStorageWriter,
    },
};

#[async_trait]
impl FredStorageReader for EconomicMongoStorageReader {
    async fn get_series(&self, series_id: &str) -> Result<Option<EconomicSeries>> {
        let Ok(repo) = self.manager.economic_series().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        Ok(repo.find_by_id(series_id.to_owned()).await.ok())
    }

    async fn list_active(&self) -> Result<Vec<EconomicSeries>> {
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
    async fn upsert_fred_series(&self, series: EconomicSeries) -> Result<()> {
        let Ok(repo) = self.manager.economic_series().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(series).await
    }
}
