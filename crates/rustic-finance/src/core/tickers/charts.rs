use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use tracing::debug;

use crate::{
    domain::{TickerIndicator, dto::ticker_chart_entity::TickerChartEntity},
    storage::reader::StorageReader,
};

pub async fn get_ticker_charts_from(
    reader: Arc<dyn StorageReader>,
    symbol: &str,
    date: DateTime<Utc>
) -> Result<Vec<TickerChartEntity>> {
    let indicators = reader
        .get_ticker_indicators_by_symbol(symbol, date)
        .await
        .map_err(|e| anyhow::anyhow!(format!("Get Ticker error: {}", e)))?;

    debug!("Symbol {} indicators: {}", symbol, indicators.len());

    let indicator_map: HashMap<String, TickerIndicator> = indicators
        .iter()
        .map(|t| (t.id.clone(), t.clone()))
        .collect();

    let history = reader
        .get_ticker_history_by_date(symbol, date)
        .await
        .map_err(|e| anyhow::anyhow!(format!("Get Ticker error: {}", e)))?;

    debug!("Symbol {} history: {}", symbol, history.len());

    let charts = history
        .into_iter()
        .filter_map(|b| {
            indicator_map.get(&b.id).map(|val_a| {
                let sma_50 = val_a
                    .values
                    .get("sma_50")
                    .unwrap_or(&Decimal::ZERO)
                    .to_f64()
                    .unwrap_or_default();

                let sma_200 = val_a
                    .values
                    .get("sma_200")
                    .unwrap_or(&Decimal::ZERO)
                    .to_f64()
                    .unwrap_or_default();

                TickerChartEntity {
                    symbol: b.metadata.symbol,
                    date: b.date,
                    close: b.close.to_f64().unwrap_or_default(),
                    sma_50,
                    sma_200,
                }
            })
        })
        .collect();

    Ok(charts)
}
