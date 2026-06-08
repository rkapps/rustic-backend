use crate::domain::{BeaNipaData, BeaRegionalData, CensusData, EconomicSeries};
use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
pub trait StorageReader:
    FredStorageReader + BeaStorageReader + CensusStorageReader + Send + Sync + Debug
{
}

#[async_trait]
pub trait FredStorageReader: Send + Sync + Debug {
    async fn get_series(&self, series_id: &str) -> Result<Option<EconomicSeries>>;
    async fn list_active(&self) -> Result<Vec<EconomicSeries>>;
}

#[async_trait]
pub trait BeaStorageReader: Send + Sync + Debug {
    // BEA NIPA
    async fn get_bea_nipa(&self, id: &str) -> Result<Option<BeaNipaData>>;
    async fn get_bea_nipa_by_table(&self, table_name: &str, year: &str)
    -> Result<Vec<BeaNipaData>>;

    async fn get_bea_regional(&self, id: &str) -> Result<Option<BeaRegionalData>>;
    async fn get_bea_regional_by_table(
        &self,
        table_name: &str,
        year: &str,
    ) -> Result<Vec<BeaRegionalData>>;

    async fn get_bea_regional_filtered(
        &self,
        table_name: &str,
        geo_fips: Option<&str>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        year: &str,
    ) -> Result<Vec<BeaRegionalData>>;
}

#[async_trait]
pub trait CensusStorageReader: Send + Sync + Debug {
    async fn get_census(&self, id: &str) -> Result<Option<CensusData>>;
    async fn get_census_filtered(
        &self,
        dataset: &str,
        variable: &str,
        geo_fips: Option<&str>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        year: &str,
    ) -> Result<Vec<CensusData>>;
}
