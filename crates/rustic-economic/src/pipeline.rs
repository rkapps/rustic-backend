#[cfg(feature = "writer")]
use anyhow::Result;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tracing::info;

use crate::{
    core::helper::resolve_years,
    domain::config::{BeaNipaConfig, BeaRegionalConfig, CensusConfig, EconomicConfig, FredConfig},
    service::EconomicService,
};
pub struct EconomicDataPipeline {
    service: Arc<EconomicService>,
}

impl EconomicDataPipeline {
    pub fn new(service: Arc<EconomicService>) -> Self {
        Self { service }
    }

    pub async fn run(&self, pipeline_config: EconomicDataPipelineConfig) -> Result<()> {
        info!("Economic Data Pipeline Config: {:#?}", pipeline_config);

        if pipeline_config.update_fred {
            self.run_fred(pipeline_config.config.clone(), pipeline_config.clean_fred)
                .await?;
        }

        if pipeline_config.update_bea {
            self.run_bea(
                pipeline_config.config.bea_nipa,
                pipeline_config.config.bea_regional,
                pipeline_config.clean_bea,
            )
            .await?;
        }

        if pipeline_config.update_census {
            self.run_census(pipeline_config.config.census, pipeline_config.clean_census)
                .await?;
        }
        Ok(())
    }

    pub async fn run_fred(&self, config: EconomicConfig, clean: bool) -> Result<()> {
        info!("Economic Data Fred Pipeline started...");
        if clean {
            self.service.clean_fred().await?;
        }
        if let Err(e) = self.update_fred(config.fred_series).await {
            tracing::error!("FRED pipeline failed: {}", e);
        } else {
            tracing::info!("FRED pipeline complete");
        }
        Ok(())
    }

    pub async fn run_bea(
        &self,
        bea_nipas: Vec<BeaNipaConfig>,
        bea_regionals: Vec<BeaRegionalConfig>,
        clean: bool,
    ) -> Result<()> {
        info!("Economic Data Bea Pipeline started...");

        if clean {
            self.service.clean_bea().await?;
        }
        if let Err(e) = self.update_bea(bea_nipas, bea_regionals).await {
            tracing::error!("BEA pipeline failed: {}", e);
        } else {
            tracing::info!("BEA pipeline complete");
        }

        Ok(())
    }

    pub async fn run_census(&self, censuss: Vec<CensusConfig>, clean: bool) -> Result<()> {
        info!("Economic Data Census Pipeline started...");

        if clean {
            self.service.clean_census().await?;
        }

        if let Err(e) = self.update_census(censuss).await {
            tracing::error!("Census pipeline failed: {}", e);
        } else {
            tracing::info!("Census pipeline complete");
        }

        Ok(())
    }

