use crate::{
    domain::{BeaNipaData, BeaRegionalData},
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::BeaStorageReader,
        writer::BeaStorageWriter,
    },
};
use anyhow::Result;
use async_trait::async_trait;
use rustic_storage::{Repository, SearchCriteria};
use tracing::debug;

#[async_trait]
impl BeaStorageReader for EconomicMongoStorageReader {
    async fn get_bea_nipa(&self, id: &str) -> Result<Option<BeaNipaData>> {
        let Ok(repo) = self.manager.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;
        Ok(repo.find_by_id(id.to_owned()).await.ok())
    }

    async fn get_bea_regional(&self, id: &str) -> Result<Option<BeaRegionalData>> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaRegional Repository"));
        };
        let mut repo = repo.lock().await;
        Ok(repo.find_by_id(id.to_owned()).await.ok())
    }

    async fn get_bea_nipa_by_table(
        &self,
        table_name: &str,
        year: &str,
    ) -> Result<Vec<BeaNipaData>> {
        let Ok(repo) = self.manager.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;

        let criteria = SearchCriteria::new()
            .eq("table_name", table_name)
            .eq("time_period", year);
        repo.find(Some(criteria)).await
    }

    async fn get_bea_regional_by_table(
        &self,
        table_name: &str,
        year: &str,
    ) -> Result<Vec<BeaRegionalData>> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaRegional Repository"));
        };
        let mut repo = repo.lock().await;

        let criteria = SearchCriteria::new()
            .eq("code", table_name)
            .eq("time_period", year);

        repo.find(Some(criteria)).await
    }

    async fn get_bea_regional_filtered(
        &self,
        table_name: &str,
        geo_fips: Option<&str>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        year: &str,
    ) -> Result<Vec<BeaRegionalData>> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaRegional Repository"));
        };
        let mut repo = repo.lock().await;

        // use contains till the table name feld is added. code has tablename + linecode
        let mut criteria = SearchCriteria::new()
            .contains("code", table_name)
            .eq("time_period", year);

        if let Some(fips) = geo_fips {
            criteria = criteria.eq("geo_fips", fips);
        }
        if let Some(gt) = geo_type {
            criteria = criteria.eq("geo_type", gt);
        }
        if let Some(prefix) = state_prefix {
            criteria = criteria.starts_with("geo_fips", prefix);
        }
        debug!("get_bea_regional_filtered SearchCriteria: {:#?}", criteria);
        repo.find(Some(criteria)).await
    }
}

#[async_trait]
impl BeaStorageWriter for EconomicMongoStorageWriter {
    async fn delete_all_bea_nipa(&self) -> Result<()> {
        let Ok(repo) = self.manager.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.delete_many(Some(SearchCriteria::new())).await?;
        Ok(())
    }

    async fn upsert_bea_nipa(&self, data: BeaNipaData) -> Result<()> {
        let Ok(repo) = self.manager.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(data).await
    }

    async fn upsert_bea_nipa_bulk(&self, datas: Vec<BeaNipaData>) -> Result<()> {
        let Ok(repo) = self.manager.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;
        repo.bulk_update(datas).await
    }

    // BEA Regional
    async fn delete_all_bea_regional(&self) -> Result<()> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.delete_many(Some(SearchCriteria::new())).await?;
        Ok(())
    }

    async fn upsert_bea_regional_bulk(&self, datas: Vec<BeaRegionalData>) -> Result<()> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaRegional Repository"));
        };
        let mut repo = repo.lock().await;
        repo.bulk_update(datas).await
    }
}
