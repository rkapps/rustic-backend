use anyhow::Result;
use bin_shared::get_finance_writer_service;
use chrono::Utc;
use rustic_admin::finance::load_tickers;
use rustic_boot::schema::update_rustic_platform;
use rustic_core::set_logger;
use rustic_economic::schema::update_economic_db;
use rustic_finance::schema::update_finance_db;
use std::{env, path::PathBuf};
use tracing::{error, info};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "admin")]
struct Cli {
    #[command(subcommand)]
    command: AdminCommands,
}

#[derive(Subcommand)]
enum AdminCommands {
    CheckEnv,
    LoadTickers {
        #[arg(short, long)]
        file: PathBuf,
    },
    PruneIndicators, // keep last 5 years
    PruneSentiments, // keep last 30 days
    PruneEmbeddings, // keep last 30 days
    UpdateEconomicSchema,
    UpdateFinanceSchema,
    UpdatePlatformSchema,
    UpdateTickersEOD {
        #[arg(short, long)]
        symbols: Option<String>,
        #[arg(short, long)]
        update: Option<bool>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        "rustic_admin=info,rustic_finance=info,rustic-economic=info".to_string()
    });
    set_logger(filter);
    let cli = Cli::parse();

    // RUSTIC_CORE_MONGO_URI
    // Used for rustic_fiance and rustic_economic
    let rustic_core_mongo_uri = env::var("RUSTIC_CORE_MONGO_URI")
        .expect("RUSTIC_CORE_MONGO_URI envrionment variable not set");
    info!("Rustic Core Mongo uri: {:?}", rustic_core_mongo_uri);

    // RUSTIC_CLIENT_MONGO_URI
    // Used for rustic_platform and client specific data
    let rustic_client_mongo_uri = env::var("RUSTIC_CLIENT_MONGO_URI")
        .expect("RUSTIC_CLIENT_MONGO_URI envrionment variable not set");
    info!("Rustic Client Mongo uri: {:?}", rustic_client_mongo_uri);

    let rustic_platform_mongo_db = env::var("RUSTIC_PLATFORM_DB_NAME")
        .expect("RUSTIC_PLATFORM_DB_NAME envrionment variable not set");
    let rustic_economic_mongo_db = env::var("RUSTIC_ECONOMIC_DB_NAME")
        .expect("RUSTIC_ECONOMIC_DB_NAME envrionment variable not set");
    let rustic_finance_mongo_db = env::var("RUSTIC_FINANCE_DB_NAME")
        .expect("RUSTIC_FINANCE_DB_NAME envrionment variable not set");

    // cli commands
    match cli.command {
        AdminCommands::CheckEnv => {}
        AdminCommands::LoadTickers { file } => {
            info!("Load Tickers PipeLine started...");
            load_tickers(&rustic_core_mongo_uri, file).await?;
            info!("Load Tickers PipeLine done.");
        }
        AdminCommands::PruneEmbeddings => {
            let service = get_finance_writer_service(&rustic_core_mongo_uri).await?;
            info!("Pruning embeddings older than 30 days...");
            let cutoff = Utc::now() - chrono::Duration::days(30);

            match service.delete_ticker_embeddings_before(cutoff).await {
                Ok(_) => info!("Prune embeddings complete"),
                Err(e) => error!("Prune embeddings failed: {:?}", e),
            }
        }

        AdminCommands::PruneIndicators => {
            let service = get_finance_writer_service(&rustic_core_mongo_uri).await?;
            info!("Pruning indicators older than 5 years...");
            let cutoff = Utc::now() - chrono::Duration::days(365 * 5);

            match service.delete_ticker_indicators_before(cutoff).await {
                Ok(_) => info!("Prune indicators complete"),
                Err(e) => error!("Prune indicators failed: {:?}", e),
            }
        }

        AdminCommands::PruneSentiments => {
            let service = get_finance_writer_service(&rustic_core_mongo_uri).await?;
            info!("Pruning sentiments older than 30 days...");
            let cutoff = Utc::now() - chrono::Duration::days(30);

            match service.delete_ticker_sentiments_before(cutoff).await {
                Ok(_) => info!("Prune sentiments complete"),
                Err(e) => error!("Prune sentiments failed: {:?}", e),
            }
        }

        AdminCommands::UpdateEconomicSchema => {
            update_economic_db(&rustic_core_mongo_uri, &rustic_economic_mongo_db).await?;
        }
        AdminCommands::UpdateFinanceSchema => {
            update_finance_db(&rustic_core_mongo_uri, &rustic_finance_mongo_db).await?;
        }
        AdminCommands::UpdatePlatformSchema => {
            update_rustic_platform(&rustic_client_mongo_uri, &rustic_platform_mongo_db).await?;
        }
        AdminCommands::UpdateTickersEOD { symbols, update } => {
            let symbols_str = symbols.as_deref().unwrap_or("");
            let update = update.is_none();
            info!("Tickers EOD PipeLine started...");

            let service = get_finance_writer_service(&rustic_core_mongo_uri).await?;
            match service.update_eod_tickers(symbols_str, update).await {
                Ok(_) => info!("Tickers EOD update completed successfully."),
                Err(e) => error!("Tickers EOD update failed: {:?}", e),
            }
        }
    }

    Ok(())
}
