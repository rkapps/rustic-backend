use std::{env, sync::Arc};

use anyhow::Result;
use rustic_core::set_logger;
use rustic_economic::{
    pipeline::EconomicDataPipeline, service::EconomicDataService, storage::EconomicStorageManager,
};
use rustic_providers::{BeaClient, CensusClient, FredClient};

#[tokio::main]

async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "rustic_core=info,rustic_economic=info".to_string());

    set_logger(filter);
    // Find these again for rusticai
    let mongo_db =
        env::var("RUSTIC_AI_DB_NAME").expect("RUSTIC_AI_DB_NAME envrionment variable not set");
    let mongo_uri = env::var("MONGO_URI").expect("MONGO_URI envrionment variable not set");

    let fred_api_key = env::var("FRED_API_KEY").expect("FRED_API_KEY environment variable not set");
    let fred_client = Arc::new(FredClient::new(fred_api_key)?);

    let census_api_key =
        env::var("CENSUS_API_KEY").expect("CENSUS_API_KEY environment variable not set");
    let census_client = Arc::new(CensusClient::new(census_api_key)?);

    let bea_api_key = env::var("BEA_API_KEY").expect("BEA_API_KEY environment variable not set");
    let bea_client = Arc::new(BeaClient::new(bea_api_key)?);
    let economic_storage_manager = EconomicStorageManager::new(&mongo_uri, &mongo_db).await?;
    let economic_data_service = EconomicDataService::new(
        Arc::new(economic_storage_manager),
        fred_client,
        bea_client,
        census_client,
    );
    let pipeline = EconomicDataPipeline::new(Arc::new(economic_data_service));
    // let _ = pipeline.clean().await;
    // let _ = pipeline.run().await;

    Ok(())
}
