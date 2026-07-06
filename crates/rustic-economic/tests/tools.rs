use std::{env, sync::Arc};

use anyhow::Result;
use rustic_core::{Tool, set_logger};
use rustic_economic::{
    storage::mongo::{manager::EconomicMongoStorageManager, reader::EconomicMongoStorageReader},
    tools::{bea::BeaDataTool, census::CensusDataTool, fred::FredDataTool},
};
use serde_json::json;
use tokio::fs;
use tracing::info;

pub async fn get_manager() -> Result<EconomicMongoStorageManager> {
    let mongo_uri = env::var("MONGO_URI").expect("MONGO_URI envrionment variable not set");
    info!("Mongo uri: {}", mongo_uri);

    // uri is the same for all
    let mongo_db = env::var("RUSTIC_ECONOMIC_DB_NAME")
        .expect("RUSTIC_ECONOMIC_DB_NAME envrionment variable not set");
    info!(
        "Economic Data Mongo uri: {:?} db: {:?}",
        mongo_uri, mongo_db
    );

    let manager = EconomicMongoStorageManager::new(&mongo_uri, &mongo_db).await;
    manager
}

#[tokio::test]
async fn test_tool_fred_series() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = FredDataTool::new(Arc::new(reader.clone()));
    let value = json!({"series_ids": ["CPIAUCSL", "UMCSENT", "UNRATE", "PCE", "DFFFRC1A027NBEA", "DFDHRC1Q027SBEA"], "limit": 10});

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_fred_series.json", &json).await?;
    Ok(())
}

#[tokio::test]
async fn test_tool_bea_nipa() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = BeaDataTool::new(Arc::new(reader.clone()));
    let value = json!({"dataset": "nipa", "table_name":"T20100", "year": "LAST5"});

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_nipa.json", &json).await?;
    Ok(())
}


#[tokio::test]
async fn test_tool_bea_regional_cainc1() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = BeaDataTool::new(Arc::new(reader.clone()));
    let value = json!({
        "dataset": "regional", "table_name":"CAINC1", "year": "LAST5",
        "geo_fips": "06000"
    }); 

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_regional_cainc1.json", &json).await?;
    Ok(())
}


#[tokio::test]
async fn test_tool_bea_regional_cainc1_county() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = BeaDataTool::new(Arc::new(reader.clone()));
    let value = json!({
        "dataset": "regional", "table_name":"CAINC1", "year": "LAST5",
        "geo_type": "COUNTY", "state_prefix": "06"
    }); 

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_regional_cainc1_county.json", &json).await?;
    Ok(())
}

#[tokio::test]
async fn test_tool_bea_regional_cainc5n() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = BeaDataTool::new(Arc::new(reader.clone()));
    let value = json!({
        "dataset": "regional", "table_name":"CAINC5N", "year": "LAST5",
        "geo_type": "COUNTY", "state_prefix": "06"
    }); 

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_regional_cainc5n.json", &json).await?;
    Ok(())
}


#[tokio::test]
async fn test_tool_census() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = CensusDataTool::new(Arc::new(reader.clone()));
    let value = json!(
        {   
            "dataset": "acs5", "variables": ["B19013_001E", "B25077_001E", "B25003_002E", "B01002_001E"], "year": "LAST5", 
            "geo_type": "state"
        }
    );

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_census.json", &json).await?;
    Ok(())
}
