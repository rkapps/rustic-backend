use chrono::{Datelike, Duration, Utc};
use rustic_providers::economic::census::model::CensusRecord;

use crate::domain::census::Census;

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
    all_records: &mut Vec<Census>,
    records: Vec<CensusRecord>,
    dataset: &str,
    year: &str,
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

        all_records.push(Census {
            id,
            dataset: dataset.to_string(),
            year: year.to_string(),
            variable: record.variable.clone(),
            value: record.value.clone(),
            geo_name,
            geo_fips: record.geo_fips.clone(),
            geo_type: record.geo_type.clone(),
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

// pub fn get_bea_regional_code(table_name: &str) -> String {
//     match table_name {
//         "CAINC1" => "CAINC1-1".to_string(),
//         "CAINC4" => "CAINC4-10".to_string(),
//         "CAINC5N" => "CAINC5N-10".to_string(),
//         "CAGDP1" => "CAGDP1-1".to_string(),
//         _ => "Unknown Metric".to_string(),
//     }
// }

pub fn get_bea_metric_description(code: &str) -> String {
    match code {
        "CAINC1-1" => "Personal Income by State".to_string(),
        "CAINC5N-700" => "Retail Trade Total Earnings by State".to_string(),
        "CAINC5N-701" => "Motor Vehicle and Parts Dealers Earnings by State".to_string(),
        "CAINC5N-702" => "Furniture and Home Furnishings Stores Earnings by State".to_string(),
        "CAINC5N-703" => "Electronics and Appliance Stores Earnings by State".to_string(),
        "CAINC5N-704" => {
            "Building Material and Garden Equipment Dealers Earnings by State".to_string()
        }
        "CAINC5N-705" => "Food and Beverage Stores Earnings by State".to_string(),
        "CAINC5N-708" => "Clothing and Clothing Accessories Stores Earnings by State".to_string(),
        "CAINC5N-709" => {
            "Sporting Goods, Hobby, Musical Instrument and Book Stores Earnings by State"
                .to_string()
        }
        "CAINC5N-711" => "General Merchandise Stores Earnings by State".to_string(),
        "CAINC5N-521" => {
            "Furniture and Related Product Manufacturing Earnings by State".to_string()
        }
        "CAINC5N-535" => "Apparel Manufacturing Earnings by State".to_string(),
        "CAINC5N-1800" => "Accommodation and Food Services Earnings by State".to_string(),
        "CAINC5N-1802" => "Food Services and Drinking Places Earnings by State".to_string(),
        "CAINC5N-1700" => "Arts, Entertainment, and Recreation Earnings by State".to_string(),
        "CAGDP1-1" => "GDP by State".to_string(),
        _ => "Unknown Metric".to_string(),
    }
}
