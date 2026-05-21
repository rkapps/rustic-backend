pub mod economic;
pub mod finance;

pub use economic::traits::EconomicProvider;
pub use economic::types::{SeriesData, DataPoint, SeriesInfo};
pub use economic::service::{EconomicDataService, EconomicDataServiceBuilder};
pub use economic::fred::FredClient;
pub use economic::bea::BeaClient;   
pub use economic::census::CensusClient; 