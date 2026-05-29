use anyhow::Result;
use async_trait::async_trait;
use rustic_core::HttpClient;
use tracing::info;
use std::sync::Arc;

use super::model::{BeaDataRow, BeaResponse};
use crate::economic::bea::model::BeaRegionalRow;
use crate::economic::traits::EconomicProvider;
use crate::economic::types::{DataPoint, SeriesData};

const BEA_BASE_URL: &str = "https://apps.bea.gov/api/data";

/// Async client for the [Bureau of Economic Analysis (BEA) API](https://apps.bea.gov/API/signup/).
///
/// Supports the **NIPA** dataset (national accounts: GDP, personal income) and the
/// **Regional** dataset (state- and county-level economic metrics).
///
/// Implements [`EconomicProvider`] with the series ID format `"TABLE:SERIES_CODE"`,
/// e.g. `"T20100:A065RC"`.
#[derive(Debug, Clone)]
pub struct BeaClient {
    http_client: Arc<HttpClient>,
    api_key: String,
}

impl BeaClient {
    /// Create a new client. Requires a BEA API key (free at <https://apps.bea.gov/API/signup/>).
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            http_client: Arc::new(HttpClient::new()?),
            api_key: api_key.into(),
        })
    }

    /// Fetch rows from a NIPA (National Income and Product Accounts) table.
    ///
    /// Common `table_name` values:
    /// - `"T10101"` — GDP
    /// - `"T20100"` — Personal Income and Outlays
    /// - `"T20200"` — Personal Income by State
    ///
    /// `frequency`: `"A"` (annual), `"Q"` (quarterly), `"M"` (monthly).
    /// `year`: specific year (`"2024"`) or `"LAST5"` for the last 5 years.
    pub async fn get_nipa(
        &self,
        table_name: &str,
        frequency: &str,
        year: &str,
    ) -> Result<Vec<BeaDataRow>> {
        let url = format!(
            "{}?UserID={}&method=GetData&datasetname=NIPA&TableName={}&Frequency={}&Year={}&ResultFormat=JSON",
            BEA_BASE_URL, self.api_key, table_name, frequency, year
        );
        info!("Nipa Url: {}", url);

        let response: BeaResponse = self.http_client.get_request(url, None).await?;

        if let Some(error) = response.bea_api.error {
            return Err(anyhow::anyhow!(
                "BEA error {}: {}",
                error.code,
                error.description
            ));
        }

        let data = response
            .bea_api
            .results
            .ok_or_else(|| anyhow::anyhow!("No results"))?
            .data
            .ok_or_else(|| anyhow::anyhow!("No data"))?;

        let rows: Vec<BeaDataRow> = serde_json::from_value(data)?;
        Ok(rows)
    }

    /// Fetch rows from the Regional dataset (state- and county-level metrics).
    ///
    /// Common `table_name` values:
    /// - `"CAINC1"` — Personal Income by State
    /// - `"CAINC4"` — Personal Income by County
    /// - `"SASUMMARY"` — State Annual Summary
    ///
    /// `geo_fips`: `"STATE"` for all states, a FIPS code for a specific area.
    pub async fn get_regional(
        &self,
        table_name: &str,
        line_code: &str,
        geo_fips: &str,
        year: &str,
    ) -> Result<Vec<BeaRegionalRow>> {
        let url = format!(
            "{}?UserID={}&method=GetData&datasetname=Regional&TableName={}&LineCode={}&GeoFips={}&Year={}&ResultFormat=JSON",
            BEA_BASE_URL, self.api_key, table_name, line_code, geo_fips, year
        );
        info!("Regional Url: {}", url);
        let response: BeaResponse = self.http_client.get_request(url, None).await?;

        if let Some(error) = response.bea_api.error {
            return Err(anyhow::anyhow!(
                "BEA error {}: {}",
                error.code,
                error.description
            ));
        }

        let data = response
            .bea_api
            .results
            .ok_or_else(|| anyhow::anyhow!("No results"))?
            .data
            .ok_or_else(|| anyhow::anyhow!("No data"))?;

        let rows: Vec<BeaRegionalRow> = serde_json::from_value(data)?;
        Ok(rows)
    }

    /// Parse DataValue string to f64
    /// BEA returns values like "24,905,900" with commas
    fn parse_value(value: &str) -> Option<f64> {
        value.replace(",", "").parse::<f64>().ok()
    }

    /// Map NIPA rows to canonical SeriesData
    /// Filters by series_code (line item)
    fn map_to_series(
        &self,
        rows: Vec<BeaDataRow>,
        series_id: &str,
        series_code: &str,
    ) -> SeriesData {
        let observations: Vec<DataPoint> = rows
            .into_iter()
            .filter(|r| r.series_code == series_code)
            .filter_map(|r| {
                Self::parse_value(&r.data_value).map(|v| DataPoint {
                    date: r.time_period,
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
            provider: "bea".to_string(),
        }
    }
}

#[async_trait]
impl EconomicProvider for BeaClient {
    async fn get_series(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SeriesData> {
        // BEA series_id format: "TABLE:SERIES_CODE"
        // e.g. "T20100:A065RC" → Personal Income from T20100
        let parts: Vec<&str> = series_id.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!(
                "BEA series_id must be in format TABLE:SERIES_CODE e.g. T20100:A065RC"
            ));
        }

        let table_name = parts[0];
        let series_code = parts[1];
        let frequency = frequency.unwrap_or("A").to_uppercase();
        let year = "LAST5"; // last 5 years

        let rows = self.get_nipa(table_name, &frequency, year).await?;
        let mut data = self.map_to_series(rows, series_id, series_code);

        // apply limit
        if let Some(limit) = limit {
            data.data_points.truncate(limit);
        }

        Ok(data)
    }

    fn provider_name(&self) -> &str {
        "bea"
    }
}

#[cfg(test)]
mod tests {
    use rustic_core::set_logger;

    use super::*;

    #[tokio::test]
    async fn test_bea_regional_raw() {

        set_logger("rustic_providers=debug,rustic_core=trace".to_string());

        let api_key = std::env::var("BEA_API_KEY").unwrap();
        let client = HttpClient::new().unwrap();

        let url = format!(
            "{}?UserID={}&method=GetData&datasetname=Regional&TableName=CAINC1&LineCode=1&GeoFips=STATE&Year=2023&ResultFormat=JSON",
            BEA_BASE_URL, api_key
        );

        let response: serde_json::Value = client.get_request(url, None).await.unwrap();

        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    }

    #[tokio::test]
    async fn test_bea_personal_income() {
        set_logger("rustic_providers=debug,rustic_core=trace".to_string());

        let api_key = std::env::var("BEA_API_KEY").expect("BEA_API_KEY not set");

        let client = BeaClient::new(api_key).unwrap();

        let rows = client.get_nipa("T20100", "A", "2024").await.unwrap();

        println!("{}", serde_json::to_string_pretty(&rows).unwrap());
        assert!(!rows.is_empty());
    }

    #[tokio::test]
    async fn test_bea_state_income() {

        set_logger("rustic_providers=debug,rustic_core=trace".to_string());
        let api_key = std::env::var("BEA_API_KEY").unwrap();
        let client = BeaClient::new(api_key).unwrap();

        // personal income by state
        let rows = client
            .get_regional("CAINC1", "1", "STATE", "2023")
            .await
            .unwrap();

        println!("{}", serde_json::to_string_pretty(&rows).unwrap());
    }

    #[tokio::test]
    async fn test_bea_get_series() {

        set_logger("rustic_providers=debug,rustic_core=trace".to_string());

        let api_key = std::env::var("BEA_API_KEY").expect("BEA_API_KEY not set");
        let client = BeaClient::new(api_key).unwrap();

        // T20100:A065RC = Personal Income from T20100
        let data = client
            .get_series("T20100:A065RC", Some("A"), Some(5))
            .await
            .unwrap();

        println!("{}", serde_json::to_string_pretty(&data).unwrap());
        assert!(!data.data_points.is_empty());
    }
}
