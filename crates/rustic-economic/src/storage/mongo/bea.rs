use crate::{
    domain::bea::{BeaNipa, BeaRegional},
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::BeaStorageReader,
        writer::BeaStorageWriter,
    },
    tools::domain::{BeaNipaEntity, BeaRegionalEntity},
};
use anyhow::Result;
use async_trait::async_trait;
use rustic_storage::{Repository, SearchCriteria};
use serde_json::json;
use tracing::{debug, error, trace};

#[async_trait]
impl BeaStorageReader for EconomicMongoStorageReader {
    async fn get_bea_nipa(&self, id: &str) -> Result<BeaNipa> {
        match self.manager.bea_nipa().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find_by_id(id.to_string()).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting BeaNipaData: {}", e));
            }
        }
    }

    async fn get_bea_regional(&self, id: &str) -> Result<BeaRegional> {
        match self.manager.bea_regional().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find_by_id(id.to_string()).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting BeaRegionalData: {}", e));
            }
        }
    }

    async fn get_bea_nipa_by_table_series(
        &self,
        table_name: String,
        series_codes: Vec<String>,
        years: Vec<String>,
    ) -> Result<Vec<BeaNipaEntity>> {
        let Ok(repo) = self.manager.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;
        let pipeline = vec![
            // 1. Filter data immediately at the start of the query
            json!( {
                "$match": {
                    "table_name": table_name,
                    "series_code": { "$in": series_codes},
                    "time_period": { "$in": years }
                }
            }),
            // 2. First Grouping: Build the series value data vectors
            json!( {
                "$group": {
                    "_id": {
                        "table_name": "$table_name",
                        "code": "$series_code",
                        "description": "$line_description"
                    },
                    "data": {
                        "$push": { "year": "$time_period",
                        "value": {
                                "$convert": {
                                    // 1. Remove all commas from the string first
                                    "input": {
                                        "$replaceAll": {
                                            "input": "$data_value",
                                            "find": ",",
                                            "replacement": ""
                                        }
                                    },
                                    "to": "double",
                                    // 2. Safe error fallbacks to keep the query from crashing
                                    "onError": 0.0,
                                    "onNull": 0.0
                                }
                            }
                        }
                    }
                }
            }),
            // 3. Second Grouping: Roll individual series arrays up into the table object
            json!( {
                "$group": {
                    "_id": "$_id.table_name",
                    "series": {
                        "$push": {
                            "code": "$_id.code",
                            "description": "$_id.description",
                            "data": "$data"
                        }
                    }
                }
            }),
            // 4. Project: Format keys to map 1:1 with your Rust structs
            json!( {
                "$project": {
                    "_id": 0,
                    "dataset": { "$literal": "NIPA" },
                    "table_name": "$_id",
                    "series": 1
                }
            }),
        ];
        let results = repo.aggregate(pipeline).await?;
        debug!(
            target: "economic-tool",
            "results: {:#?}", results
        );
        let mut entities: Vec<BeaNipaEntity> = Vec::new();

        for (index, val) in results.iter().enumerate() {
            match serde_json::from_value::<BeaNipaEntity>(val.clone()) {
                Ok(entity) => entities.push(entity),
                Err(err) => {
                    // This will print the exact field, key, or type that Serde is choking on!
                    error!(
                        target: "economic-tool",
                        "Serde failed at item index {}: {}", index, err
                    );
                }
            }
        }
        debug!(
            target: "economic-tool",
            "bea_nipa: {:#?}", entities
        );
        Ok(entities)
    }

    async fn get_bea_regional_by_table_series(
        &self,
        codes: Vec<String>,
        years: Vec<String>,
        geo_fips: Vec<String>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
    ) -> Result<Vec<BeaRegionalEntity>> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;

        let mut match_conditions = vec![
            json! ({ "code": { "$in": codes} }),
            json! ({ "time_period": { "$in": years } }),
        ];

        // Append optional fields only if they contain data
        if let Some(geo_type) = geo_type {
            match_conditions.push(json!({ "geo_type": geo_type })); // Maps your raw field to the struct
        }
        if !geo_fips.is_empty() {
            match_conditions.push(json! ({ "geo_fips": { "$in": geo_fips }}));
        }

        if let Some(prefix) = state_prefix {
            match_conditions.push(json!( {
                "geo_fips": {
                    // The ^ symbol anchors the regex match strictly to the start of the string
                    "$regex": format!("^{}", prefix),
                    "$options": "i" // Optional: case-insensitive if your prefixes ever contain characters
                }
            }));
        }
        debug!(
            target: "economic-tool",
            "match_conditions: {:#?}", match_conditions
        );

        let pipeline = vec![
            // 1. Filter data immediately at the start of the query
            json! ({ "$match": { "$and": match_conditions } }),
            // 2. First Grouping: Build the series value data vectors
            json!( {
                "$group": {
                    "_id": {
                        "code": "$code",
                        "geo_type": "$geo_type",
                        "geo_name": "$geo_name",
                        "geo_fips": "$geo_fips"
                    },
                    "data": {
                        "$push": { "year": "$time_period",
                        "value": {
                                "$convert": {
                                    // 1. Remove all commas from the string first
                                    "input": {
                                        "$replaceAll": {
                                            "input": "$data_value",
                                            "find": ",",
                                            "replacement": ""
                                        }
                                    },
                                    "to": "double",
                                    // 2. Safe error fallbacks to keep the query from crashing
                                    "onError": 0.0,
                                    "onNull": 0.0
                                }
                            }
                        }
                    }
                }
            }),
            // Stage 3: Group into CensusGeoEntity array per Dataset/Variable
            // Stage 3: Group into CensusGeoEntity array per Dataset/Variable
            json!({
                "$group": {
                    "_id": {
                        "code": "$_id.code", // 👈 FIX 1: Read from Stage 2's _id wrapper!
                    },
                    "geos": {
                        "$push": {
                            "geo_type": "$_id.geo_type",
                            "geo_name": "$_id.geo_name",
                            "geo_fips": "$_id.geo_fips",
                            "data": "$data"
                        }
                    }
                }
            }),
            // Stage 4: Project fields 1:1 with your CensusEntity struct layout
            json!({
                "$project": {
                    "_id": 0,
                    "code": "$_id.code", // 👈 FIX 2: Read from Stage 3's _id wrapper!
                    "geos": 1
                }
            }),
        ];
        let results = repo.aggregate(pipeline).await?;
        trace!(
            target: "economic-tool",
            "results: {:#?}", results
        );
        let mut entities: Vec<BeaRegionalEntity> = Vec::new();

        for (index, val) in results.iter().enumerate() {
            match serde_json::from_value::<BeaRegionalEntity>(val.clone()) {
                Ok(entity) => entities.push(entity),
                Err(err) => {
                    // This will print the exact field, key, or type that Serde is choking on!
                    error!(
                        target: "economic-tool",
                        "Serde failed at item index {}: {}", index, err
                    );
                }
            }
        }
        trace!(
            target: "economic-tool",
            "bea_regional: {:#?}", entities
        );
        Ok(entities)
    }

    async fn get_bea_regional_by_table(
        &self,
        table_name: &str,
        year: &str,
    ) -> Result<Vec<BeaRegional>> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaRegional Repository"));
        };
        let mut repo = repo.lock().await;

        let criteria = SearchCriteria::new()
            .eq("code", table_name)
            .eq("time_period", year);

        repo.find(Some(criteria)).await
    }

    async fn get_bea_regional_filtered(
        &self,
        table_name: &str,
        geo_fips: Option<&str>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        year: &str,
    ) -> Result<Vec<BeaRegional>> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaRegional Repository"));
        };
        let mut repo = repo.lock().await;

        // use contains till the table name feld is added. code has tablename + linecode
        let mut criteria = SearchCriteria::new()
            .contains("code", table_name)
            .eq("time_period", year);

        if let Some(fips) = geo_fips {
            criteria = criteria.eq("geo_fips", fips);
        }
        if let Some(gt) = geo_type {
            criteria = criteria.eq("geo_type", gt);
        }
        if let Some(prefix) = state_prefix {
            criteria = criteria.starts_with("geo_fips", prefix);
        }
        debug!("get_bea_regional_filtered SearchCriteria: {:#?}", criteria);
        repo.find(Some(criteria)).await
    }
}

