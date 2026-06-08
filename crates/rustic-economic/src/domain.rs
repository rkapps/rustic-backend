use std::str::FromStr;

use chrono::{DateTime, Utc};
use rustic_providers::DataPoint;
use rustic_storage::RepoModel;
use serde::{Deserialize, Serialize};

pub const ECONOMIC_SERIES_COLLECTION: &str = "economic_series";
pub const BEA_NIPA_COLLECTION: &str = "economic_bea_nipa";
pub const BEA_REGIONAL_COLLECTION: &str = "economic_bea_regional";
pub const CENSUS_COLLECTION: &str = "economic_census";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EconomicSeries {
    pub id: String,        // same as series_id — MongoDB _id
    pub series_id: String, // "CPIAUCSL"
    pub source: EconomicSource,
    pub name: String,
    pub frequency: Frequency,
    pub category: String, // "consumer_health" | "consumer_spending" | "housing"
    pub sectors: Vec<String>, // ["furniture", "all"]
    pub active: bool,
    pub observations: Vec<DataPoint>,
    pub last_refreshed: Option<DateTime<Utc>>,
    pub next_refresh: Option<DateTime<Utc>>,
}

impl RepoModel<String> for EconomicSeries {
    fn id(&self) -> String {
        self.clone().id
    }
    fn collection(&self) -> &'static str {
        ECONOMIC_SERIES_COLLECTION
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EconomicSource {
    #[default]
    Fred,
    Bea,
    Census,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    #[default]
    Monthly, // m
    Quarterly, // q
    Annual,    // a
}

impl FromStr for Frequency {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "m" => Ok(Frequency::Monthly),
            "q" => Ok(Frequency::Quarterly),
            "a" => Ok(Frequency::Annual),
            _ => Ok(Frequency::Monthly),
        }
    }
}

impl Frequency {
    pub fn as_str(&self) -> &str {
        match self {
            Frequency::Monthly => "m",
            Frequency::Quarterly => "q",
            Frequency::Annual => "a",
        }
    }

    pub fn refresh_days(&self) -> i64 {
        match self {
            Frequency::Monthly => 1,
            Frequency::Quarterly => 7,
            Frequency::Annual => 30,
        }
    }
}

impl EconomicSeries {
    pub fn is_fresh(&self) -> bool {
        self.next_refresh.map(|r| Utc::now() < r).unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaNipaData {
    pub id: String, // "bea_nipa_T20100_A191RC_2024"
    pub table_name: String,
    pub series_code: String,
    pub line_number: String,
    pub line_description: String,
    pub time_period: String,
    pub metric_name: String,
    pub cl_unit: String,
    pub unit_mult: String,
    pub data_value: String,
    pub last_refreshed: DateTime<Utc>,
    pub next_refresh: DateTime<Utc>,
}
impl RepoModel<String> for BeaNipaData {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn collection(&self) -> &'static str {
        BEA_NIPA_COLLECTION
    }
}

impl BeaNipaData {
    pub fn is_fresh(&self) -> bool {
        Utc::now() < self.next_refresh
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaRegionalData {
    pub id: String, // "bea_regional_CAINC1_48_2024"
    pub code: String,
    pub geo_fips: String,
    pub geo_name: String,
    pub geo_type: String, // "state" | "county" | "metro"
    pub time_period: String,
    pub data_value: String,
    pub cl_unit: String,
    pub unit_mult: String,
    pub last_refreshed: DateTime<Utc>,
    pub next_refresh: DateTime<Utc>,
}
impl RepoModel<String> for BeaRegionalData {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn collection(&self) -> &'static str {
        BEA_REGIONAL_COLLECTION
    }
}

impl BeaRegionalData {
    pub fn is_fresh(&self) -> bool {
        Utc::now() < self.next_refresh
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CensusData {
    pub id: String,               // "census_acs5_2023_B19013_001E_04000"
    pub dataset: String,          // "acs5"
    pub year: String,             // "2023"
    pub variable: String,         // "B19013_001E"
    pub value: String,            // "76872"
    pub geo_fips: String,         // "04000"
    pub geo_name: String,         // "Arizona"
    pub geo_type: Option<String>, // "state" | "county"
    pub last_refreshed: DateTime<Utc>,
    pub next_refresh: DateTime<Utc>,
}

impl RepoModel<String> for CensusData {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn collection(&self) -> &'static str {
        CENSUS_COLLECTION
    }
}

impl CensusData {
    pub fn is_fresh(&self) -> bool {
        Utc::now() < self.next_refresh
    }
}
