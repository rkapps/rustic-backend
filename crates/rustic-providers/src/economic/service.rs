// rustic-providers/src/economic/service.rs

use super::traits::EconomicProvider;
use super::types::SeriesData;
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct EconomicDataService {
    fred: Option<Arc<dyn EconomicProvider>>,
    bea: Option<Arc<dyn EconomicProvider>>,
    census: Option<Arc<dyn EconomicProvider>>,
}

impl EconomicDataService {
    pub fn builder() -> EconomicDataServiceBuilder {
        EconomicDataServiceBuilder::default()
    }

    /// Fetch FRED series
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

    /// Fetch BEA series
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

    /// Fetch Census series
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

// ── Builder ───────────────────────────────────────────────────────────────────

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