    async fn update_fred(&self, fred_series: Vec<FredConfig>) -> Result<()> {
        let limit = 90;
        for series in fred_series {
            match self
                .service
                .update_fred_series(
                    &series.series_id,
                    &series.frequency,
                    limit as usize,
                    &series.name,
                    &series.category,
                )
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Fred Series {} error: {} ", series.series_id, e);
                    continue;
                }
            };
        }
        Ok(())
    }

    async fn update_bea(
        &self,
        bea_nipas: Vec<BeaNipaConfig>,
        bea_regionals: Vec<BeaRegionalConfig>,
    ) -> Result<()> {
        let mut bea_nipam = HashMap::new();
        let year = "LAST5";
        let years = resolve_years(year);
        for bea_nipa in bea_nipas {
            if bea_nipam.contains_key(&bea_nipa.table_name) {
                continue;
            }
            bea_nipam.insert(bea_nipa.table_name.clone(), bea_nipa.table_name.clone());
            for year in years.clone() {
                match self
                    .service
                    .update_bea_nipa(&bea_nipa.table_name, "A,Q", &year)
                    .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(
                            "BEA NIPA for year: {} table: {} failed: {}",
                            year,
                            bea_nipa.table_name,
                            e
                        );
                        continue;
                    }
                };
            }
        }

        for bea_regional in bea_regionals {
            // self.service
            //     .update_bea_regional(&bea_regional.code, &bea_regional.line_code, "00000", &year)
            //     .await?;

            self.service
                .update_bea_regional(&bea_regional.code, &bea_regional.line_code, "STATE", year)
                .await?;
            tokio::time::sleep(Duration::from_millis(700)).await;

            self.service
                .update_bea_regional(&bea_regional.code, &bea_regional.line_code, "COUNTY", year)
                .await?;
        }
        Ok(())
    }

    async fn update_census(&self, censuss: Vec<CensusConfig>) -> Result<()> {
        let years = resolve_years("LAST5");
        let variables: Vec<&str> = censuss.iter().map(|c| c.variable.as_str()).collect();
        self.service
            .update_census("acs5", &variables, &years)
            .await?;
        Ok(())
    }

    // pub async fn run_bea(&self, clean: bool) -> Result<()> {
    //     info!("Economic Data Bea Pipeline started...");

    //     if clean {
    //         self.service.clean_bea().await?;
    //     }
    //     if let Err(e) = self.update_bea().await {
    //         tracing::error!("BEA pipeline failed: {}", e);
    //     } else {
    //         tracing::info!("BEA pipeline complete");
    //     }

    //     Ok(())
    // }

    // pub async fn run_census(&self, clean: bool) -> Result<()> {
    //     info!("Economic Data Census Pipeline started...");

    //     if clean {
    //         self.service.clean_census().await?;
    //     }

    //     if let Err(e) = self.update_census().await {
    //         tracing::error!("Census pipeline failed: {}", e);
    //     } else {
    //         tracing::info!("Census pipeline complete");
    //     }

    //     Ok(())
    // }

    // async fn update_fred(&self) -> Result<()> {
    //     let series = vec![
    //         // (series_id, frequency, limit, name, category, sectors)
    //         (
    //             "CPIAUCSL",
    //             "m",
    //             12,
    //             "Consumer Price Index",
    //             "consumer_health",
    //             vec!["all"],
    //         ),
    //         (
    //             "UMCSENT",
    //             "m",
    //             12,
    //             "Consumer Sentiment",
    //             "consumer_health",
    //             vec!["all"],
    //         ),
    //         (
    //             "UNRATE",
    //             "m",
    //             12,
    //             "Unemployment Rate",
    //             "consumer_health",
    //             vec!["all"],
    //         ),
    //         (
    //             "DSPIC96",
    //             "m",
    //             12,
    //             "Real Disposable Income",
    //             "consumer_health",
    //             vec!["all"],
    //         ),
    //         (
    //             "PCE",
    //             "m",
    //             12,
    //             "Personal Consumption Expenditures",
    //             "consumer_health",
    //             vec!["all"],
    //         ),
    //         (
    //             "HOUST",
    //             "m",
    //             12,
    //             "Housing Starts",
    //             "housing",
    //             vec!["all", "furniture", "home"],
    //         ),
    //         (
    //             "PERMIT",
    //             "m",
    //             12,
    //             "Building Permits",
    //             "housing",
    //             vec!["all", "furniture", "home"],
    //         ),
    //         (
    //             "RSFSXMV",
    //             "m",
    //             12,
    //             "Building Materials Retail",
    //             "consumer_spending",
    //             vec!["furniture", "home"],
    //         ),
    //         (
    //             "DFFFRC1A027NBEA",
    //             "a",
    //             5,
    //             "Furniture and Furnishings",
    //             "consumer_spending",
    //             vec!["furniture", "home"],
    //         ),
    //         (
    //             "DFDHRC1Q027SBEA",
    //             "q",
    //             8,
    //             "Furnishings Durable Equipment",
    //             "consumer_spending",
    //             vec!["furniture", "home"],
    //         ),
    //         (
    //             "DCAFRC1A027NBEA",
    //             "a",
    //             5,
    //             "Clothing and Footwear",
    //             "consumer_spending",
    //             vec!["apparel"],
    //         ),
    //         (
    //             "DREQRC1Q027SBEA",
    //             "q",
    //             8,
    //             "Recreational Goods",
    //             "consumer_spending",
    //             vec!["recreation"],
    //         ),
    //     ];

    //     for (series_id, frequency, limit, name, category, sectors) in series {
    //         self.service
    //             .update_fred_series(
    //                 series_id,
    //                 frequency,
    //                 limit,
    //                 name,
    //                 category,
    //                 &sectors.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    //             )
    //             .await?;
    //     }
    //     Ok(())
    // }

    // async fn update_bea(&self) -> Result<()> {
    //     let years = "2026,2025,2024,2023,2022,2021,2020";
    //     // let years = vec!["2026", "2025", "2024", "2023", "2022", "2021", "2020"];

    //     let tables: Vec<&str> = vec!["T20100", "T20305", "T20600"];
    //     // NIPA
    //     for table in &tables {
    //         match self.service.update_bea_nipa(table, "A,Q", years).await {
    //             Ok(c) => c,
    //             Err(e) => {
    //                 tracing::warn!(
    //                     "BEA NIPA for year: {} table: {} failed: {}",
    //                     years,
    //                     table,
    //                     e
    //                 );
    //                 continue;
    //             }
    //         };
    //     }

    //     let years = vec!["LAST5"];
    //     let tables: Vec<(&str, &str)> = vec![
    //         ("CAINC1", "1"),
    //         ("CAINC4", "10"),
    //         ("CAINC5N", "10"),
    //         ("CAGDP1", "1"),
    //     ];

    //     self.service
    //         .update_bea_regional(&tables, "STATE", &years)
    //         .await?;
    //     tokio::time::sleep(Duration::from_millis(700)).await;

    //     self.service
    //         .update_bea_regional(&tables, "COUNTY", &years)
    //         .await?;
    //     tokio::time::sleep(Duration::from_millis(700)).await;

    //     Ok(())
    // }

    // async fn update_census(&self) -> Result<()> {
    //     let variables = [
    //         // "B19013_001E", // median income
    //         "B01002_001E", // median age
    //         "B01003_001E", // population
    //         "B25003_002E", // owner occupied
    //         "B25077_001E", // median home value
    //         "B17001_002E", // below poverty
    //         "B23025_005E", // unemployed
    //     ];

    //     let vars: Vec<&str> = variables.to_vec();
    //     let years = vec!["2025", "2024", "2023", "2022", "2021", "2020"];

    //     self.service.update_census("acs5", &vars, years).await?;

    //     Ok(())
    // }
}

