use std::{env, sync::Arc};

use anyhow::Result;
use rustic_core::{Tool, set_logger};
use rustic_economic::{
    storage::mongo::{manager::EconomicMongoStorageManager, reader::EconomicMongoStorageReader}, tools::{bea_nipa::BeaNipaDataTool, bea_regional::BeaRegionalDataTool, census::CensusDataTool, fred::FredDataTool},
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

    let tool = BeaNipaDataTool::new(Arc::new(reader.clone()));
    let value = json!({"table_name": "T20100", "series_codes": ["DPCERC", "A065RC"], "year": "2023"});

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_nipa.json", &json).await?;
    Ok(())
}


#[tokio::test]
async fn test_tool_bea_regional_cainc1_national() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = BeaRegionalDataTool::new(Arc::new(reader.clone()));
    let value = json!({
        "code":"CAINC1", "line_codes": ["1"], "year": "LAST5", "geo_fips": ["00000"]
    }); 

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_regional_cainc1_national.json", &json).await?;
    Ok(())
}

#[tokio::test]
async fn test_tool_bea_regional_cainc1_state() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = BeaRegionalDataTool::new(Arc::new(reader.clone()));
    let value = json!({
        "code":"CAINC1", "line_codes": ["1"], "year": "LAST5", "geo_fips": ["06000", "04000" ]
    }); 

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_regional_cainc1_state.json", &json).await?;
    Ok(())
}


#[tokio::test]
async fn test_tool_bea_regional_cainc1_county() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = BeaRegionalDataTool::new(Arc::new(reader.clone()));
    let value = json!({
        "code":"CAINC1", "line_codes": ["1"],  "year": "LAST5", "geo_type": "COUNTY", "state_prefix": "06"
    }); 

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_regional_cainc1_county.json", &json).await?;
    Ok(())
}

#[tokio::test]
async fn test_tool_bea_regional_cainc5n_state() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = BeaRegionalDataTool::new(Arc::new(reader.clone()));
    let value = json!({
        "code":"CAINC5N", "line_codes": ["701", "704", "521"], "year": "LAST5",  "geo_fips": ["06000", "04000" ]
    }); 

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_regional_cainc5n_state.json", &json).await?;
    Ok(())
}

#[tokio::test]
async fn test_tool_bea_regional_cainc5n_county() -> Result<()> {
    set_logger("economic-tool=debug".to_string());
    let manager = get_manager().await?;
    let reader = EconomicMongoStorageReader::new(manager);

    let tool = BeaRegionalDataTool::new(Arc::new(reader.clone()));
    let value = json!({
        "code":"CAINC5N", "line_codes": ["700", "704", "521"],  "year": "LAST5",
        "geo_type": "COUNTY", "state_prefix": "06"
    }); 

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_bea_regional_cainc5n_county.json", &json).await?;
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
            "geo_fips": ["06045", "04000" ]
        }
    );

    let result = tool.execute(value).await?;
    // save to fixture
    let json = serde_json::to_string_pretty(&result).unwrap();
    fs::write("tests/fixtures/tool_census.json", &json).await?;
    Ok(())
}
