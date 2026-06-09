use chrono::{Duration, Utc};
use rustic_providers::economic::{bea::model::BeaParamValue, census::model::CensusRecord};

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

// pub(crate) fn fips_to_census_geo(geo_fips: &str) -> String {
//     if geo_fips == "00000" {
//         "us:1".to_string()
//     } else if geo_fips.ends_with("000") {
//         // state — 06000 → state:06
//         format!("state:{}", &geo_fips[..2])
//     } else {
//         // county — 06075 → county:075&in=state:06
//         format!("county:{}&in=state:{}", &geo_fips[2..], &geo_fips[..2])
//     }
// }

// pub(crate) fn geo_type(geo_fip: &BeaParamValue) -> &'static str {
//     let key = geo_fip.key.as_str();
//     let name = geo_fip.description.as_str();

//     if key == "00000" {
//         "national"
//     } else if ("91000"..="98000").contains(&key) {
//         "region"
//     } else if key.ends_with("000") {
//         "state"
//     } else if name.contains("Metropolitan") || name.contains("Nonmetropolitan") {
//         "metro"
//     } else if name.contains("Division") {
//         "division"
//     } else {
//         "county"
//     }
// }

pub(crate) fn resolve_years(year: &str) -> Vec<String> {
    let current_year = 2026; // latest available BEA year

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
        let geo_name = record.geo_name
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