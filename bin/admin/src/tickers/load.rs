use anyhow::Result;
use rustic_ml::embeddings::openai::OpenAIEmbeddingClient;
use tracing::info;
use std::{env, path::PathBuf, sync::Arc};

use rustic_finance::service::FinanceService;

use crate::tickers::seed::{load_ticker_seeds_from_file, load_ticker_seeds_from_gcs};

pub async fn load_tickers(mongo_uri: &str, file: PathBuf) -> Result<()> {

    let file_path = if file.to_str().unwrap_or("").starts_with("gs://") {
        load_ticker_seeds_from_gcs(file.to_str().unwrap()).await?
    } else {
        file
    };
    let ticker_seeds = load_ticker_seeds_from_file(file_path)?;

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


    let service = FinanceService::new(
        &mongo_uri,
        &mongo_db,
        embedding_client,
        Some(alpha_key),
        Some(tiingo_token),
        Some(coinmarketcap_key),
    )
    .await?;

    info!("reached");
    let _ = service.load_tickers(&ticker_seeds, true).await?;

    Ok(())
}
