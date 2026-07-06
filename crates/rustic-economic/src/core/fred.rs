use std::{str::FromStr, sync::Arc};

use anyhow::Result;
use chrono::Utc;
use rustic_providers::FredClient;
use tracing::info;

use crate::{
    core::helper::next_refresh,
    domain::{EconomicSeries, EconomicSource, Frequency},
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::FredStorageReader,
        writer::FredStorageWriter,
    },
};

pub async fn get_fred_series(
    reader: Arc<EconomicMongoStorageReader>,
    series_id: &str,
) -> Result<EconomicSeries> {
    let stored = reader.get_series(series_id).await?;
    Ok(stored)
}

pub async fn update_fred_series(
    writer: Arc<EconomicMongoStorageWriter>,
    fred: Arc<FredClient>,
    series_id: &str,
    frequency: &str,
    limit: usize,
    name: &str,
    category: &str,
    sectors: &[String],
) -> Result<()> {
    let data = fred
        .get_series(series_id, Some(frequency), Some(limit))
        .await?;

    let series = EconomicSeries {
        id: series_id.to_string(),
        series_id: series_id.to_string(),
        source: EconomicSource::Fred,
        name: name.to_string(),
        frequency: Frequency::from_str(frequency)?,
        category: category.to_string(),
        sectors: sectors.to_vec(),
        active: true,
        observations: data.data_points,
        last_refreshed: Some(Utc::now()),
        next_refresh: Some(next_refresh(frequency)),
    };
    info!(
        target: "economic-tool",
        "Series: {} observations: {:?}",
        series_id,
        series.observations.len()
    );
    writer.upsert_fred_series(series).await
}
