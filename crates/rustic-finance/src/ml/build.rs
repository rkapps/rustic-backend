use crate::{
    core::helper::get_tickers_for_symbols,
    ml::labeller::build_labels,
    storage::{
        FinanceMongoStorageReader, mongo::writer::FinanceMongoStorageWriter,
        reader::TickerIndicatorStorageReader,
    },
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rustic_ml::ml::predictions::models::lr::LinearRegressionModel;
use std::sync::Arc;
use tracing::{info, warn};

pub async fn build_ticker_prediction_models(
    reader: &Arc<FinanceMongoStorageReader>,
    _writer: &Arc<FinanceMongoStorageWriter>,
    symbols: &str,
    from_date: DateTime<Utc>,
    periods: &[usize],
    min_samples: usize,
) -> Result<()> {
    let tickers = get_tickers_for_symbols(reader, symbols).await?;
    let length = tickers.len();
    for (i, ticker) in tickers.iter().enumerate() {
        info!(
            target: "ml",
            "Training Ticker: {} {}/{}", ticker.symbol, i + 1, length
        );
        let indicators = reader
            .get_ticker_indicators_by_symbol(&ticker.symbol, from_date)
            .await?;
        if indicators.len() < min_samples {
            warn!(
                target: "ml",
                "Ticker {} insufficient samples: {} rows",
                ticker.symbol,
                indicators.len()
            );
            continue;
        }

        for period in periods {
            let samples = build_labels(ticker, &indicators, *period);
            info!(
                target: "ml",
                "  Period: {} samples: {}", period, samples.len()
            );

            LinearRegressionModel::train(samples)?;
        }
    }

    Ok(())
}
