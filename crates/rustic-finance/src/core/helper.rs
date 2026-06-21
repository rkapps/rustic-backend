use std::sync::Arc;

use anyhow::Result;
use crate::{domain::Ticker, storage::{FinanceMongoStorageReader, TickerStorageReader}};


pub(crate) async fn get_tickers_for_symbols(
    reader: &Arc<FinanceMongoStorageReader>,
    symbols: &str,
) -> Result<Vec<Ticker>> {
    if !symbols.is_empty() {
        use tracing::debug;

        let list: Vec<String> = symbols.split(',').map(|s| s.to_string()).collect();
        debug!("List: {:?}", list);
        reader.get_tickers_by_symbols(list).await
    } else {
        reader.get_tickers_by_total_assets().await
    }
}