use anyhow::Result;
use rustic_storage::{core::index::IndexDefinition, mongo::create_indexes_safe};
use tracing::info;

use crate::storage::mongo::manager::EconomicMongoStorageManager;

pub async fn update_economic_db(mongo_uri: &str, mongo_db: &str) -> Result<()> {
    // let mongo_db = env::var("RUSTIC_ECONOMIC_DB_NAME")
    //     .expect("RUSTIC_ECONOMIC_DB_NAME envrionment variable not set");
    info!("Updating schema for {} ...", mongo_db);

    let manager = EconomicMongoStorageManager::new(mongo_uri, mongo_db).await?;

    // fred
    let repo = manager.economic_series().await?;
    let indexes = get_economic_series_index_definitions();
    create_indexes_safe(repo, indexes).await?;

    // bea nipa
    let repo = manager.bea_nipa().await?;
    let indexes = get_economic_bea_nipa_index_definitions();
    create_indexes_safe(repo, indexes).await?;

    // bea regional
    let repo = manager.bea_regional().await?;
    let indexes = get_economic_bea_regional_index_definitions();
    create_indexes_safe(repo, indexes).await?;

    // census
    let repo = manager.census().await?;
    let indexes = get_economic_census_index_definitions();
    create_indexes_safe(repo, indexes).await?;

    Ok(())
}

fn get_economic_series_index_definitions() -> Vec<IndexDefinition> {
    vec![
        IndexDefinition::new(vec![("id", 1)])
            .unique()
            .named("idx_id"),
        IndexDefinition::new(vec![("series_id", 1)]).named("idx_series_id"),
    ]
}

fn get_economic_bea_nipa_index_definitions() -> Vec<IndexDefinition> {
    vec![
        IndexDefinition::new(vec![("id", 1)])
            .unique()
            .named("idx_id"),
        IndexDefinition::new(vec![("table_name", 1), ("time_period", 1)])
            .named("idx_table_name_time_period"),
    ]
}

fn get_economic_bea_regional_index_definitions() -> Vec<IndexDefinition> {
    vec![
        IndexDefinition::new(vec![("id", 1)])
            .unique()
            .named("idx_id"),
        IndexDefinition::new(vec![
            ("code", 1),
            ("time_period", 1),
            ("geo_type", 1),
            ("geo_fips", 1),
        ])
        .named("idx_code_time_period_geo_type_geo_fips"),
    ]
}

fn get_economic_census_index_definitions() -> Vec<IndexDefinition> {
    vec![
        IndexDefinition::new(vec![("id", 1)])
            .unique()
            .named("idx_id"),
        IndexDefinition::new(vec![
            ("dataset", 1),
            ("variable", 1),
            ("year", 1),
            ("geo_type", 1),
            ("geo_fips", 1),
        ])
        .named("idx_dataset_variable_year_geo_type_geo_fips"),
    ]
}
