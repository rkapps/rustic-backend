pub mod bea;
pub mod census;
pub mod fred;
pub mod service;
pub mod traits;
pub mod types;

pub use bea::BeaClient;
pub use census::CensusClient;
pub use fred::FredClient;
pub use service::{EconomicDataService, EconomicDataServiceBuilder};
pub use traits::EconomicProvider;
pub use types::{DataPoint, SeriesData, SeriesInfo};
