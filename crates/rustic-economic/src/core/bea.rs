use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use rustic_providers::{BeaClient, economic::bea::model::BeaParamValue};
use tracing::{error, info};

use crate::{
    core::helper::{get_bea_metric_description, next_refresh, resolve_years},
    domain::bea::{BeaNipa, BeaRegional},
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::BeaStorageReader,
        writer::BeaStorageWriter,
    },
    tools::domain::{BeaNipaEntity, BeaRegionalEntity},
};

pub async fn get_geo_fips(bea: BeaClient) -> Result<Vec<BeaParamValue>> {
    bea.get_geo_fips().await
}

pub async fn get_bea_nipa(
    reader: Arc<EconomicMongoStorageReader>,
    table_name: &str,
    series_codes: Vec<String>,
    year: &str,
) -> Result<Vec<BeaNipaEntity>> {
    let years: Vec<String> = resolve_years(year);

    info!(
        target: "economic-tool",
        "Bea nipa table_name: {:?} years: {:?}",
        table_name, years,
    );
    let result = reader
        .get_bea_nipa_by_table_series(table_name.to_string(), series_codes, years)
        .await?;
    Ok(result)
}

pub async fn get_bea_regional(
    reader: Arc<EconomicMongoStorageReader>,
    code: &str,
    line_codes: Vec<String>,
    geo_fips: Vec<String>,
    geo_type: Option<&str>,
    state_prefix: Option<&str>,
    year: &str,
) -> Result<Vec<BeaRegionalEntity>> {
    let years = resolve_years(year);

    let mut codes = Vec::new();
    for line_code in line_codes {
        let code = format!("{}-{}", code, line_code);
        codes.push(code);
    }

    let mut results = reader
        .get_bea_regional_by_table_series(codes, years, geo_fips, geo_type, state_prefix)
        .await?;
    // add the description
    for result in &mut results {
        result.description = get_bea_metric_description(&result.code);
    }
    Ok(results)
}

pub async fn update_bea_nipa(
    writer: Arc<EconomicMongoStorageWriter>,
    bea: Arc<BeaClient>,
    table_name: &str,
    frequency: &str,
    year: &str,
) -> Result<()> {
    info!(
        target: "economic-tool",
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

        let new_record = BeaNipa {
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
    code: &str,
    line_code: &str,
    geo_fips: &str,
    year: &str,
) -> Result<()> {
    let mut all_rows = Vec::new();
    let rows = bea.get_regional(code, line_code, geo_fips, year).await?;

    for row in rows {
        let id = format!(
            "bea_regional_{}_{}_{}_{}",
            code, line_code, row.geo_fips, row.time_period
        );

        let geo_type = if row.geo_fips.eq("00000") {
            "US"
        } else {
            geo_fips
        };
        let new_row = BeaRegional {
            id,
            code: row.code.clone(),
            geo_fips: row.geo_fips.clone(),
            geo_name: row.geo_name.clone(),
            geo_type: geo_type.to_string(),
            time_period: row.time_period.clone(),
            data_value: row.data_value.clone(),
            cl_unit: row.cl_unit.clone(),
            unit_mult: row.unit_mult.clone(),
            last_refreshed: Utc::now(),
            next_refresh: next_refresh("a"),
        };
        all_rows.push(new_row);
    }

    info!(
        "all records for year: {} code: {} linecode: {} - {}",
        year,
        code,
        line_code,
        all_rows.len()
    );

    match writer.upsert_bea_regional_bulk(all_rows).await {
        Ok(c) => c,
        Err(e) => error!("Update census_bulk error: {}", e),
    };
    Ok(())
}
