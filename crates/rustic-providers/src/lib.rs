//! # rustic-providers
//!
//! Async clients for U.S. economic data APIs, unified behind a single
//! [`EconomicProvider`] trait and composable via [`EconomicDataService`].
//!
//! ## Providers
//!
//! | Client | Source | Key data |
//! |---|---|---|
//! | [`FredClient`] | St. Louis Fed (FRED) | Time series — CPI, unemployment, interest rates |
//! | [`BeaClient`] | Bureau of Economic Analysis | GDP, personal income (national & state) |
//! | [`CensusClient`] | U.S. Census Bureau | ACS demographics, poverty, trade statistics |
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use rustic_providers::{EconomicDataService, FredClient, BeaClient};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let service = EconomicDataService::builder()
//!         .with_fred(Arc::new(FredClient::new(std::env::var("FRED_API_KEY")?)?))
//!         .with_bea(Arc::new(BeaClient::new(std::env::var("BEA_API_KEY")?)?))
//!         .build();
//!
//!     // Fetch last 12 months of CPI
//!     let cpi = service.fred_series("CPIAUCSL", Some("m"), Some(12)).await?;
//!     println!("{} data points", cpi.data_points.len());
//!     Ok(())
//! }
//! ```

pub mod economic;
pub mod finance;

pub use economic::bea::BeaClient;
pub use economic::census::CensusClient;
pub use economic::fred::FredClient;
pub use economic::service::{EconomicDataService, EconomicDataServiceBuilder};
pub use economic::traits::EconomicProvider;
pub use economic::types::{DataPoint, SeriesData, SeriesInfo};
