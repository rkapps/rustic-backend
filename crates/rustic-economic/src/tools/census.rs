use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, info};

use crate::{core::census::get_census_data, storage::mongo::reader::EconomicMongoStorageReader};

#[derive(Debug)]
pub struct CensusDataTool {
    reader: Arc<EconomicMongoStorageReader>,
}

impl CensusDataTool {
    pub fn new(reader: Arc<EconomicMongoStorageReader>) -> Self {
        Self { reader }
    }
}

#[async_trait]
impl Tool for CensusDataTool {
    fn name(&self) -> String {
        "census_data".to_string()
    }

    fn description(&self) -> String {
        r#"Fetch demographic and economic data from the US Census Bureau ACS survey.
            Use for population, age distribution, income, poverty, homeownership,
            employment, and education by state or county.

            AGE: B01003_001E (total population), B01002_001E (median age),
            B09001_001E (under 18), B09021_022E (65+).

            INCOME: B19013_001E (median household income), B19301_001E (per capita income),
            B17001_002E (below poverty), B19083_001E (income inequality/Gini).

            HOMEOWNERSHIP: B25003_002E (owner occupied), B25003_003E (renter occupied),
            B25077_001E (median home value), B25064_001E (median gross rent).

            EMPLOYMENT: B23025_004E (employed), B23025_005E (unemployed).

            EDUCATION: B15003_022E (bachelor degree), B15003_025E (doctorate).

            Geographic levels via geo parameter:
            state:*    = all 50 states
            county:*   = all counties
            us:1       = national total

            Dataset: acs1 (1-year, larger areas only) or acs5 (5-year, all areas).
            Year: 2023 is latest available for most variables."#
            .to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "year": {
                    "type": "string",
                    "description": "Year or range. Examples: 2025,2024,2023,2022, LAST5, LAST3, LAST2"
                },
                "dataset": {
                    "type": "string",
                    "enum": ["acs5"],
                    "description": "acs5=5-year estimates (includes rural areas)"
                },
                "variables": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "ACS variable codes e.g. B19013_001E (median income), B01003_001E (population)"
                },
                "geo_fips": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "FIPS codes for filtering. National: 00000. States: 06000=California, 48000=Texas, 04000=Arizona. Counties: 06075=San Francisco, 04013=Maricopa."
                },
                "geo_type": {
                    "type": "string",
                    "enum": ["US", "STATE", "COUNTY"],
                    "description": "Filter by geography type"
                },
                "state_prefix": {
                    "type": "string",
                    "description": "2-digit state code for all counties in a state. 06=California, 48=Texas, 04=Arizona"
                }
            },
            "required": ["year", "dataset", "variables"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let variables: Vec<String> = params["variables"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("variables required"))?
            .iter()
            // Extract the inner string slice instead of converting the JSON block to a string
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        let dataset = params["dataset"].as_str().unwrap_or("acs5");
        let year = params["year"].as_str().unwrap_or("LAST5");
        let geo_type = params["geo_type"].as_str();
        let state_prefix = params["state_prefix"].as_str();

        let mut geo_fips: Vec<String> = params["geo_fips"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str()) // Safely extracts Option<&str>
                    .map(String::from) // Converts &str to owned String
                    .collect()
            })
            .unwrap_or_default(); // Returns empty Vec if "geo_fips" is missing or not an array

        if geo_fips.is_empty() && geo_type.is_none() && state_prefix.is_none() {
            geo_fips = ["00000".to_string()].to_vec();
        }

        info!(
            target: "economic-tool",
            "Census data - dataset: {} variables: {:?} year: {:?} geo_type: {:?} geo_fips: {:?}", dataset, variables, year, geo_type, geo_fips
        );

        let records = get_census_data(
            self.reader.clone(),
            variables.clone(),
            dataset,
            geo_fips,
            geo_type,
            state_prefix,
            year,
        )
        .await?;
        debug!(
            target: "economic-tool",
            "Census: {}", records.len()
        );
        // census
        Ok(json!({
            "census": if records.is_empty() {Value::Null} else {json!(records)},
            "provider":   "census"
        }))
    }
}
