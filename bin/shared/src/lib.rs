use anyhow::Result;
use rustic_core::load_content;
use rustic_economic::{domain::config::EconomicConfig, service::EconomicService};
use tracing::info;
use std::{env, sync::Arc};

use rustic_finance::service::FinanceService;
use rustic_ml::embeddings::openai::OpenAIEmbeddingClient;

pub async fn get_finance_reader_service(mongo_uri: &str) -> Result<FinanceService> {
    let mongo_db = env::var("RUSTIC_FINANCE_DB_NAME")
        .expect("RUSTIC_FINANCE_DB_NAME envrionment variable not set");
    let openai_api_key: String =
        env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

    let embedding_client = Arc::new(OpenAIEmbeddingClient::new(&openai_api_key)?);

    FinanceService::new_reader(
        mongo_uri,
        &mongo_db,
        embedding_client,
    )
    .await
}


pub async fn get_finance_writer_service(mongo_uri: &str) -> Result<FinanceService> {
    let mongo_db = env::var("RUSTIC_FINANCE_DB_NAME")
        .expect("RUSTIC_FINANCE_DB_NAME envrionment variable not set");
    let openai_api_key: String =
        env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

    let embedding_client = Arc::new(OpenAIEmbeddingClient::new(&openai_api_key)?);
    let alpha_key =
        env::var("ALPHA_API_KEY").expect("ALPHA_API_KEY not found in environment variables.");
    let tiingo_token =
        env::var("TIINGO_API_TOKEN").expect("TIINGO_API_TOKEN not found in environment variables.");
    let coinmarketcap_key = env::var("COINMARKETCAP_API_KEY")
        .expect("COINMARKETCAP_API_KEY not found in environment variables.");

    FinanceService::new(
        mongo_uri,
        &mongo_db,
        embedding_client,
        Some(alpha_key),
        Some(tiingo_token),
        Some(coinmarketcap_key),
    )
    .await
}

pub async fn get_economic_reader_service(mongo_uri: &str, config_dir: &str, file_name: &str ) -> Result<EconomicService> {
    let mongo_db = env::var("RUSTIC_ECONOMIC_DB_NAME")
        .expect("RUSTIC_AI_DB_NAME envrionment variable not set");

    let path = format!("{}/{}", config_dir, file_name);

    info!(
        "Economic data Mongo uri: {:?} db: {:?} Config dir: {}",
        mongo_uri, mongo_db, path
    );

    let economic_config_content = load_content(path).await?;
    let economic_config: EconomicConfig = serde_json::from_str(&economic_config_content)?;
    
    EconomicService::new_reader(
        mongo_uri,
        &mongo_db,
        economic_config
    ).await
}


pub async fn get_economic_writer_service(mongo_uri: &str, config_dir: &str, file_name: &str) -> Result<EconomicService> {
    let mongo_db = env::var("RUSTIC_ECONOMIC_DB_NAME")
        .expect("RUSTIC_AI_DB_NAME envrionment variable not set");

    let path = format!("{}/{}", config_dir, file_name);
    let economic_config_content = load_content(path).await?;
    let economic_config: EconomicConfig = serde_json::from_str(&economic_config_content)?;

    EconomicService::new(
        mongo_uri,
        &mongo_db,
        env::var("FRED_API_KEY").ok(),
        env::var("BEA_API_KEY").ok(),
        env::var("CENSUS_API_KEY").ok(),
        economic_config
    )
    .await
}
