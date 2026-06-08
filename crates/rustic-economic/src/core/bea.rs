use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use rustic_providers::{BeaClient, economic::bea::model::BeaParamValue};
use tracing::{error, info};

use crate::{
    core::helper::{geo_type, next_refresh, resolve_years},
    domain::{BeaNipaData, BeaRegionalData},
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::BeaStorageReader,
        writer::BeaStorageWriter,
    },
};

pub async fn get_geo_fips(bea: BeaClient) -> Result<Vec<BeaParamValue>> {
    bea.get_geo_fips().await
}

pub async fn get_bea_nipa(
    reader: Arc<EconomicMongoStorageReader>,
    table_name: &str,
    year: &str,
) -> Result<Vec<BeaNipaData>> {
    let years: Vec<String> = if year == "LAST5" {
        vec![
            "2025".to_string(),
            "2024".to_string(),
            "2023".to_string(),
            "2022".to_string(),
            "2021".to_string(),
        ]
    } else {
        year.split(',').map(|y| y.trim().to_string()).collect()
    };
    let mut result = Vec::new();

    for y in &years {
        let stored = reader.get_bea_nipa_by_table(table_name, y).await?;
        result.extend(stored);
    }

    Ok(result)
}

pub async fn get_bea_regional(
    reader: Arc<EconomicMongoStorageReader>,
    table_name: &str,
    geo_fips: Option<&str>,
    geo_type: Option<&str>,
    state_prefix: Option<&str>,
    year: &str,
) -> Result<Vec<BeaRegionalData>> {
    let years = resolve_years(year);
    let mut result = Vec::new();

    for y in &years {
        let stored = reader
            .get_bea_regional_filtered(table_name, geo_fips, geo_type, state_prefix, y)
            .await?;
        result.extend(stored);
    }

    Ok(result)
}

pub async fn update_bea_nipa(
    writer: Arc<EconomicMongoStorageWriter>,
    bea: Arc<BeaClient>,
    table_name: &str,
    frequency: &str,
    year: &str,
) -> Result<()> {
    info!(
        "Bea nipa table_name: {} frequency: {} years: {}",
        table_name, frequency, year,
    );

    let rows = bea.get_nipa(table_name, frequency, year).await?;
    let mut all_records = Vec::new();
    for row in &rows {
        let id = format!(
            "bea_nipa_{}_{}_{}",
            table_name, row.series_code, row.time_period
        );

        let new_record = BeaNipaData {
            id,
            table_name: row.table_name.clone(),
            series_code: row.series_code.clone(),
            line_number: row.line_number.clone(),
            line_description: row.line_description.clone(),
            time_period: row.time_period.clone(),
            metric_name: row.metric_name.clone(),
            cl_unit: row.cl_unit.clone(),
            unit_mult: row.unit_mult.clone(),
            data_value: row.data_value.clone(),
            last_refreshed: Utc::now(),
            next_refresh: next_refresh("m"),
        };
        all_records.push(new_record);
    }
    match writer.upsert_bea_nipa_bulk(all_records).await {
        Ok(c) => c,
        Err(e) => error!("Update bea_nipa bulk error: {}", e),
    };
    Ok(())
}

pub async fn update_bea_regional(
    writer: Arc<EconomicMongoStorageWriter>,
    bea: Arc<BeaClient>,
    tables: Vec<(&str, &str)>,
    geo_fips: &[BeaParamValue],
    years: &[&str],
) -> Result<()> {
    // loop through the years
    for year in years {
        // loop through the tables
        for table in &tables {
            let mut all_rows = Vec::new();

            // loop through the geo-fips
            for (i, geo_fip) in geo_fips.iter().enumerate() {
                let rows = match bea.get_regional(table.0, table.1, &geo_fip.key, year).await {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(
                            "BEA Regional for year: {} table: {} geo_flip {:?} failed: {}",
                            year,
                            table.0,
                            geo_fip.key,
                            e
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(10000)).await;

                        match bea.get_regional(table.0, table.1, &geo_fip.key, year).await {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::warn!(
                                    "BEA Regional for year: {} table: {} geo_flip {:?} failed: {}",
                                    year,
                                    table.0,
                                    geo_fip.key,
                                    e
                                );
                                Vec::new()
                            }
                        }
                    }
                };

                for row in rows {
                    let id = format!(
                        "bea_regional_{}_{}_{}",
                        table.0, row.geo_fips, row.time_period
                    );
                    let new_row = BeaRegionalData {
                        id,
                        code: row.code.clone(),
                        geo_fips: row.geo_fips.clone(),
                        geo_name: row.geo_name.clone(),
                        geo_type: geo_type(geo_fip).to_owned(),
                        time_period: row.time_period.clone(),
                        data_value: row.data_value.clone(),
                        cl_unit: row.cl_unit.clone(),
                        unit_mult: row.unit_mult.clone(),
                        last_refreshed: Utc::now(),
                        next_refresh: next_refresh("a"),
                    };
                    all_rows.push(new_row);
                }

                if i % 20 == 0 {
                    info!("i: {} geo_fip: {}", i, geo_fip.key);
                    tokio::time::sleep(tokio::time::Duration::from_millis(10000)).await;
                }
            }

            info!(
                "all records for year: {} table: {} - {}",
                year,
                table.0,
                all_rows.len()
            );

            match writer.upsert_bea_regional_bulk(all_rows).await {
                Ok(c) => c,
                Err(e) => error!("Update census_bulk error: {}", e),
            };
        }
    }

    Ok(())
}
