use super::traits::EconomicProvider;
use super::types::SeriesData;
use anyhow::Result;
use std::sync::Arc;

/// Facade that routes requests to the appropriate [`EconomicProvider`].
///
/// Construct via [`EconomicDataService::builder()`]. Any subset of providers
/// may be configured; calling a method for an unconfigured provider returns an
/// error rather than panicking.
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use rustic_providers::{EconomicDataService, FredClient};
///
/// # #[tokio::main] async fn main() -> anyhow::Result<()> {
/// let service = EconomicDataService::builder()
///     .with_fred(Arc::new(FredClient::new(std::env::var("FRED_API_KEY")?)?))
///     .build();
///
/// let data = service.fred_series("UNRATE", Some("m"), Some(24)).await?;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub struct EconomicDataService {
    fred: Option<Arc<dyn EconomicProvider>>,
    bea: Option<Arc<dyn EconomicProvider>>,
    census: Option<Arc<dyn EconomicProvider>>,
}

impl EconomicDataService {
    /// Return a builder for constructing an [`EconomicDataService`].
    pub fn builder() -> EconomicDataServiceBuilder {
        EconomicDataServiceBuilder::default()
    }

    /// Fetch a FRED time series. `series_id` is a plain FRED series code such as `"CPIAUCSL"`.
    pub async fn fred_series(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SeriesData> {
        self.fred
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("FRED provider not configured"))?
            .get_series(series_id, frequency, limit)
            .await
    }

    /// Fetch a BEA series. `series_id` format: `"TABLE:SERIES_CODE"`, e.g. `"T20100:A065RC"`.
    pub async fn bea_series(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SeriesData> {
        self.bea
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("BEA provider not configured"))?
            .get_series(series_id, frequency, limit)
            .await
    }

    /// Fetch a Census series. `series_id` format: `"YEAR/DATASET/VARIABLE/GEO"`,
    /// e.g. `"2023/acs1/B19013_001E/state:*"`.
    pub async fn census_series(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SeriesData> {
        self.census
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Census provider not configured"))?
            .get_series(series_id, frequency, limit)
            .await
    }
}

/// Builder for [`EconomicDataService`]. All providers are optional.
#[derive(Debug, Default)]
pub struct EconomicDataServiceBuilder {
    fred: Option<Arc<dyn EconomicProvider>>,
    bea: Option<Arc<dyn EconomicProvider>>,
    census: Option<Arc<dyn EconomicProvider>>,
}

impl EconomicDataServiceBuilder {
    pub fn with_fred(mut self, provider: Arc<dyn EconomicProvider>) -> Self {
        self.fred = Some(provider);
        self
    }

    pub fn with_bea(mut self, provider: Arc<dyn EconomicProvider>) -> Self {
        self.bea = Some(provider);
        self
    }

    pub fn with_census(mut self, provider: Arc<dyn EconomicProvider>) -> Self {
        self.census = Some(provider);
        self
    }

    pub fn build(self) -> EconomicDataService {
        EconomicDataService {
            fred: self.fred,
            bea: self.bea,
            census: self.census,
        }
    }
}
