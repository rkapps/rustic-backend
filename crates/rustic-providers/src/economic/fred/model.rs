use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FredObservationsResponse {
    pub observations: Vec<FredObservation>,
}

#[derive(Debug, Deserialize)]
pub struct FredObservation {
    pub date: String,
    pub value: String, // FRED returns strings, "." for missing
}

#[derive(Debug, Deserialize)]
pub struct FredSeriesResponse {
    pub seriess: Vec<FredSeriesRecord>,
}

#[derive(Debug, Deserialize)]
pub struct FredSeriesRecord {
    pub id: String,
    pub title: String,
    pub frequency: String,
    pub units: String,
    pub seasonal_adjustment: String,
    pub last_updated: String,
    pub notes: Option<String>,
}
