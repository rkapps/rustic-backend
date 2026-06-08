use anyhow::Result;
use async_trait::async_trait;
use rustic_storage::{Repository, SearchCriteria};
use tracing::debug;

use crate::{
    domain::CensusData,
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::CensusStorageReader,
        writer::CensusStorageWriter,
    },
};

#[async_trait]
impl CensusStorageReader for EconomicMongoStorageReader {
    async fn get_census(&self, id: &str) -> Result<Option<CensusData>> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;
        Ok(repo.find_by_id(id.to_owned()).await.ok())
    }

    async fn get_census_filtered(
        &self,
        dataset: &str,
        variable: &str,
        geo_fips: Option<&str>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        year: &str,
    ) -> Result<Vec<CensusData>> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;
        let mut criteria = SearchCriteria::new()
            .eq("dataset", dataset)
            .eq("year", year)
            .eq("variable", variable);

        if let Some(fips) = geo_fips {
            criteria = criteria.eq("geo_fips", fips);
        }
        if let Some(gt) = geo_type {
            criteria = criteria.eq("geo_type", gt);
        }
        if let Some(prefix) = state_prefix {
            criteria = criteria.starts_with("geo_fips", prefix);
        }
        debug!("get_census_filtered SearchCriteria: {:#?}", criteria);

        repo.find(Some(criteria)).await
    }
}

#[async_trait]
impl CensusStorageWriter for EconomicMongoStorageWriter {
    async fn delete_all_census(&self) -> Result<()> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.delete_many(Some(SearchCriteria::new())).await?;
        Ok(())
    }
    async fn upsert_census_bulk(&self, datas: Vec<CensusData>) -> Result<()> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;
        repo.bulk_update(datas).await
    }

    async fn upsert_census(&self, data: CensusData) -> Result<()> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(data).await
    }
}
