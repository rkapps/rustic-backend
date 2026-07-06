use anyhow::Result;
use async_trait::async_trait;
use rustic_storage::{Repository, SearchCriteria};
use serde_json::json;
use tracing::{debug, error, trace};

use crate::{
    domain::CensusData,
    storage::{
        mongo::{reader::EconomicMongoStorageReader, writer::EconomicMongoStorageWriter},
        reader::CensusStorageReader,
        writer::CensusStorageWriter,
    },
    tools::domain::CensusEntity,
};

#[async_trait]
impl CensusStorageReader for EconomicMongoStorageReader {
    async fn get_census(&self, id: &str) -> Result<CensusData> {
        match self.manager.census().await {
            Ok(repo) => {
                let mut repo = repo.lock().await;
                repo.find_by_id(id.to_string()).await
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error getting CensusData: {}", e));
            }
        }
    }

    async fn get_census_by_dataset_variable(
        &self,
        dataset: &str,
        variables: Vec<String>,
        geo_fips: Option<&str>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        years: Vec<String>,
    ) -> Result<Vec<CensusEntity>> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;

        let mut match_conditions = vec![
            json! ({ "dataset": dataset }),
            json! ({ "variable": { "$in": variables } }),
            json! ({ "year": { "$in": years } }),
        ];


        // Append optional fields only if they contain data
        if let Some(geo_type) = geo_type {
            match_conditions.push(json!({ "geo_type": geo_type })); // Maps your raw field to the struct
        }
        if let Some(geo_fips) = geo_fips {
            match_conditions.push(json! ({ "geo_fips": geo_fips }));
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

        // 3. Construct the pipeline array
        let pipeline = vec![
            // Stage 1: Dynamic Filters
            json! ({ "$match": { "$and": match_conditions } }),
            // Stage 2: Group into CensusValueEntity array per Geography
            json! ({
                "$group": {
                    "_id": {
                        "dataset": "$dataset",
                        "variable": "$variable",
                        "geo_type": "$geo_type",
                        "geo_name": "$geo_name",
                        "geo_fips": "$geo_fips"
                    },
                    "data": {
                        "$push": {
                            "year": "$year",
                            "value": "$value"
                        }
                    }
                }
            }),
            // Stage 3: Group into CensusGeoEntity array per Dataset/Variable
            json!({
                "$group": {
                    "_id": {
                        "dataset": "$_id.dataset",
                        "variable": "$_id.variable"
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
                    "dataset": "$_id.dataset",
                    "variable": "$_id.variable",
                    "geos": 1
                }
            }),
        ];

        let results = repo.aggregate(pipeline).await?;
        debug!(
            target: "economic-tool",
            "results: {:#?}", results.len()
        );
        let mut entities: Vec<CensusEntity> = Vec::new();

        for (index, val) in results.iter().enumerate() {
            match serde_json::from_value::<CensusEntity>(val.clone()) {
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
            "census: {:#?}", entities
        );
        Ok(entities)
    }

    async fn get_census_filtered(
        &self,
        dataset: &str,
        variable: &str,
        geo_fips: Option<&str>,
        geo_type: Option<&str>,
        state_prefix: Option<&str>,
        year: &str,
    ) -> Result<Vec<CensusData>> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;
        let mut criteria = SearchCriteria::new()
            .eq("dataset", dataset)
            .eq("year", year)
            .eq("variable", variable);

        if let Some(fips) = geo_fips {
            criteria = criteria.eq("geo_fips", fips);
        }
        if let Some(gt) = geo_type {
            criteria = criteria.eq("geo_type", gt);
        }
        if let Some(prefix) = state_prefix {
            criteria = criteria.starts_with("geo_fips", prefix);
        }
        debug!("get_census_filtered SearchCriteria: {:#?}", criteria);

        repo.find(Some(criteria)).await
    }
}

#[async_trait]
impl CensusStorageWriter for EconomicMongoStorageWriter {
    async fn delete_all_census(&self) -> Result<()> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting EconomicSeries Repository"));
        };
        let mut repo = repo.lock().await;
        repo.delete_many(Some(SearchCriteria::new())).await?;
        Ok(())
    }
    async fn upsert_census_bulk(&self, datas: Vec<CensusData>) -> Result<()> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;
        repo.bulk_update(datas).await
    }

    async fn upsert_census(&self, data: CensusData) -> Result<()> {
        let Ok(repo) = self.manager.census().await else {
            return Err(anyhow::anyhow!("Error getting Census Repository"));
        };
        let mut repo = repo.lock().await;
        repo.update(data).await
    }
}
