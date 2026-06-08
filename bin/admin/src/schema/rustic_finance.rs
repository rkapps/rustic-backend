use anyhow::Result;
use std::env;
use tracing::info;


pub async fn update_finance_db(mongo_uri: &str) -> Result<()> {
    let mongo_db = env::var("RUSTIC_FINANCE_DB_NAME")
        .expect("RUSTIC_FINANCE_DB_NAME envrionment variable not set");
    info!("Updating schema for {} ...", mongo_db);
    Ok(())
}