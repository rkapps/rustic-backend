use chrono::{DateTime, Utc};
use rustic_storage::RepoModel;
use serde::{Deserialize, Serialize};

use crate::domain::{BEA_NIPA_COLLECTION, BEA_REGIONAL_COLLECTION};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaNipa {
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
impl RepoModel<String> for BeaNipa {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn collection(&self) -> &'static str {
        BEA_NIPA_COLLECTION
    }
}

impl BeaNipa {
    pub fn is_fresh(&self) -> bool {
        Utc::now() < self.next_refresh
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaRegional {
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
impl RepoModel<String> for BeaRegional {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn collection(&self) -> &'static str {
        BEA_REGIONAL_COLLECTION
    }
}

impl BeaRegional {
    pub fn is_fresh(&self) -> bool {
        Utc::now() < self.next_refresh
    }
}
