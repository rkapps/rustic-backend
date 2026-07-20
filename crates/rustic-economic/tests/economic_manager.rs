use std::env;

use anyhow::Result;
use rustic_economic::storage::mongo::manager::EconomicMongoStorageManager;
use tracing::info;

pub async fn get_manager() -> Result<EconomicMongoStorageManager> {
    let mongo_uri = env::var("MONGO_URI").expect("MONGO_URI envrionment variable not set");
    info!("Mongo uri: {}", mongo_uri);

    // uri is the same for all
    let mongo_db = env::var("RUSTIC_ECONOMIC_DB_NAME")
        .expect("RUSTIC_ECONOMIC_DB_NAME envrionment variable not set");
    info!(
        "Economic Data Mongo uri: {:?} db: {:?}",
        mongo_uri, mongo_db
    );

    let manager = EconomicMongoStorageManager::new(&mongo_uri, &mongo_db).await;
    manager
}
