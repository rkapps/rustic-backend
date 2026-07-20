use crate::{
    domain::{
        bea::{BeaNipa, BeaRegional},
        census::Census,
        fred::FredSeries,
    },
    tools::domain::{BeaNipaEntity, BeaRegionalEntity, CensusEntity},
};
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
    async fn get_series(&self, series_id: &str) -> Result<FredSeries>;
    async fn list_active(&self) -> Result<Vec<FredSeries>>;
}

#[async_trait]
pub trait BeaStorageReader: Send + Sync + Debug {
    // BEA NIPA
    async fn get_bea_nipa(&self, id: &str) -> Result<BeaNipa>;
    async fn get_bea_nipa_by_table_series(
        &self,
        table_name: String,
        series_codes: Vec<String>,
        years: Vec<String>,
    ) -> Result<Vec<BeaNipaEntity>>;

    async fn get_bea_regional(&self, id: &str) -> Result<BeaRegional>;
    async fn get_bea_regional_by_table_series(
        &self,
        codes: Vec<String>,
        years: Vec<String>,
        geo_fips: Vec<String>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
    ) -> Result<Vec<BeaRegionalEntity>>;

    async fn get_bea_regional_by_table(
        &self,
        table_name: &str,
        year: &str,
    ) -> Result<Vec<BeaRegional>>;

    async fn get_bea_regional_filtered(
        &self,
        table_name: &str,
        geo_fips: Option<&str>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        year: &str,
    ) -> Result<Vec<BeaRegional>>;
}

#[async_trait]
pub trait CensusStorageReader: Send + Sync + Debug {
    async fn get_census(&self, id: &str) -> Result<Census>;
    async fn get_census_by_dataset_variable(
        &self,
        dataset: &str,
        variables: Vec<String>,
        geo_fips: Vec<String>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        years: Vec<String>,
    ) -> Result<Vec<CensusEntity>>;

    async fn get_census_filtered(
        &self,
        dataset: &str,
        variable: &str,
        geo_fips: Option<&str>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        year: &str,
    ) -> Result<Vec<Census>>;
}
