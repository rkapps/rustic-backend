// rustic-economic/src/service/impl.rs

use anyhow::Result;
use chrono::{Duration, Utc};
use std::sync::Arc;

use rustic_providers::{
    BeaClient, CensusClient, DataPoint, FredClient, economic::{bea::model::{BeaDataRow, BeaRegionalRow}, census::model::CensusRecord},
};

use crate::{
    domain::{BeaNipaData, BeaRegionalData, CensusData, EconomicSeries},
    storage::EconomicStorageManager,
};

#[derive(Debug, Clone)]
pub struct EconomicDataService {
    storage: Arc<EconomicStorageManager>,
    fred: Arc<FredClient>,
    bea: Arc<BeaClient>,
    census: Arc<CensusClient>,
}

impl EconomicDataService {
    pub fn new(
        storage: Arc<EconomicStorageManager>,
        fred: Arc<FredClient>,
        bea: Arc<BeaClient>,
        census: Arc<CensusClient>,
    ) -> Self {
        Self {
            storage,
            fred,
            bea,
            census,
        }
    }

    fn next_refresh(frequency: &str) -> chrono::DateTime<Utc> {
        let now = Utc::now();
        match frequency {
            "m" => now + Duration::days(1),
            "q" => now + Duration::days(7),
            "a" => now + Duration::days(30),
            _ => now + Duration::days(1),
        }
    }

    pub async fn get_fred_series(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<DataPoint>> {
        // check storage
        if let Some(stored) = self.storage.get_series(series_id).await? {
            if stored.is_fresh() {
                let obs = match limit {
                    Some(n) => stored.observations.into_iter().take(n).collect(),
                    None => stored.observations,
                };
                return Ok(obs);
            }
        }
        // fetch from FRED
        let data = self.fred.get_series(series_id, frequency, limit).await?;

        // upsert to storage
        let series = EconomicSeries {
            id: series_id.to_string(),
            series_id: series_id.to_string(),
            observations: data.data_points,
            last_refreshed: Some(Utc::now()),
            next_refresh: Some(Self::next_refresh(frequency.unwrap_or("m"))),
            ..Default::default()
        };
        self.storage.upsert_series(series.clone()).await?;

        Ok(series.observations)
    }

    pub async fn get_bea_nipa(
        &self,
        table_name: &str,
        frequency: &str,
        year: &str,
    ) -> Result<Vec<BeaDataRow>> {
        let id = format!("bea_nipa_{}_{}", table_name, year);

        if let Some(stored) = self.storage.get_bea_nipa(&id).await? {
            if stored.is_fresh() {
                return Ok(stored.rows);
            }
        }

        let rows = self.bea.get_nipa(table_name, frequency, year).await?;

        self.storage
            .upsert_bea_nipa(BeaNipaData {
                id,
                table_name: table_name.to_string(),
                frequency: frequency.to_string(),
                year: year.to_string(),
                rows: rows.clone(),
                last_refreshed: Utc::now(),
                next_refresh: Self::next_refresh("m"),
            })
            .await?;

        Ok(rows)
    }

    pub async fn get_bea_regional(
        &self,
        table_name: &str,
        line_code: &str,
        geo_fips: &str,
        year: &str,
    ) -> Result<Vec<BeaRegionalRow>> {
        let id = format!("bea_regional_{}_{}_{}", table_name, geo_fips, year);
    
        if let Some(stored) = self.storage.get_bea_regional(&id).await? {
            if stored.is_fresh() {
                return Ok(stored.rows);
            }
        }
    
        let rows = self.bea
            .get_regional(table_name, line_code, geo_fips, year)
            .await?;
    
        self.storage.upsert_bea_regional(BeaRegionalData {
            id,
            table_name: table_name.to_string(),
            geo_fips: geo_fips.to_string(),
            year: year.to_string(),
            rows: rows.clone(),
            last_refreshed: Utc::now(),
            next_refresh: Self::next_refresh("a"),
        }).await?;
    
        Ok(rows)
    }


    pub async fn get_census_data(
        &self,
        variables: &[&str],
        geo: &str,
        dataset: &str,
        year: &str,
    ) -> Result<Vec<CensusRecord>> {
        let id = format!("census_{}_{}_{}", dataset, geo, year);
    
        if let Some(stored) = self.storage.get_census(&id).await? {
            if stored.is_fresh() {
                return Ok(stored.records);
            }
        }
    
        let records = self.census
            .get_acs(year, dataset, variables, geo)
            .await?;
    
        self.storage.upsert_census(CensusData {
            id,
            dataset: dataset.to_string(),
            geo: geo.to_string(),
            year: year.to_string(),
            records: records.clone(),
            last_refreshed: Utc::now(),
            next_refresh: Self::next_refresh("a"),
        }).await?;
    
        Ok(records)
    }
}
