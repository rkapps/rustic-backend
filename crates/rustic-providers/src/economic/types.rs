use serde::{Deserialize, Serialize};

/// A single dated observation from an economic time series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    /// ISO-8601 date string (`"2024-01"`, `"2023-Q3"`, `"2023"`, etc.).
    pub date: String,
    /// Numeric value for the observation.
    pub value: f64,
}

/// Metadata describing an economic series (currently only populated for FRED).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesInfo {
    pub id: String,
    pub title: String,
    /// Frequency code, e.g. `"Monthly"`, `"Quarterly"`, `"Annual"`.
    pub frequency: String,
    pub units: String,
    pub seasonal_adj: String,
    pub last_updated: String,
    pub notes: Option<String>,
}

/// Canonical response returned by every [`EconomicProvider`](super::traits::EconomicProvider).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesData {
    /// Provider-specific series identifier passed to `get_series`.
    pub series_id: String,
    /// Human-readable series title when available.
    pub title: Option<String>,
    /// Frequency code echoed from the request or provider default.
    pub frequency: String,
    pub units: Option<String>,
    pub data_points: Vec<DataPoint>,
    /// Short provider tag: `"fred"`, `"bea"`, or `"census"`.
    pub provider: String,
}
