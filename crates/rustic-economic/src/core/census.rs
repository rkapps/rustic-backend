use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use rustic_providers::{CensusClient, economic::bea::model::BeaParamValue};
use tracing::{error, info};

use crate::{
    core::helper::{fips_to_census_geo, next_refresh, resolve_years},
    domain::CensusData,
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::CensusStorageReader,
        writer::CensusStorageWriter,
    },
};

pub async fn get_census_data(
    reader: Arc<EconomicMongoStorageReader>,
    variables: &[&str],
    dataset: &str,
    geo_fips: Option<&str>,
    geo_type: Option<&str>,
    state_prefix: Option<&str>,
    year: &str,
) -> Result<Vec<CensusData>> {
    // expand LAST5 to actual years
    let years = resolve_years(year);

    let mut result = Vec::new();

    for y in &years {
        for variable in variables {
            let stored = reader
                .get_census_filtered(dataset, variable, geo_fips, geo_type, state_prefix, y)
                .await?;

            result.extend(stored);
        }
    }
    Ok(result)
}

pub async fn update_census(
    writer: Arc<EconomicMongoStorageWriter>,
    census: Arc<CensusClient>,
    dataset: &str,
    variables: &[&str],
    years: Vec<&str>,
    geo_fips: Vec<BeaParamValue>,
) -> Result<()> {
    for year in &years {
        let mut vars = vec!["NAME"];
        vars.extend_from_slice(variables);

        let mut all_records = Vec::new();

        for (i, geo_fip) in geo_fips.iter().enumerate() {
            // skip divisions and metro portions
            if geo_fip.key == "division" || geo_fip.key == "metro" {
                continue;
            }

            let census_geo = fips_to_census_geo(&geo_fip.key);
            if i % 40 == 0 {
                info!(
                    "i: {} Census dataset: {} variables: {:?} geo: {} years: {}",
                    i, dataset, vars, geo_fip.key, year,
                );
            }

            let records = match census.get_acs(year, "acs5", &vars, &census_geo).await {
                Ok(c) => c,
                Err(_) => continue,
            };

            for record in &records {
                let id = format!(
                    "census_{}_{}_{}_{}",
                    dataset, year, record.variable, geo_fip.key
                );
                let geo_name = record.geo_name.clone();
                // "San Francisco County, California" → "San Francisco"
                let geo_name = geo_name
                    .split(',')
                    .next()
                    .unwrap_or(&geo_name)
                    .trim()
                    .replace(" County", "")
                    .replace(" Parish", "") // Louisiana uses Parish
                    .replace(" Borough", "") // Alaska uses Borough
                    .replace(" Census Area", "") // Alaska
                    .trim()
                    .to_string();
                let new_record = CensusData {
                    id,
                    dataset: dataset.to_string(),
                    year: year.to_string(),
                    variable: record.variable.clone(),
                    value: record.value.clone(),
                    geo_name,
                    geo_fips: geo_fip.key.clone(),
                    geo_type: record.geo_type.clone(),
                    last_refreshed: Utc::now(),
                    next_refresh: next_refresh("a"),
                };
                all_records.push(new_record);
            }
        }

        info!("all records for year: {} - {}", year, all_records.len());
        match writer.upsert_census_bulk(all_records).await {
            Ok(c) => c,
            Err(e) => error!("Update census_bulk error: {}", e),
        };
    }

    Ok(())
}
