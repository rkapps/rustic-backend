use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BeaResponse {
    #[serde(rename = "BEAAPI")]
    pub bea_api: BeaApi,
}

#[derive(Debug, Deserialize)]
pub struct BeaApi {
    #[serde(rename = "Results")]
    pub results: Option<BeaResults>,
    #[serde(rename = "Error")]
    pub error: Option<BeaError>,
}


#[derive(Debug, Deserialize)]
pub struct BeaResults {
    #[serde(rename = "Statistic")]
    pub statistic:    Option<String>,
    #[serde(rename = "UnitOfMeasure")]
    pub unit:         Option<String>,
    #[serde(rename = "Data")]
    pub data:         Option<serde_json::Value>,  // ← flexible, handle both
}


#[derive(Debug, Deserialize, Serialize)]
pub struct BeaDataRow {
    #[serde(rename = "TableName")]
    pub table_name: String,
    #[serde(rename = "SeriesCode")]
    pub series_code: String,
    #[serde(rename = "LineNumber")]
    pub line_number: String,
    #[serde(rename = "LineDescription")]
    pub line_description: String,
    #[serde(rename = "TimePeriod")]
    pub time_period: String,
    #[serde(rename = "METRIC_NAME")]
    pub metric_name: String,
    #[serde(rename = "CL_UNIT")]
    pub cl_unit: String,
    #[serde(rename = "UNIT_MULT")]
    pub unit_mult: String,
    #[serde(rename = "DataValue")]
    pub data_value: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BeaRegionalRow {
    #[serde(rename = "Code")]
    pub code:        String,
    #[serde(rename = "GeoFips")]
    pub geo_fips:    String,
    #[serde(rename = "GeoName")]
    pub geo_name:    String,
    #[serde(rename = "TimePeriod")]
    pub time_period: String,
    #[serde(rename = "DataValue")]
    pub data_value:  String,
    #[serde(rename = "CL_UNIT")]
    pub cl_unit:     String,
    #[serde(rename = "UNIT_MULT")]
    pub unit_mult:   String,
}



#[derive(Debug, Deserialize)]
pub struct BeaError {
    #[serde(rename = "APIErrorCode")]
    pub code: String,
    #[serde(rename = "APIErrorDescription")]
    pub description: String,
}