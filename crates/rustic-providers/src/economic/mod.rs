pub mod bea;
pub mod census;
pub mod fred;
pub mod types;
pub mod traits;
pub mod service;

pub use fred::FredClient;
pub use bea::BeaClient;
pub use census::CensusClient;
pub use traits::EconomicProvider;
pub use types::{SeriesData, DataPoint, SeriesInfo};
pub use service::{EconomicDataService, EconomicDataServiceBuilder};