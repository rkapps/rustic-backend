#[cfg(feature = "writer")]
use anyhow::Result;
use std::{sync::Arc, time::Duration};
use tracing::info;

use crate::service::EconomicService;

pub struct EconomicDataPipeline {
    service: Arc<EconomicService>,
}

impl EconomicDataPipeline {
    pub fn new(service: Arc<EconomicService>) -> Self {
        Self { service }
    }

    pub async fn run_fred(&self, clean: bool) -> Result<()> {
        info!("Economic Data Fred Pipeline started...");
        if clean {
            self.service.clean_fred().await?;
        }
        if let Err(e) = self.update_fred().await {
            tracing::error!("FRED pipeline failed: {}", e);
        } else {
            tracing::info!("FRED pipeline complete");
        }
        Ok(())
    }

    pub async fn run_bea(&self, clean: bool) -> Result<()> {
        info!("Economic Data Bea Pipeline started...");

        if clean {
            self.service.clean_bea().await?;
        }
        if let Err(e) = self.update_bea().await {
            tracing::error!("BEA pipeline failed: {}", e);
        } else {
            tracing::info!("BEA pipeline complete");
        }

        Ok(())
    }

    pub async fn run_census(&self, clean: bool) -> Result<()> {
        info!("Economic Data Census Pipeline started...");

        if clean {
            self.service.clean_census().await?;
        }

        if let Err(e) = self.update_census().await {
            tracing::error!("Census pipeline failed: {}", e);
        } else {
            tracing::info!("Census pipeline complete");
        }

        Ok(())
    }

    async fn update_fred(&self) -> Result<()> {
        let series = vec![
            // (series_id, frequency, limit, name, category, sectors)
            (
                "CPIAUCSL",
                "m",
                12,
                "Consumer Price Index",
                "consumer_health",
                vec!["all"],
            ),
            (
                "UMCSENT",
                "m",
                12,
                "Consumer Sentiment",
                "consumer_health",
                vec!["all"],
            ),
            (
                "UNRATE",
                "m",
                12,
                "Unemployment Rate",
                "consumer_health",
                vec!["all"],
            ),
            (
                "DSPIC96",
                "m",
                12,
                "Real Disposable Income",
                "consumer_health",
                vec!["all"],
            ),
            (
                "PCE",
                "m",
                12,
                "Personal Consumption Expenditures",
                "consumer_health",
                vec!["all"],
            ),
            (
                "HOUST",
                "m",
                12,
                "Housing Starts",
                "housing",
                vec!["all", "furniture", "home"],
            ),
            (
                "PERMIT",
                "m",
                12,
                "Building Permits",
                "housing",
                vec!["all", "furniture", "home"],
            ),
            (
                "RSFSXMV",
                "m",
                12,
                "Building Materials Retail",
                "consumer_spending",
                vec!["furniture", "home"],
            ),
            (
                "DFFFRC1A027NBEA",
                "a",
                5,
                "Furniture and Furnishings",
                "consumer_spending",
                vec!["furniture", "home"],
            ),
            (
                "DFDHRC1Q027SBEA",
                "q",
                8,
                "Furnishings Durable Equipment",
                "consumer_spending",
                vec!["furniture", "home"],
            ),
            (
                "DCAFRC1A027NBEA",
                "a",
                5,
                "Clothing and Footwear",
                "consumer_spending",
                vec!["apparel"],
            ),
            (
                "DREQRC1Q027SBEA",
                "q",
                8,
                "Recreational Goods",
                "consumer_spending",
                vec!["recreation"],
            ),
        ];

        for (series_id, frequency, limit, name, category, sectors) in series {
            self.service
                .update_fred_series(
                    series_id,
                    frequency,
                    limit,
                    name,
                    category,
                    &sectors.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                )
                .await?;
        }
        Ok(())
    }

    async fn update_bea(&self) -> Result<()> {
        let years = "2026,2025,2024,2023,2022,2021,2020";
        // let years = vec!["2026", "2025", "2024", "2023", "2022", "2021", "2020"];

        let tables: Vec<&str> = vec!["T20100", "T20305", "T20600"];
        // NIPA
        for table in &tables {
            match self.service.update_bea_nipa(table, "A,Q", years).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "BEA NIPA for year: {} table: {} failed: {}",
                        years,
                        table,
                        e
                    );
                    continue;
                }
            };
        }

        let years = vec!["LAST5"];
        let tables: Vec<(&str, &str)> = vec![
            ("CAINC1", "1"),
            ("CAINC4", "10"),
            ("CAINC5N", "10"),
            ("CAGDP1", "1"),
        ];

        self.service
            .update_bea_regional(&tables, "STATE", &years)
            .await?;
        tokio::time::sleep(Duration::from_millis(700)).await;

        self.service
            .update_bea_regional(&tables, "COUNTY", &years)
            .await?;
        tokio::time::sleep(Duration::from_millis(700)).await;

        Ok(())
    }

    async fn update_census(&self) -> Result<()> {
        let variables = [
            // "B19013_001E", // median income
            "B01002_001E", // median age
            "B01003_001E", // population
            "B25003_002E", // owner occupied
            "B25077_001E", // median home value
            "B17001_002E", // below poverty
            "B23025_005E", // unemployed
        ];

        let vars: Vec<&str> = variables.to_vec();
        let years = vec!["2025","2024","2023","2022", "2021", "2020"];

        self.service.update_census("acs5", &vars, years).await?;

        Ok(())
    }
}
