use super::types::SeriesData;
use anyhow::Result;
use async_trait::async_trait;

/// Shared interface implemented by every economic data provider.
///
/// Each provider defines its own `series_id` format:
/// - **FRED**: plain series ID, e.g. `"CPIAUCSL"`
/// - **BEA**: `"TABLE:SERIES_CODE"`, e.g. `"T20100:A065RC"`
/// - **Census**: `"YEAR/DATASET/VARIABLE/GEO"`, e.g. `"2023/acs1/B19013_001E/state:*"`
#[async_trait]
pub trait EconomicProvider: Send + Sync + std::fmt::Debug {
    /// Fetch a time series by provider-specific ID.
    ///
    /// - `frequency` — optional frequency hint (`"m"`, `"q"`, `"a"`); providers
    ///   use their own default when `None`.
    /// - `limit` — cap on the number of returned [`SeriesData::data_points`].
    async fn get_series(
        &self,
        series_id: &str,
        frequency: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SeriesData>;

    /// Short identifier used in log messages and [`SeriesData::provider`].
    fn provider_name(&self) -> &str;
}
