use anyhow::Result;
use async_trait::async_trait;
use rustic_agent::Tool;
use rustic_providers::CensusClient;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug)]
pub struct CensusTool {
    client: Arc<CensusClient>,
}

impl CensusTool {
    pub fn new(client: Arc<CensusClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for CensusTool {
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
                    "description": "Survey year e.g. 2023. ACS 1-year available for 2005+. Default: 2023"
                },
                "dataset": {
                    "type": "string",
                    "enum": ["acs1", "acs5"],
                    "description": "acs1 = 1-year estimates (larger areas), acs5 = 5-year estimates (all areas including rural). Default: acs1"
                },
                "variables": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Census variable codes. Common: B19013_001E (median household income), B01003_001E (population), B17001_002E (poverty), B23025_005E (unemployed), B25077_001E (median home value)"
                },
                "geo": {
                    "type": "string",
                    "description": "Geographic level. state:* (all states), county:* (all counties), us:1 (national). Default: state:*"
                }
            },
            "required": ["variables"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let year = params["year"].as_str().unwrap_or("2023");
        let dataset = params["dataset"].as_str().unwrap_or("acs1");
        let geo = params["geo"].as_str().unwrap_or("state:*");

        let variables: Vec<&str> = params["variables"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("variables required"))?
            .iter()
            .filter_map(|v| v.as_str())
            .collect();

        if variables.is_empty() {
            return Err(anyhow::anyhow!("at least one variable required"));
        }

        // always include NAME for readability
        let mut vars_with_name = vec!["NAME"];
        vars_with_name.extend(variables.iter());
        vars_with_name.dedup();

        let records = self
            .client
            .get_acs(year, dataset, &vars_with_name, geo)
            .await?;

        Ok(json!({
            "year":      year,
            "dataset":   dataset,
            "geo":       geo,
            "variables": variables,
            "data":      records,
            "count":     records.len(),
            "provider":  "census",
            "note":      "Values are estimates. Negative values (-666666666) indicate data not available."
        }))
    }
}
