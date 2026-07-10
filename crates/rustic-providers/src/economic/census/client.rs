use anyhow::Result;
use async_trait::async_trait;
use rustic_core::HttpClient;
use std::sync::Arc;
use tracing::{info, warn};

use super::model::{CensusRawResponse, CensusRecord};
use crate::economic::traits::EconomicProvider;
use crate::economic::types::{DataPoint, SeriesData};

const CENSUS_BASE_URL: &str = "https://api.census.gov/data";

/// Async client for the [U.S. Census Bureau API](https://www.census.gov/data/developers/data-sets.html).
///
/// Supports the **ACS** (American Community Survey), **SAIPE** poverty estimates,
/// and **International Trade** datasets. The Census API returns rows as arrays of
/// arrays; this client parses them into typed [`CensusRecord`]s.
///
/// Implements [`EconomicProvider`] with the series ID format
/// `"YEAR/DATASET/VARIABLE/GEO"`, e.g. `"2023/acs1/B19013_001E/state:*"`.
#[derive(Debug, Clone)]
pub struct CensusClient {
    http_client: Arc<HttpClient>,
    api_key: String,
}

impl CensusClient {
    /// Create a new client. Requires a Census API key (free at <https://api.census.gov/data/key_signup.html>).
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            http_client: Arc::new(HttpClient::new()?),
            api_key: api_key.into(),
        })
    }

    /// Fetch rows from the American Community Survey (ACS).
    ///
    /// - `dataset`: `"acs1"` (1-year estimates) or `"acs5"` (5-year estimates).
    /// - `variables`: column codes to retrieve, e.g. `["NAME", "B19013_001E"]`.
    ///   Include `"NAME"` to get human-readable geography names.
    /// - `geo`: geography filter, e.g. `"state:*"`, `"county:*"`, `"us:1"`.
    ///
    /// Common variable codes:
    /// - `B19013_001E` — Median household income
    /// - `B01003_001E` — Total population
    /// - `B17001_002E` — Population below poverty level
    /// - `B23025_005E` — Unemployed civilian population
    /// - `B25077_001E` — Median home value
    pub async fn get_acs(
        &self,
        year: &str,
        dataset: &str,
        variables: &[&str],
        geo: &str,
    ) -> Result<Vec<CensusRecord>> {
        let vars = variables.join(",");
        let url = format!(
            "{}/{}/acs/{}?get={}&for={}&key={}",
            CENSUS_BASE_URL, year, dataset, vars, geo, self.api_key
        );
        info!("ACS Url: {}", url);
        match self.http_client.get_request(url, None).await {
            Ok(raw) => Ok(self.parse_response(raw, variables)),
            Err(e) if e.to_string().contains("404") => {
                warn!(
                    "Census ACS not available for year: {} dataset: {}",
                    year, dataset
                );
                Ok(vec![])
            }
            Err(e) => Err(e),
        }
    }

    /// Fetch Small Area Income and Poverty Estimates (SAIPE) data.
    pub async fn get_cps(
        &self,
        year: &str,
        variables: &[&str],
        geo: &str,
    ) -> Result<Vec<CensusRecord>> {
        let vars = variables.join(",");
        let url = format!(
            "{}/timeseries/poverty/saipe?get={}&for={}&time={}&key={}",
            CENSUS_BASE_URL, vars, geo, year, self.api_key
        );
        info!("CPS Url: {}", url);

        let raw: CensusRawResponse = self.http_client.get_request(url, None).await?;

        Ok(self.parse_response(raw, variables))
    }

    /// Fetch International Trade (imports) data for a given year.
    pub async fn get_trade(&self, year: &str, variables: &[&str]) -> Result<Vec<CensusRecord>> {
        let vars = variables.join(",");
        let url = format!(
            "{}/timeseries/intltrade/imports?get={}&time={}&key={}",
            CENSUS_BASE_URL, vars, year, self.api_key
        );
        info!("Trade Url: {}", url);

        let raw: CensusRawResponse = self.http_client.get_request(url, None).await?;

        Ok(self.parse_response(raw, variables))
    }

    fn parse_response(&self, raw: CensusRawResponse, variables: &[&str]) -> Vec<CensusRecord> {
        if raw.len() < 2 {
            return vec![];
        }

        let headers = &raw[0];

        let name_idx = headers.iter().position(|h| h == "NAME");
        let state_idx = headers.iter().position(|h| h == "state");
        let county_idx = headers.iter().position(|h| h == "county");
        let country_idx = headers.iter().position(|h| h == "us");

        let geo_col = headers.last().cloned().unwrap_or_default().to_uppercase();

        let variable_indices: Vec<(&str, usize)> = variables
            .iter()
            .filter_map(|v| headers.iter().position(|h| h == v).map(|idx| (*v, idx)))
            .collect();

        let mut records = Vec::new();
        info!("Census headers: {:?}", headers);
        info!("Census first row: {:?}", raw.get(1));
        // info!("geo_type: {}", geo_col.to_uppercase());

        info!("geo_col raw: {:?}", headers.last());
        info!("geo_col uppercased: {}", geo_col);

        for row in &raw[1..] {
            let geo_name = name_idx
                .and_then(|i| row.get(i))
                .cloned()
                .unwrap_or_default();

            let geo_fips = if geo_col == "COUNTY" {
                let state = state_idx
                    .and_then(|i| row.get(i))
                    .cloned()
                    .unwrap_or_default();
                let county = county_idx
                    .and_then(|i| row.get(i))
                    .cloned()
                    .unwrap_or_default();
                format!("{}{}", state, county)
            } else if geo_col == "STATE" {
                let state = state_idx
                    .and_then(|i| row.get(i))
                    .cloned()
                    .unwrap_or_default();
                format!("{}000", state)
            } else {
                format!("00000")
            };

            for (variable, idx) in &variable_indices {
                records.push(CensusRecord {
                    geo_fips: geo_fips.clone(),
                    geo_name: geo_name.clone(),
                    geo_type: Some(geo_col.clone()),
                    variable: variable.to_string(),
                    value: row.get(*idx).cloned().unwrap_or_default(),
                });
            }
        }

        records
    }

    /// Map Census records to canonical SeriesData
    fn map_to_series(&self, records: Vec<CensusRecord>, series_id: &str, year: &str) -> SeriesData {
        let observations: Vec<DataPoint> = records
            .into_iter()
            .filter_map(|r| {
                r.value.parse::<f64>().ok().map(|v| DataPoint {
                    date: year.to_string(),
                    value: v,
                })
            })
            .collect();

        SeriesData {
            series_id: series_id.to_string(),
            title: None,
            frequency: "A".to_string(),
            units: None,
            data_points: observations,
            provider: "census".to_string(),
        }
    }
}

