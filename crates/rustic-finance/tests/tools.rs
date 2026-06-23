use std::{env, sync::Arc};

use anyhow::Result;
use rustic_core::Tool;
use rustic_finance::{
    storage::{
        FinanceMongoStorageReader, TickerStorageReader, mongo::manager::FinanceMongoStorageManager,
    },
    tools::{ticker_screening::TickerScreeningTool, ticker_snapshot::TickerSnapshotTool},
};
use rustic_ml::embeddings::openai::OpenAIEmbeddingClient;
use serde_json::json;
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
async fn test_tool_snapshot() -> Result<()> {
    let manager = get_manager().await?;
    let reader = FinanceMongoStorageReader::new(manager);

    let tool = TickerSnapshotTool::new(Arc::new(reader.clone()));
    let value = json!({"symbols":["STX","WDC","NTAP","PSTG"]});

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_snapshot.json", &json).await?;

    // println!("{:#?}", taxonomy);
    Ok(())
}

#[tokio::test]
async fn test_tool_screening() -> Result<()> {
    let manager = get_manager().await?;
    let reader = FinanceMongoStorageReader::new(manager);
    let openai_api_key: String =
        env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

    let embedding_client = Arc::new(OpenAIEmbeddingClient::new(&openai_api_key)?);

    let tool = TickerScreeningTool::new(Arc::new(reader.clone()), embedding_client);
    let value = json!({"assets_cap_range": "large", "industry": "Banks - Regional - US"});

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_screening.json", &json).await?;

    // println!("{:#?}", taxonomy);
    Ok(())
}
