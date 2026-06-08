use std::{env, sync::Arc};

use anyhow::Result;
use rustic_core::set_logger;
use rustic_economic::{pipeline::EconomicDataPipeline, service::EconomicService};

#[tokio::main]

async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "rustic_core=info,rustic_economic=info".to_string());

    set_logger(filter);

    let mongo_uri = env::var("MONGO_URI").expect("MONGO_URI envrionment variable not set");
    let mongo_db =
        env::var("RUSTIC_ECONOMIC_DB_NAME").expect("RUSTIC_AI_DB_NAME envrionment variable not set");

    let economic_service = EconomicService::new(
        &mongo_uri,
        &mongo_db,
        env::var("FRED_API_KEY").ok(),
        env::var("BEA_API_KEY").ok(),
        env::var("CENSUS_API_KEY").ok(),
        
    )
    .await?;
    let pipeline = EconomicDataPipeline::new(Arc::new(economic_service));
    // let _ = pipeline.run_fred(false).await;
    // let _ = pipeline.run_bea(false).await;
    let _ = pipeline.run_census(false).await;

    Ok(())
}
