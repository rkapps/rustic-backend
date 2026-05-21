use anyhow::Result;
use async_trait::async_trait;
use rustic_agent::Tool;
use rustic_providers::BeaClient;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug)]
pub struct BeaTool {
    client: Arc<BeaClient>,
}

impl BeaTool {
    pub fn new(client: Arc<BeaClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for BeaTool {
    fn name(&self) -> String {
        "bea_data".to_string()
    }

    fn description(&self) -> String {
        r#"Fetch economic data from the Bureau of Economic Analysis (BEA).
        Use BEA specifically for STATE-LEVEL data and detailed spending breakdowns
        not available in FRED.
        
        NIPA TABLES (national spending detail):
        T20305 → PCE by detailed product type (furniture, apparel, recreation lines)
        T20900 → PCE by function (housing, health, recreation, education)
        T10101 → GDP and major components
        T20100 → Personal income and outlays
        
        REGIONAL TABLES (state-level — BEA unique value):
        CAINC1    → Personal income by state (LineCode=1 for total)
        SASUMMARY → State annual economic summary
        SAPPCE    → State personal consumption by category
        
        KEY LINE CODES for T20305:
        Furniture/furnishings     → SeriesCode DFFFRC
        Apparel/clothing          → SeriesCode DCAFRC
        Recreation goods/vehicles → SeriesCode DREQRC
        Home maintenance/repair   → SeriesCode DFHHRC
        Food services             → SeriesCode DFDSRC
        
        FREQUENCY: A (annual) or Q (quarterly).
        YEAR: 2024 for latest, LAST5 for trend analysis.
        For national totals use FRED instead — BEA shines for regional/state data."#
            .to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "dataset": {
                    "type": "string",
                    "enum": ["nipa", "regional"],
                    "description": "nipa = National Income and Product Accounts (GDP, personal income, PCE). regional = data by US state"
                },
                "table_name": {
                    "type": "string",
                    "description": "BEA table name. NIPA tables: T20100 (Personal Income and Outlays), T10101 (GDP), T20600 (Personal Income by State). Regional tables: CAINC1 (Personal Income by State), SASUMMARY (State Annual Summary)"
                },
                "frequency": {
                    "type": "string",
                    "enum": ["A", "Q"],
                    "description": "A = Annual, Q = Quarterly. Default: A"
                },
                "year": {
                    "type": "string",
                    "description": "Year e.g. 2024, or LAST5 for last 5 years. Default: LAST5"
                },
                "line_code": {
                    "type": "string",
                    "description": "Line code for regional data. Default: 1 (total)"
                },
                "geo_fips": {
                    "type": "string",
                    "description": "Geographic FIPS for regional data. STATE = all states, 00000 = US total. Default: STATE"
                }
            },
            "required": ["dataset", "table_name"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let dataset = params["dataset"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("dataset required"))?;
        let table_name = params["table_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("table_name required"))?;
        let frequency = params["frequency"].as_str().unwrap_or("A");
        let year = params["year"].as_str().unwrap_or("LAST5");

        match dataset {
            "nipa" => {
                let rows = self.client.get_nipa(table_name, frequency, year).await?;
                Ok(json!({
                    "dataset":    dataset,
                    "table_name": table_name,
                    "frequency":  frequency,
                    "year":       year,
                    "data":       rows,
                    "count":      rows.len(),
                    "provider":   "bea"
                }))
            }
            "regional" => {
                let line_code = params["line_code"].as_str().unwrap_or("1");
                let geo_fips = params["geo_fips"].as_str().unwrap_or("STATE");

                let rows = self
                    .client
                    .get_regional(table_name, line_code, geo_fips, year)
                    .await?;
                Ok(json!({
                    "dataset":    dataset,
                    "table_name": table_name,
                    "line_code":  line_code,
                    "geo_fips":   geo_fips,
                    "year":       year,
                    "data":       rows,
                    "count":      rows.len(),
                    "unit":       "Thousands of dollars",
                    "provider":   "bea",
                    "note":       "UNIT_MULT=3 means thousands of dollars"
                }))
            }
            _ => Err(anyhow::anyhow!("dataset must be 'nipa' or 'regional'")),
        }
    }
}
