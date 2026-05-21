use serde::{Deserialize, Serialize};

/// Census API returns array of arrays
/// First array is headers, rest are data rows
pub type CensusRawResponse = Vec<Vec<String>>;

/// Parsed Census record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CensusRecord {
    pub name: String,
    pub value: String,
    pub geo_id: Option<String>,
    pub geo_type: Option<String>,
}

/// Census variable metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CensusVariable {
    pub code: String,
    pub label: String,
}
