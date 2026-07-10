use serde::{Deserialize, Serialize};



#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EconomicConfig {
    pub fred_series: Vec<FredConfig>,
    pub bea_nipa: Vec<BeaNipaConfig>,
    pub bea_regional: Vec<BeaRegionalConfig>,
    pub census: Vec<CensusConfig>
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FredConfig {
    pub series_id: String,
    pub name: String,
    pub category: String,
    pub frequency: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BeaNipaConfig {
    pub table_name: String,
    pub series_code: String,
    pub description: String
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BeaRegionalConfig {
    pub code: String,
    pub line_code: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CensusConfig {
    pub variable: String,
    pub description: String
}