use anyhow::Result;
use rustic_storage::{Repository, core::index::IndexDefinition, mongo::create_indexes_safe};
use tracing::info;

use crate::storage::mongo::manager::FinanceMongoStorageManager;

pub async fn update_finance_db(mongo_uri: &str, mongo_db: &str) -> Result<()> {
    info!("Updating schema for {} ...", mongo_db);

    let manager = FinanceMongoStorageManager::new(mongo_uri, &mongo_db).await?;

    // tickers
    let repo = manager.tickers().await?;
    let indexes = get_tickers_index_definitions();
    create_indexes_safe(repo, indexes).await?;

    // control
    let repo = manager.ticker_controls().await?;
    let indexes = get_ticker_controls_index_defintions();
    create_indexes_safe(repo, indexes).await?;

    // embeddings
    let repo = manager.ticker_embeddings().await?;
    let indexes = get_ticker_embeddings_index_defintions();
    create_indexes_safe(repo, indexes).await?;

    // history
    // time series always comes first
    let repo = manager.ticker_history().await?;
    let mut repo = repo.lock().await;
    let _ = repo
        .create_time_series_collection("date", "metadata", "hours")
        .await;

    let repo = manager.ticker_history().await?;
    let indexes = get_ticker_history_index_definitions();
    create_indexes_safe(repo, indexes).await?;

    // indicators
    let repo = manager.ticker_indicators().await?;
    let indexes = get_ticker_indicators_index_defintions();
    create_indexes_safe(repo, indexes).await?;

    // news
    let repo = manager.ticker_news().await?;
    let indexes = get_ticker_news_index_defintions();
    create_indexes_safe(repo, indexes).await?;

    // sentiments
    let repo = manager.ticker_sentiments().await?;
    let indexes = get_ticker_sentiments_index_defintions();
    create_indexes_safe(repo, indexes).await?;

    Ok(())
}

fn get_tickers_index_definitions() -> Vec<IndexDefinition> {
    vec![
        get_id_index_definitions(),
        get_symbol_index_definitions(),
        IndexDefinition::new(vec![("sector", 1), ("industry", 1)]).named("idx_sector_industry"),
    ]
}

fn _get_ticker_alphs_index_defintions() -> Vec<IndexDefinition> {
    vec![
        get_id_index_definitions(),
        IndexDefinition::new(vec![("key", 1), ("date", -1)]).named("idx_key_n_date"),
    ]
}

fn get_ticker_controls_index_defintions() -> Vec<IndexDefinition> {
    vec![get_id_index_definitions(), get_symbol_index_definitions()]
}

// embeddings
fn get_ticker_embeddings_index_defintions() -> Vec<IndexDefinition> {
    vec![
        get_id_index_definitions(),
        get_symbol_index_definitions(),
        IndexDefinition::new(vec![("date", 1)]).named("idx_date"),
    ]
}

// history
fn get_ticker_history_index_definitions() -> Vec<IndexDefinition> {
    vec![IndexDefinition::new(vec![("metadata.symbol", 1), ("date", 1)]).named("idx_symbol_date")]
}

// indicator
fn get_ticker_indicators_index_defintions() -> Vec<IndexDefinition> {
    vec![
        get_id_index_definitions(),
        IndexDefinition::new(vec![("symbol", 1), ("date", 1)]).named("idx_symbol_date"),
    ]
}

// news
fn get_ticker_news_index_defintions() -> Vec<IndexDefinition> {
    vec![
        get_id_index_definitions(),
        get_symbol_index_definitions(),
        IndexDefinition::new(vec![("date", 1)]).named("idx_date"),
    ]
}

// sentiments
fn get_ticker_sentiments_index_defintions() -> Vec<IndexDefinition> {
    vec![
        get_id_index_definitions(),
        get_symbol_index_definitions(),
        IndexDefinition::new(vec![("date", 1)]).named("idx_date"),
    ]
}
fn get_id_index_definitions() -> IndexDefinition {
    IndexDefinition::new(vec![("id", 1)])
        .unique()
        .named("idx_id")
        .unique()
}

fn get_symbol_index_definitions() -> IndexDefinition {
    IndexDefinition::new(vec![("symbol", 1)]).named("idx_symbol")
}
