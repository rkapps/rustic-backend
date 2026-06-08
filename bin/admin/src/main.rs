use anyhow::Result;
use rustic_admin::{
    schema::{
        rustic_economic::update_economic_db, rustic_finance::update_finance_db,
        rustic_main::update_rustic_main,
    }, tickers::load::load_tickers,
};
use rustic_core::set_logger;
use std::{env, path::PathBuf};
use tracing::info;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "admin")]
struct Cli {
    #[command(subcommand)]
    command: AdminCommands,
}

#[derive(Subcommand)]
enum AdminCommands {
    LoadTickers {
        #[arg(short, long)]
        file: PathBuf,
    },
    UpdateSchema, // keep last 5 years
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "rustic_admin=info".to_string());
    set_logger(filter);
    let cli = Cli::parse();

    // uri is the same for all
    let mongo_uri = env::var("MONGO_URI").expect("MONGO_URI envrionment variable not set");

    info!("Mongo uri: {}", mongo_uri);
    match cli.command {
        AdminCommands::LoadTickers { file } => {
            info!("Load Tickers PipeLine started...");
            let _ = load_tickers(&mongo_uri, file).await?;
            info!("Load Tickers PipeLine done.");
        }
        AdminCommands::UpdateSchema => {
            update_rustic_main(&mongo_uri).await?;
            update_economic_db(&mongo_uri).await?;
            update_finance_db(&mongo_uri).await?;
        }
    }

    Ok(())
}
