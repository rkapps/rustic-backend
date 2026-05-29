// rustic-economic/src/domain/mod.rs

use chrono::{DateTime, Utc};
use rustic_providers::{DataPoint, economic::{bea::model::{BeaDataRow, BeaRegionalRow}, census::model::CensusRecord}};
use rustic_storage::RepoModel;
use serde::{Deserialize, Serialize};

pub const ECONOMIC_SERIES_COLLECTION: &str = "economic_series";
pub const BEA_NIPA_COLLECTION: &str = "economic_bea_nipa";
pub const BEA_REGIONAL_COLLECTION: &str = "economic_bea_regional";
pub const CENSUS_COLLECTION: &str = "economic_census";


#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EconomicSeries {
    pub id: String,                          // same as series_id — MongoDB _id
    pub series_id: String,                   // "CPIAUCSL"
    pub source: EconomicSource,
    pub name: String,
    pub frequency: Frequency,
    pub category: String,                    // "consumer_health" | "consumer_spending" | "housing"
    pub sectors: Vec<String>,                // ["furniture", "all"]
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
    Monthly,   // m
    Quarterly, // q
    Annual,    // a
}

impl Frequency {
    pub fn as_str(&self) -> &str {
        match self {
            Frequency::Monthly   => "m",
            Frequency::Quarterly => "q",
            Frequency::Annual    => "a",
        }
    }

    pub fn refresh_days(&self) -> i64 {
        match self {
            Frequency::Monthly   => 1,
            Frequency::Quarterly => 7,
            Frequency::Annual    => 30,
        }
    }
}

impl EconomicSeries {
    pub fn is_fresh(&self) -> bool {
        self.next_refresh
            .map(|r| Utc::now() < r)
            .unwrap_or(false)
    }
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaNipaData {
    pub id: String,
    pub table_name: String,
    pub frequency: String,
    pub year: String,
    pub rows: Vec<BeaDataRow>,
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


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaRegionalData {
    pub id: String,
    pub table_name: String,
    pub geo_fips: String,
    pub year: String,
    pub rows: Vec<BeaRegionalRow>,
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


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CensusData {
    pub id: String,
    pub dataset: String,
    pub geo: String,
    pub year: String,
    pub records: Vec<CensusRecord>,
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


impl BeaNipaData {
    pub fn is_fresh(&self) -> bool {
        Utc::now() < self.next_refresh
    }
}

impl BeaRegionalData {
    pub fn is_fresh(&self) -> bool {
        Utc::now() < self.next_refresh
    }
}

impl CensusData {
    pub fn is_fresh(&self) -> bool {
        Utc::now() < self.next_refresh
    }
}
