use anyhow::{Context, Result};
use bin_shared::get_finance_writer_service;
use calamine::{Reader, Xlsx, open_workbook};
use rustic_core::config::load::download_gcs_to_file;
use std::path::PathBuf;

use rustic_finance::domain::dto::ticker_seed::TickerSeed;

pub async fn load_tickers(mongo_uri: &str, file: PathBuf) -> Result<()> {
    let file_path = if file.to_str().unwrap_or("").starts_with("gs://") {
        load_ticker_seeds_from_gcs(file.to_str().unwrap()).await?
    } else {
        file
    };
    let ticker_seeds = load_ticker_seeds_from_file(file_path)?;

    let service = get_finance_writer_service(mongo_uri).await?;
    service.load_tickers(&ticker_seeds, true).await?;

    Ok(())
}

pub fn load_ticker_seeds_from_file(file: PathBuf) -> Result<Vec<TickerSeed>> {
    let mut workbook: Xlsx<_> =
        open_workbook(&file).with_context(|| format!("Failed to open file: {:?}", file))?;

    let sheet = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| anyhow::anyhow!("No sheet found"))??;

    let mut tickers = Vec::new();

    for row in sheet.rows() {
        // skip header row
        let ticker = TickerSeed {
            asset_type: row[0]
                .to_string()
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid asset type: {}", e))?,
            exchange: row[1].to_string(),
            symbol: row[2].to_string(),
            name: row[3].to_string(),
            sector: row[4].to_string(),
            industry: row[5].to_string(),
            overview: row[6].to_string(),
        };
        tickers.push(ticker);
    }

    Ok(tickers)
}

pub async fn load_ticker_seeds_from_gcs(gcs_path: &str) -> anyhow::Result<PathBuf> {
    download_gcs_to_file(gcs_path).await
}
