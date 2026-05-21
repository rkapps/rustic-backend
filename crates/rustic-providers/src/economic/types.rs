use serde::{Deserialize, Serialize};

/// A single observation from an economic time series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub date:     String,
    pub value:    f64,
}

/// Metadata about an economic series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesInfo {
    pub id:             String,
    pub title:          String,
    pub frequency:      String,
    pub units:          String,
    pub seasonal_adj:   String,
    pub last_updated:   String,
    pub notes:          Option<String>,
}

/// Result of a series observations fetch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesData {
    pub series_id:    String,
    pub title:        Option<String>,
    pub frequency:    String,
    pub units:        Option<String>,
    pub observations: Vec<DataPoint>,
    pub provider:     String,
}