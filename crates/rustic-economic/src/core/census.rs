use std::{sync::Arc, time::Duration};

use anyhow::Result;
use rustic_providers::CensusClient;
use tracing::{error, info};

use crate::{
    core::helper::{get_variable_description, process_census_records, resolve_years},
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::CensusStorageReader,
        writer::CensusStorageWriter,
    },
    tools::domain::CensusEntity,
};

pub async fn get_census_data(
    reader: Arc<EconomicMongoStorageReader>,
    variables: Vec<String>,
    dataset: &str,
    geo_fips: Vec<String>,
    geo_type: Option<&str>,
    state_prefix: Option<&str>,
    year: &str,
) -> Result<Vec<CensusEntity>> {
    // expand LAST5 to actual years
    let years = resolve_years(year);
    let mut results = reader
        .get_census_by_dataset_variable(dataset, variables, geo_fips, geo_type, state_prefix, years)
        .await?;

    // add the description
    for result in &mut results {
        result.description = get_variable_description(&result.variable);
    }
    Ok(results)
}

pub async fn update_census(
    writer: Arc<EconomicMongoStorageWriter>,
    census: Arc<CensusClient>,
    dataset: &str,
    variables: &[&str],
    years: &[String],
) -> Result<()> {
    let mut vars = vec!["NAME"];
    vars.extend_from_slice(variables);

    for year in years {
        let mut all_records = Vec::new();

        // One call for the county
        let state_records = census.get_acs(year, "acs5", &vars, "us:1").await?;
        process_census_records(&mut all_records, state_records, dataset, year);

        // One call for all states
        let state_records = census.get_acs(year, "acs5", &vars, "state:*").await?;
        process_census_records(&mut all_records, state_records, dataset, year);

        tokio::time::sleep(Duration::from_millis(500)).await;

        // One call for all counties nationwide
        let county_records = match census
            .get_acs(year, "acs5", &vars, "county:*&in=state:*")
            .await
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    "Census for year: {} dataset: {} variables {:?} failed: {}",
                    year,
                    "acs5",
                    vars,
                    e
                );
                continue;
            }
        };
        process_census_records(&mut all_records, county_records, dataset, year);

        tokio::time::sleep(Duration::from_millis(500)).await;

        info!("all records for year: {} - {}", year, all_records.len());
        match writer.upsert_census_bulk(all_records).await {
            Ok(_) => {}
            Err(e) => error!("Update census_bulk error: {}", e),
        };
    }

    Ok(())
}
