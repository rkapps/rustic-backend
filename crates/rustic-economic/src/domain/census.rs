use chrono::{DateTime, Utc};
use rustic_storage::RepoModel;
use serde::{Deserialize, Serialize};

use crate::domain::CENSUS_COLLECTION;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Census {
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

impl RepoModel<String> for Census {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn collection(&self) -> &'static str {
        CENSUS_COLLECTION
    }
}

impl Census {
    pub fn is_fresh(&self) -> bool {
        Utc::now() < self.next_refresh
    }
}
