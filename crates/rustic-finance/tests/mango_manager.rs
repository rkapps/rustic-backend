use std::env;

use anyhow::Result;
use rustic_finance::storage::{
    FinanceMongoStorageReader, TickerStorageReader, mongo::manager::FinanceMongoStorageManager,
};
use tokio::fs;
use tracing::info;

pub(crate) async fn get_manager() -> Result<FinanceMongoStorageManager> {
    let mongo_uri = env::var("MONGO_URI").expect("MONGO_URI envrionment variable not set");
    info!("Mongo uri: {}", mongo_uri);

    // uri is the same for all
    let mongo_db = env::var("RUSTIC_FINANCE_DB_NAME")
        .expect("RUSTIC_FINANCE_DB_NAME envrionment variable not set");
    info!("Finance Data Mongo uri: {:?} db: {:?}", mongo_uri, mongo_db);

    let manager = FinanceMongoStorageManager::new(&mongo_uri, &mongo_db).await;

    manager
}

#[tokio::test]
async fn test_get_snapshots() -> Result<()> {
    let manager = get_manager().await?;
    let reader = FinanceMongoStorageReader::new(manager);

    let tickers = reader
        .get_tickers_by_symbols(vec!["NVDA".to_string(), "AAPL".to_string()])
        .await?;

    // save to fixture
    let json = serde_json::to_string_pretty(&tickers).unwrap();
    fs::write("tests/fixtures/tickers.json", &json).await?;

    assert!(!tickers.is_empty());
    // println!("{:#?}", taxonomy);
    Ok(())
}