#[derive(Debug)]
pub struct EconomicDataPipelineConfig {
    pub config: EconomicConfig,
    pub update_fred: bool,
    pub clean_fred: bool,
    pub update_bea: bool,
    pub clean_bea: bool,
    pub update_census: bool,
    pub clean_census: bool,
}

impl EconomicDataPipelineConfig {
    pub fn new(config: EconomicConfig) -> Self {
        EconomicDataPipelineConfig {
            config,
            update_fred: true,
            clean_fred: true,
            update_bea: true,
            clean_bea: true,
            update_census: true,
            clean_census: true,
        }
    }

    pub fn new_fred(config: EconomicConfig, clean: bool) -> Self {
        EconomicDataPipelineConfig {
            config,
            update_fred: true,
            clean_fred: clean,
            update_bea: false,
            clean_bea: false,
            update_census: false,
            clean_census: false,
        }
    }

    pub fn new_bea(config: EconomicConfig, clean: bool) -> Self {
        EconomicDataPipelineConfig {
            config,
            update_fred: false,
            clean_fred: false,
            update_bea: true,
            clean_bea: clean,
            update_census: false,
            clean_census: false,
        }
    }

    pub fn new_census(config: EconomicConfig, clean: bool) -> Self {
        EconomicDataPipelineConfig {
            config,
            update_fred: false,
            clean_fred: false,
            update_bea: false,
            clean_bea: false,
            update_census: true,
            clean_census: clean,
        }
    }
}
