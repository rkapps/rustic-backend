use std::str::FromStr;

use chrono::{DateTime, Utc};
use rustic_providers::DataPoint;
use rustic_storage::RepoModel;
use serde::{Deserialize, Serialize};

use crate::domain::FRED_SERIES_COLLECTION;


#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct FredSeries {
    pub id: String,        // same as series_id — MongoDB _id
    pub series_id: String, // "CPIAUCSL"
    pub source: FredSource,
    pub name: String,
    pub frequency: Frequency,
    pub category: String, // "consumer_health" | "consumer_spending" | "housing"
    pub active: bool,
    pub observations: Vec<DataPoint>,
    pub last_refreshed: Option<DateTime<Utc>>,
    pub next_refresh: Option<DateTime<Utc>>,
}

impl RepoModel<String> for FredSeries {
    fn id(&self) -> String {
        self.clone().id
    }
    fn collection(&self) -> &'static str {
        FRED_SERIES_COLLECTION
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FredSource {
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

impl FredSeries {
    pub fn is_fresh(&self) -> bool {
        self.next_refresh.map(|r| Utc::now() < r).unwrap_or(false)
    }
}