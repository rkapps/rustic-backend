use rustic_core::serialize_vec_or_null;
use rustic_providers::DataPoint;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct FredSeriesEntity {
    pub series_id: String,
    pub category: String,
    pub name: String,
    #[serde(serialize_with = "serialize_vec_or_null")]
    pub observations: Vec<DataPoint>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BeaNipaEntity {
    pub dataset: String,
    pub table_name: String,
    pub series: Vec<BeaNipaSeriesEntity>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BeaNipaSeriesEntity {
    pub code: String,
    #[serde(default)]
    pub description: String,
    pub data: Vec<BeaValueEntity>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BeaValueEntity {
    pub year: String,
    pub value: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BeaRegionalEntity {
    pub code: String,
    #[serde(default)]
    pub description: String,
    pub geos: Vec<BeaGeoEntity>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BeaGeoEntity {
    pub geo_type: String,
    pub geo_name: String,
    pub geo_fips: String,
    pub data: Vec<BeaValueEntity>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CensusEntity {
    pub dataset: String,
    pub variable: String,
    // 👈 Add this field to hold your calculated human description
    #[serde(default)]
    pub description: String,
    pub geos: Vec<CensusGeoEntity>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CensusGeoEntity {
    pub geo_type: String,
    pub geo_name: String,
    pub geo_fips: String,
    pub data: Vec<CensusValueEntity>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CensusValueEntity {
    pub year: String,
    pub value: String,
}
