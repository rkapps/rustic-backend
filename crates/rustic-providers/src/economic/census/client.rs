use anyhow::Result;
use async_trait::async_trait;
use rustic_core::HttpClient;
use std::sync::Arc;

use super::model::{CensusRawResponse, CensusRecord};
use crate::economic::traits::EconomicProvider;
use crate::economic::types::{DataPoint, SeriesData};

const CENSUS_BASE_URL: &str = "https://api.census.gov/data";

#[derive(Debug, Clone)]
pub struct CensusClient {
    http_client: Arc<HttpClient>,
    api_key: String,
}

impl CensusClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            http_client: Arc::new(HttpClient::new()?),
            api_key: api_key.into(),
        })
    }

    /// Fetch ACS data
    /// dataset: "acs1" (1-year) or "acs5" (5-year)
    /// variables: e.g. ["NAME", "B19013_001E"]
    /// geo: e.g. "state:*", "county:*", "us:1"
    ///
    /// Common variables:
    /// B19013_001E → Median household income
    /// B01003_001E → Total population
    /// B17001_002E → Population below poverty level
    /// B23025_005E → Unemployed population
    /// B25077_001E → Median home value
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

        let raw: CensusRawResponse = self.http_client.get_request(url, None).await?;

        Ok(self.parse_response(raw, variables))
    }

    /// Fetch Current Population Survey data
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

        let raw: CensusRawResponse = self.http_client.get_request(url, None).await?;

        Ok(self.parse_response(raw, variables))
    }

    /// Fetch International Trade data
    pub async fn get_trade(&self, year: &str, variables: &[&str]) -> Result<Vec<CensusRecord>> {
        let vars = variables.join(",");
        let url = format!(
            "{}/timeseries/intltrade/imports?get={}&time={}&key={}",
            CENSUS_BASE_URL, vars, year, self.api_key
        );

        let raw: CensusRawResponse = self.http_client.get_request(url, None).await?;

        Ok(self.parse_response(raw, variables))
    }

    /// Parse raw Census array response into records
    fn parse_response(&self, raw: CensusRawResponse, variables: &[&str]) -> Vec<CensusRecord> {
        if raw.len() < 2 {
            return vec![];
        }

        let headers = &raw[0];

        // find column indices
        let name_idx = headers.iter().position(|h| h == "NAME");
        let value_idx = headers
            .iter()
            .position(|h| variables.iter().any(|v| v == h && *h != "NAME"));
        let geo_type = headers.last().cloned();
        let geo_idx = headers.len() - 1; // geo ID is always last column

        raw[1..]
            .iter()
            .map(|row| CensusRecord {
                name: name_idx
                    .and_then(|i| row.get(i))
                    .cloned()
                    .unwrap_or_default(),
                value: value_idx
                    .and_then(|i| row.get(i))
                    .cloned()
                    .unwrap_or_default(),
                geo_id: row.get(geo_idx).cloned(),
                geo_type: geo_type.clone(),
            })
            .collect()
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
            observations,
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
            data.observations.truncate(limit);
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
        assert_eq!(records[0].name, "Alabama");
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
        assert!(!data.observations.is_empty());
    }
}
