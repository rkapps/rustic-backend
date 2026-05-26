use anyhow::Result;
use async_trait::async_trait;
use rustic_core::Tool;
use rustic_providers::FredClient;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::info;

#[derive(Debug)]
pub struct FredTool {
    client: Arc<FredClient>,
}

impl FredTool {
    pub fn new(client: Arc<FredClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for FredTool {
    fn name(&self) -> String {
        "fred_series".to_string()
    }

    fn description(&self) -> String {
        r#"Fetch economic time series from FRED (Federal Reserve Economic Data).
        Covers macro indicators AND detailed consumer spending by category.

            HOME/FURNITURE: DFFFRC1A027NBEA (furniture annual),
            DFDHRC1Q027SBEA (furnishings quarterly),
            RSFSXMV (building materials monthly - home improvement proxy).

            APPAREL: DCAFRC1A027NBEA (clothing+footwear annual),
            MRTSSM44X72USS (clothing stores monthly).

            RECREATIONAL: DREQRC1Q027SBEA (recreational goods quarterly).

            RESTAURANTS: DSERRE1Q027SBEA (food services quarterly),
            MRTSSM722USS (food services monthly).

            MACRO: CPIAUCSL (CPI), UNRATE (unemployment), PCE (total spending),
            UMCSENT (consumer sentiment), DSPIC96 (disposable income),
            HOUST (housing starts), RSXFS (retail sales).

            Returns observations sorted newest first."#
            .to_string()
    }

    fn parameters(&self) -> Value {
        json!({
                    "type": "object",
                    "properties": {
                        "series_id": {
                            "type": "string",
                            "description": "FRED series ID. Examples: CPIAUCSL (CPI/inflation), PCE (consumer spending), UNRATE (unemployment), RSXFS (retail sales), UMCSENT (consumer sentiment), GDP, HOUST (housing starts), DSPIC96 (disposable income), MRTSSM44X72USS (clothing retail)"
                        },
                        "frequency": {
                        "type": "string",
                        "enum": ["a", "q", "m", "sa"],
                        "description": "Data frequency. a=annual, q=quarterly, m=monthly, sa=semi-annual. Omit to use series default."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Number of recent observations. Default: 12 (1 year monthly)"
                        }
                    },
                    "required": ["series_id"]
                })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let series_id = params["series_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("series_id required"))?;

        let frequency = params["frequency"].as_str();
        let limit = params["limit"].as_u64().map(|l| l as usize);
        let frequency = match frequency.as_deref() {
            Some("annual") | Some("a") => Some("a"),
            Some("quarterly") | Some("q") => Some("q"),
            Some("monthly") | Some("m") => Some("m"),
            _ => None,  // let FRED use default frequency for the series
        };
        info!(
            "Series: {:?} Frequency: {:?} data: {:?}",
            series_id, frequency, limit
        );

        let data = self.client.get_series(series_id, frequency, limit).await?;

        Ok(json!({
            "series_id":    data.series_id,
            "title":        data.title,
            "frequency":    data.frequency,
            "units":        data.units,
            "observations": data.observations,
            "count":        data.observations.len(),
            "provider":     data.provider,
            "note":         "FRED data typically lags 2-4 weeks. Values sorted newest first."
        }))
    }
}
