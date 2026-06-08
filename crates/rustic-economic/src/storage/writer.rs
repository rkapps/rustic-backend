use anyhow::Result;
use std::fmt::Debug;

use async_trait::async_trait;

use crate::domain::{BeaNipaData, BeaRegionalData, CensusData, EconomicSeries};

#[async_trait]
pub trait StorageWriter:
    FredStorageWriter + BeaStorageWriter + CensusStorageWriter + Send + Sync + Debug
{
}

#[async_trait]
pub trait FredStorageWriter: Send + Sync + Debug {
    async fn delete_all_fred_series(&self) -> Result<()>;
    async fn upsert_fred_series(&self, series: EconomicSeries) -> Result<()>;
}

#[async_trait]
pub trait BeaStorageWriter: Send + Sync + Debug {
    // BEA NIPA
    async fn delete_all_bea_nipa(&self) -> Result<()>;
    async fn upsert_bea_nipa(&self, data: BeaNipaData) -> Result<()>;
    async fn upsert_bea_nipa_bulk(&self, datas: Vec<BeaNipaData>) -> Result<()>;

    // BEA Regional
    async fn delete_all_bea_regional(&self) -> Result<()>;
    async fn upsert_bea_regional_bulk(&self, datas: Vec<BeaRegionalData>) -> Result<()>;
}

#[async_trait]
pub trait CensusStorageWriter: Send + Sync + Debug {
    async fn delete_all_census(&self) -> Result<()>;
    async fn upsert_census_bulk(&self, datas: Vec<CensusData>) -> Result<()>;
    async fn upsert_census(&self, data: CensusData) -> Result<()>;
}
