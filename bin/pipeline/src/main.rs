use std::{env, sync::Arc};

use anyhow::Result;
use bin_shared::{get_economic_service, get_finance_service};
use clap::{Parser, Subcommand};
use rustic_core::set_logger;
use rustic_economic::pipeline::EconomicDataPipeline;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "pipeline")]
struct Cli {
    #[command(subcommand)]
    command: PipelineCommands,
}

#[derive(Subcommand)]
#[allow(clippy::enum_variant_names)]
enum PipelineCommands {
    UpdateEconomicData,
    UpdateTickersEod,
    UpdateStocksEtfsRealtime,
    UpdateCryptosRealtime,
    UpdateTickersNews,
}

#[tokio::main]

async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "rustic_core=info,rustic_economic=info".to_string());

    set_logger(filter);
    let cli = Cli::parse();

    // uri is the same for all
    let mongo_uri = env::var("MONGO_URI").expect("MONGO_URI envrionment variable not set");

    match cli.command {
        PipelineCommands::UpdateEconomicData => {
            let economic_service = get_economic_service(&mongo_uri).await?;
            let pipeline = EconomicDataPipeline::new(Arc::new(economic_service));
            let _ = pipeline.run_fred(false).await;
            let _ = pipeline.run_bea(false).await;
            let _ = pipeline.run_census(false).await;
        }
        PipelineCommands::UpdateTickersEod => {
            info!("Tickers EOD PipeLine started...");

            let service = get_finance_service(&mongo_uri).await?;
            match service.update_eod_tickers("", true).await {
                Ok(_) => info!("Tickers EOD update completed successfully."),
                Err(e) => error!("Tickers EOD update failed: {:?}", e),
            }
        }
        PipelineCommands::UpdateStocksEtfsRealtime => {
            let service = get_finance_service(&mongo_uri).await?;
            match service.update_realtime_stocks_etfs("", true).await {
                Ok(_) => info!("Tickers EOD update completed successfully."),
                Err(e) => error!("Tickers EOD update failed: {:?}", e),
            }
        }
        PipelineCommands::UpdateCryptosRealtime => {
            let service = get_finance_service(&mongo_uri).await?;
            match service.update_realtime_cryptos("", true).await {
                Ok(_) => info!("Tickers EOD update completed successfully."),
                Err(e) => error!("Tickers EOD update failed: {:?}", e),
            }
        }
        PipelineCommands::UpdateTickersNews => {
            info!("Tickers News PipeLine started...");
            let service = get_finance_service(&mongo_uri).await?;
            match service.update_tickers_news().await {
                Ok(_) => info!("Tickers News update completed successfully."),
                Err(e) => error!("Tickers News update failed: {:?}", e),
            }
        }
    }

    Ok(())
}
