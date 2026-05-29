use anyhow::Result;
use async_trait::async_trait;
use rustic_core::HttpClient;
use tracing::info;
use std::sync::Arc;

use super::model::{FredObservationsResponse, FredSeriesResponse};
use crate::economic::{
    traits::EconomicProvider,
    types::{DataPoint, SeriesData, SeriesInfo},
};

const FRED_BASE_URL: &str = "https://api.stlouisfed.org/fred";

/// Async client for the [FRED API](https://fred.stlouisfed.org/docs/api/fred/) (St. Louis Fed).
///
/// Implements [`EconomicProvider`] with plain FRED series IDs such as `"CPIAUCSL"` or
/// `"UNRATE"`. Observations and series metadata are fetched concurrently in [`FredClient::get_series`].
#[derive(Debug, Clone)]
pub struct FredClient {
    http_client: Arc<HttpClient>,
    api_key: String,
}

#[async_trait]
impl EconomicProvider for FredClient {
    async fn get_series(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SeriesData> {
        self.get_series(series_id, frequency, limit).await
    }

    fn provider_name(&self) -> &str {
        "fred"
    }
}

impl FredClient {
    /// Create a new client. Requires a FRED API key (free at <https://fred.stlouisfed.org/docs/api/api_key.html>).
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            http_client: Arc::new(HttpClient::new()?),
            api_key: api_key.into(),
        })
    }

    /// Fetch observations only (no metadata). Results are sorted descending and
    /// missing values (FRED sends `"."`) are silently dropped.
    ///
    /// Defaults: `frequency = "m"`, `limit = 12`.
    pub async fn get_observations(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SeriesData> {
        let frequency = frequency.unwrap_or("m");
        let limit = limit.unwrap_or(12);

        let url = format!(
            "{}/series/observations?series_id={}&api_key={}&frequency={}&sort_order=desc&limit={}&file_type=json",
            FRED_BASE_URL, series_id, self.api_key, frequency, limit
        );
        info!("Observations Url: {}", url);

        let raw: FredObservationsResponse = self.http_client.get_request(url, None).await?;

        let observations: Vec<DataPoint> = raw
            .observations
            .into_iter()
            .filter(|o| o.value != ".") // FRED uses "." for missing values
            .filter_map(|o| {
                o.value.parse::<f64>().ok().map(|v| DataPoint {
                    date: o.date,
                    value: v,
                })
            })
            .collect();

        Ok(SeriesData {
            series_id: series_id.to_string(),
            title: None,
            frequency: frequency.to_string(),
            units: None,
            data_points: observations,
            provider: "fred".to_string(),
        })
    }

    /// Fetch metadata for a series (title, units, frequency, seasonal adjustment).
    pub async fn get_series_info(&self, series_id: &str) -> Result<SeriesInfo> {
        let url = format!(
            "{}/series?series_id={}&api_key={}&file_type=json",
            FRED_BASE_URL, series_id, self.api_key
        );
        info!("Series Info Url: {}", url);

        let raw: FredSeriesResponse = self.http_client.get_request(url, None).await?;

        let series = raw
            .seriess
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Series {} not found", series_id))?;

        Ok(SeriesInfo {
            id: series.id,
            title: series.title,
            frequency: series.frequency,
            units: series.units,
            seasonal_adj: series.seasonal_adjustment,
            last_updated: series.last_updated,
            notes: series.notes,
        })
    }

    /// Fetch observations and metadata concurrently, returning a fully-populated [`SeriesData`].
    pub async fn get_series(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SeriesData> {
        let (mut data, info) = tokio::try_join!(
            self.get_observations(series_id, frequency, limit),
            self.get_series_info(series_id),
        )?;

        data.title = Some(info.title);
        data.units = Some(info.units);

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use rustic_core::set_logger;

    use crate::economic::fred::client::FredClient;
    use anyhow::Result;

    #[tokio::test]
    async fn test_fred_no_api_key() {
        let client = FredClient::new("invalid_key").unwrap();

        let result = client.get_observations("CPIAUCSL", None, None).await;

        assert!(result.is_err());
        println!("{}", result.unwrap_err()); // shows actual FRED error message
    }

    #[tokio::test]
    async fn test_fred_cpi() -> Result<()> {
        set_logger("rustic_providers=debug,rustic_core=debug".to_string());

        let api_key = env::var("FRED_API_KEY")?;
        let client = FredClient::new(api_key).unwrap();

        let data = client
            .get_series("CPIAUCSL", Some("m"), Some(12))
            .await
            .unwrap();

        println!("{}", serde_json::to_string_pretty(&data).unwrap());
        assert_eq!(data.series_id, "CPIAUCSL");
        assert!(!data.data_points.is_empty());
        assert!(data.title.is_some());

        Ok(())
    }
}
