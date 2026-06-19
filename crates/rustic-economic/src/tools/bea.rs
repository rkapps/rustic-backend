use crate::{
    core::bea::{get_bea_nipa, get_bea_regional},
    storage::mongo::reader::EconomicMongoStorageReader,
};
use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, info};

#[derive(Debug)]
pub struct BeaDataTool {
    reader: Arc<EconomicMongoStorageReader>,
}

impl BeaDataTool {
    pub fn new(reader: Arc<EconomicMongoStorageReader>) -> Self {
        Self { reader }
    }
}

#[async_trait]
impl Tool for BeaDataTool {
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
                    "description": "Year or range. Examples: 2024, 2024,2023,2022, LAST5, LAST3, LAST2, LATEST"
                },
                "geo_fips": {
                    "type": "string",
                    "description": "Specific FIPS code. 06075=San Francisco, 04013=Maricopa"
                },
                "geo_type": {
                    "type": "string",
                    "enum": ["national", "region", "state", "county", "metro", "division"],
                    "description": "Filter by geography type"
                },
                "state_prefix": {
                    "type": "string",
                    "description": "2-digit state code to get all counties in a state. 06=California, 48=Texas, 04=Arizona"
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
        // let frequency = params["frequency"].as_str().unwrap_or("A");
        let year = params["year"].as_str().unwrap_or("LAST5");

        info!(
            target: "economic-tool",
            "Dataset: {:?} Table Name: {:?} year: {:?}",
            dataset, table_name, year
        );

        match dataset {
            "nipa" => {
                let rows = get_bea_nipa(self.reader.clone(), table_name, year).await?;
                Ok(json!({
                    "dataset":    dataset,
                    "table_name": table_name,
                    "year":       year,
                    "data":       rows,
                    "provider":   "bea"
                }))
            }
            "regional" => {
                let line_code = params["line_code"].as_str().unwrap_or("1");
                let geo_fips = params["geo_fips"].as_str();
                let geo_type = params["geo_type"].as_str();
                let state_prefix = params["state_prefix"].as_str();

                let rows = get_bea_regional(
                    self.reader.clone(),
                    table_name,
                    geo_fips,
                    geo_type,
                    state_prefix,
                    year,
                )
                .await?;

                debug!(
                    target: "economic-tool",
                    "bea regional table_name: {} geo_fips: {:?} geo_type: {:?} state_prefix: {:?} year: {} - rows: {}",
                    table_name,
                    geo_fips,
                    geo_type,
                    state_prefix,
                    year,
                    rows.len()
                );

                Ok(json!({
                    "dataset":    dataset,
                    "table_name": table_name,
                    "line_code":  line_code,
                    "geo_fips":   geo_fips,
                    "year":       year,
                    "data":       if rows.is_empty() {Value::Null} else {json!(rows)},
                    "unit":       "Thousands of dollars",
                    "provider":   "bea",
                    "note":       "UNIT_MULT=3 means thousands of dollars"
                }))
            }
            _ => Err(anyhow::anyhow!("dataset must be 'nipa' or 'regional'")),
        }
    }
}
