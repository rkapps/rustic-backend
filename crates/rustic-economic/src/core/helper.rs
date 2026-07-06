use chrono::{Datelike, Duration, Utc};
use rustic_providers::economic::census::model::CensusRecord;

use crate::domain::CensusData;

pub(crate) fn next_refresh(frequency: &str) -> chrono::DateTime<Utc> {
    let now = Utc::now();
    match frequency {
        "m" => now + Duration::days(1),
        "q" => now + Duration::days(7),
        "a" => now + Duration::days(30),
        _ => now + Duration::days(1),
    }
}

pub(crate) fn resolve_years(year: &str) -> Vec<String> {
    let current_year = Utc::now().year(); // latest available BEA year
    match year {
        "LAST5" => (0..5).map(|i| (current_year - i).to_string()).collect(),
        "LAST3" => (0..3).map(|i| (current_year - i).to_string()).collect(),
        "LAST2" => (0..2).map(|i| (current_year - i).to_string()).collect(),
        "LATEST" | "LAST1" => vec![current_year.to_string()],
        _ => year.split(',').map(|y| y.trim().to_string()).collect(),
    }
}

pub fn process_census_records(
    all_records: &mut Vec<CensusData>,
    records: Vec<CensusRecord>,
    dataset: &str,
    year: &str,
    geo_type: &str,
) {
    for record in records {
        let id = format!(
            "census_{}_{}_{}_{}",
            dataset, year, record.variable, record.geo_fips
        );
        let geo_name = record
            .geo_name
            .split(',')
            .next()
            .unwrap_or(&record.geo_name)
            .trim()
            .replace(" County", "")
            .replace(" Parish", "")
            .replace(" Borough", "")
            .replace(" Census Area", "")
            .trim()
            .to_string();

        all_records.push(CensusData {
            id,
            dataset: dataset.to_string(),
            year: year.to_string(),
            variable: record.variable.clone(),
            value: record.value.clone(),
            geo_name,
            geo_fips: record.geo_fips.clone(),
            geo_type: Some(geo_type.to_string()),
            last_refreshed: Utc::now(),
            next_refresh: next_refresh("a"),
        });
    }
}

pub fn get_variable_description(variable_code: &str) -> String {
    match variable_code {
        "B19013_001E" => "Median Income".to_string(),
        "B01002_001E" => "Median Age".to_string(),
        "B01003_001E" => "Total Population".to_string(),
        "B25003_002E" => "Owner Units".to_string(),
        "B25077_001E" => "Median Home Value".to_string(),
        "B17001_002E" => "Poverty Count".to_string(),
        "B23025_005E" => "Unemployment Count".to_string(),
        _ => "Unknown Metric".to_string(),
    }
}

pub fn get_bea_regional_code(table_name: &str) -> String {
    match table_name {
        "CAINC1" => "CAINC1-1".to_string(),
        "CAINC4" => "CAINC4-10".to_string(),
        "CAINC5N" => "CAINC5N-10".to_string(),
        "CAGDP1" => "CAGDP1-1".to_string(),
        _ => "Unknown Metric".to_string(),
    }
}

pub fn get_bea_metric_description(code: &str) -> String {
    match code {
        "CAINC1-1" => "Personal Income".to_string(),
        "CAINC4-10" => "Income Summary".to_string(),
        "CAINC5N-10" => "Work Earnings".to_string(),
        "CAGDP1-1" => "Local GDP".to_string(),
        _ => "Unknown Metric".to_string(),
    }
}