#[async_trait]
impl EconomicProvider for CensusClient {
    async fn get_series(
        &self,
        series_id: &str,
        _frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SeriesData> {
        // Census series_id format: "YEAR/DATASET/VARIABLE/GEO"
        // e.g. "2023/acs1/B19013_001E/state:*"
        let parts: Vec<&str> = series_id.splitn(4, '/').collect();
        if parts.len() != 4 {
            return Err(anyhow::anyhow!(
                "Census series_id format: YEAR/DATASET/VARIABLE/GEO \
                 e.g. 2023/acs1/B19013_001E/state:*"
            ));
        }

        let year = parts[0];
        let dataset = parts[1];
        let variable = parts[2];
        let geo = parts[3];

        let records = self
            .get_acs(year, dataset, &["NAME", variable], geo)
            .await?;

        let mut data = self.map_to_series(records, series_id, year);

        if let Some(limit) = limit {
            data.data_points.truncate(limit);
        }

        Ok(data)
    }

    fn provider_name(&self) -> &str {
        "census"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_census_median_income_by_state() {
        let api_key = std::env::var("CENSUS_API_KEY").expect("CENSUS_API_KEY not set");

        let client = CensusClient::new(api_key).unwrap();

        let records = client
            .get_acs("2023", "acs1", &["NAME", "B19013_001E"], "state:*")
            .await
            .unwrap();

        println!("{}", serde_json::to_string_pretty(&records).unwrap());
        assert!(!records.is_empty());
        // assert_eq!(records[0].name, "Alabama");
    }

    #[tokio::test]
    async fn test_census_population_by_state() {
        let api_key = std::env::var("CENSUS_API_KEY").expect("CENSUS_API_KEY not set");

        let client = CensusClient::new(api_key).unwrap();

        let records = client
            .get_acs("2023", "acs1", &["NAME", "B01003_001E"], "state:*")
            .await
            .unwrap();

        println!("{}", serde_json::to_string_pretty(&records).unwrap());
        assert!(!records.is_empty());
    }

    #[tokio::test]
    async fn test_census_get_series() {
        let api_key = std::env::var("CENSUS_API_KEY").expect("CENSUS_API_KEY not set");

        let client = CensusClient::new(api_key).unwrap();

        let data = client
            .get_series("2023/acs1/B19013_001E/state:*", None, Some(10))
            .await
            .unwrap();

        println!("{}", serde_json::to_string_pretty(&data).unwrap());
        assert!(!data.data_points.is_empty());
    }
}
