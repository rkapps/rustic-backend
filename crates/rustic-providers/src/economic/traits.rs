// rustic-providers/src/economic/traits.rs

use anyhow::Result;
use async_trait::async_trait;
use super::types::SeriesData;

/// Common interface for economic data providers
#[async_trait]
pub trait EconomicProvider: Send + Sync + std::fmt::Debug {
    /// Fetch a time series by ID
    async fn get_series(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit:     Option<usize>,
    ) -> Result<SeriesData>;

    /// Provider name for logging/debugging
    fn provider_name(&self) -> &str;
}