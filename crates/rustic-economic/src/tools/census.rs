use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::service::EconomicDataService;

#[derive(Debug)]
pub struct CensusDataTool {
    service: Arc<EconomicDataService>,
}

impl CensusDataTool {
    pub fn new(service: Arc<EconomicDataService>) -> Self {
        Self { service }
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
                    "description": "Data year. Use 2023 (latest available).",
                    "default": "2023"
                },
                "dataset": {
                    "type": "string",
                    "enum": ["acs1", "acs5"],
                    "description": "acs1=1-year estimates, acs5=5-year estimates (includes rural areas)"
                },
                "variables": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "ACS variable codes e.g. B19013_001E (median income), B01003_001E (population)"
                },
                "geo": {
                    "type": "string",
                    "description": "Geography scope. Examples: 'state:*' (all states), 'state:04' (Arizona), 'county:*&in=state:04' (AZ counties), 'us:1' (national)"
                }
            },
            "required": ["year", "dataset", "variables", "geo"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let variables: Vec<&str> = params["variables"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("variables required"))?
            .iter()
            .filter_map(|v| v.as_str())
            .collect();

        let geo = params["geo"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("geo required"))?;

        let dataset = params["dataset"].as_str().unwrap_or("acs5");

        let year = params["year"].as_str().unwrap_or("2023");

        let records = self
            .service
            .get_census_data(&variables, geo, dataset, year)
            .await?;

        // census
        Ok(json!({
            "dataset": dataset,
            "geo": geo,
            "year": year,
            "records": records
        }))
    }
}