#[async_trait]
impl BeaStorageWriter for EconomicMongoStorageWriter {
    async fn delete_all_bea_nipa(&self) -> Result<()> {
        let Ok(repo) = self.manager.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.delete_many(Some(SearchCriteria::new())).await?;
        Ok(())
    }

    async fn upsert_bea_nipa(&self, data: BeaNipa) -> Result<()> {
        let Ok(repo) = self.manager.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(data).await
    }

    async fn upsert_bea_nipa_bulk(&self, datas: Vec<BeaNipa>) -> Result<()> {
        let Ok(repo) = self.manager.bea_nipa().await else {
            return Err(anyhow::anyhow!("Error getting BeaNipa Repository"));
        };
        let mut repo = repo.lock().await;
        repo.bulk_update(datas).await
    }

    // BEA Regional
    async fn delete_all_bea_regional(&self) -> Result<()> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.delete_many(Some(SearchCriteria::new())).await?;
        Ok(())
    }

    async fn upsert_bea_regional_bulk(&self, datas: Vec<BeaRegional>) -> Result<()> {
        let Ok(repo) = self.manager.bea_regional().await else {
            return Err(anyhow::anyhow!("Error getting BeaRegional Repository"));
        };
        let mut repo = repo.lock().await;
        repo.bulk_update(datas).await
    }
}
