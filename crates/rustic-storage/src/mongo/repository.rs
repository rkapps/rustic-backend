use anyhow::{Context, Result};
use async_trait::async_trait;
use bson::Document;
use bson::serialize_to_bson;
use bson::serialize_to_document;
use futures::StreamExt;
use futures::TryStreamExt;
use mongodb::IndexModel;
use mongodb::options::IndexOptions;
use mongodb::{
    bson::doc,
    options::{ReplaceOneModel, WriteModel},
};
use rustic_ml::search::similarity::search;
use serde_json::Value;

use std::marker::PhantomData;
use tracing::{error, trace};

use crate::core::index::IndexDefinition;
use crate::core::repository::RepoKey;
use crate::core::repository::RepoModel;
use crate::core::repository::Repository;
use crate::core::repository::VectorEmbedding;
use crate::core::search::SearchCriteria;
use crate::mongo::MongoCriteriaBuilder;

/// MongoDB implementation of [`Repository`].
///
/// Wraps a typed `mongodb::Collection<M>` and translates [`SearchCriteria`]
/// queries to BSON filter/sort documents via [`MongoCriteriaBuilder`].
///
/// All documents are queried and stored using the field name `"id"` as the
/// primary key — models must serialise their key under that field name.
///
/// Semantic search is performed in-process by fetching all candidate records
/// (optionally pre-filtered) and computing cosine similarity via
/// `rustic_ml::search::similarity::search`.
#[derive(Debug)]
pub struct MongoRepository<K, M>
where
    K: RepoKey,
    M: RepoModel<K>,
{
    collection: mongodb::Collection<M>,
    _phantom: PhantomData<(K, M)>,
}

impl<K, M> MongoRepository<K, M>
where
    K: RepoKey,
    M: RepoModel<K>,
{
    pub fn new(collection: mongodb::Collection<M>) -> Result<Self> {
        Ok(MongoRepository {
            collection,
            _phantom: PhantomData,
        })
    }
}

#[async_trait]
impl<K, M> Repository<K, M> for MongoRepository<K, M>
where
    K: RepoKey,
    M: RepoModel<K>,
{
    /// Execute a MongoDB aggregation pipeline and return results as JSON values.
    ///
    /// Each `Value` in `pipeline` is converted to a BSON `Document` before
    /// being forwarded to the driver.  Results are deserialized back to
    /// `serde_json::Value` so callers are not bound to the model type `M`.
    async fn aggregate(&mut self, pipeline: Vec<Value>) -> Result<Vec<Value>> {
        let bson_pipeline: Vec<Document> = pipeline
            .iter()
            .map(|v| {
                bson::serialize_to_document(v)
                    .map_err(|e| anyhow::anyhow!("BSON conversion error: {}", e))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut cursor = self.collection.aggregate(bson_pipeline).await?;
        let mut results = Vec::new();

        while let Some(doc) = cursor.try_next().await? {
            let value: serde_json::Value = bson::deserialize_from_document(doc)
                .map_err(|e| anyhow::anyhow!("JSON conversion error: {}", e))?;
            results.push(value);
        }

        Ok(results)
    }

    async fn bulk_update(&mut self, models: Vec<M>) -> Result<()> {
        let ns = self.collection.namespace();

        let operations: Vec<WriteModel> = models
            .into_iter()
            .map(|model| {
                let id = serialize_to_bson(&model.id()).ok();
                let filter = doc! { "id": id };
                let replacement = serialize_to_document(&model).expect("Failed to serialize model");

                WriteModel::ReplaceOne(
                    ReplaceOneModel::builder()
                        .namespace(ns.clone())
                        .filter(filter)
                        .replacement(replacement)
                        .upsert(true)
                        .build(),
                )
            })
            .collect();

        self.collection.client().bulk_write(operations).await?;
        Ok(())
    }

    async fn create_index(&mut self, index: IndexDefinition) -> Result<()> {
        self.create_indexes(vec![index]).await
    }

    async fn create_indexes(&mut self, indexes: Vec<IndexDefinition>) -> Result<()> {
        let index_models: Vec<IndexModel> = indexes
            .into_iter()
            .map(|idx| {
                let mut keys = Document::new();
                for (field, direction) in idx.fields {
                    keys.insert(field, direction);
                }

                let mut options = IndexOptions::builder().build();
                options.unique = Some(idx.unique);
                options.sparse = Some(idx.sparse);
                if let Some(name) = idx.name {
                    options.name = Some(name);
                }

                IndexModel::builder().keys(keys).options(options).build()
            })
            .collect();

        self.collection.create_indexes(index_models).await?;
        Ok(())
    }

    async fn insert(&mut self, model: M) -> Result<()> {
        trace!("Model: {:#?}", model);
        self.collection.insert_one(model).await?;
        Ok(())
    }

    async fn insert_many(&mut self, models: Vec<M>) -> Result<()> {
        if models.is_empty() {
            return Ok(());
        }
        self.collection.insert_many(models).ordered(false).await?;
        Ok(())
    }

    async fn delete(&mut self, model: M) -> Result<()> {
        let id = serialize_to_bson(&model.id()).ok();
        let filter = doc! { "id": id};
        self.collection.delete_one(filter).await?;
        Ok(())
    }

    async fn delete_many(&mut self, criteria: Option<SearchCriteria>) -> Result<()> {
        let criteria = &criteria.unwrap_or_default();
        let filter = MongoCriteriaBuilder::build_filter(criteria);
        self.collection.delete_many(filter).await?;
        Ok(())
    }

    async fn find_by_id(&mut self, id: K) -> Result<M> {
        let id = serialize_to_bson(&id)?;
        let filter = doc! { "id":  id};
        trace!("Repo: {} Filter: {:?}", self.collection.name(), filter);
        let Some(result) = self
            .collection
            .find_one(filter)
            .await
            .context("Failed to execute find query")?
        else {
            return Err(anyhow::anyhow!("Could not find document"));
        };

        Ok(result)
    }

    // find one value that matches
    async fn find_one(&mut self, search: Option<SearchCriteria>) -> Result<M> {
        let items = self.find(search).await?;
        Ok(items[0].clone())
    }

    // find_finds filtered values
    async fn find(&mut self, criteria: Option<SearchCriteria>) -> Result<Vec<M>> {
        let mut results = Vec::new();

        let criteria = &criteria.unwrap_or_default();
        let filter = MongoCriteriaBuilder::build_filter(criteria);
        let sort = MongoCriteriaBuilder::build_sort(criteria);
        let limit = criteria.limit.unwrap_or(0).try_into().unwrap_or(0);

        trace!(
            "Repo: {} Filter: {:?} Sort: {:?} Limit: {}",
            self.collection.name(),
            filter,
            sort,
            limit
        );
        let mut cursor = match self.collection.find(filter).sort(sort).limit(limit).await {
            Ok(c) => c,
            Err(e) => {
                return Err(anyhow::anyhow!("Collection - find error : {}", e));
            }
        };

        let mut fetched = 0;
        while let Some(result) = cursor.next().await {
            match result {
                Ok(item) => {
                    fetched += 1;
                    results.push(item);
                }
                Err(e) => {
                    error!("Deserialization error on item {}: {}", fetched + 1, e);
                    return Err(anyhow::anyhow!("Deserialization failed: {}", e));
                }
            }
        }
        trace!("Results: {:?}", results.len());

        Ok(results)
    }

    async fn find_all(&mut self) -> Result<Vec<M>> {
        Ok(self.find(None).await?)
    }

    async fn semantic_search(
        &mut self,
        query_vector: &[f32],
        top_k: usize,
        criteria: Option<SearchCriteria>,
    ) -> Result<Vec<(M, f32)>>
    where
        M: VectorEmbedding + RepoModel<K>,
    {
        let items = self.find(criteria).await?;
        let candidates: Vec<(K, Vec<f32>)> = items
            .iter()
            .map(|entry| (entry.id().clone(), entry.vector().to_vec()))
            .collect();

        let results = search(query_vector, &candidates, top_k);

        // iterator through result and return vector of (M, f32)
        let final_results: Vec<(M, f32)> = results
            .iter()
            .filter_map(|(id, score)| {
                items
                    .iter()
                    .find(|item| item.id() == *id)
                    .cloned()
                    .map(|item| (item, *score))
            })
            .collect();
        Ok(final_results)
    }

    async fn update(&mut self, model: M) -> Result<()> {
        trace!("Model before serialize");
        let id = serialize_to_bson(&model.id()).ok();
        let filter = doc! { "id": id};
        trace!("Model: {:#?}", model);

        self.collection
            .replace_one(filter, model)
            .upsert(true)
            .await?;
        Ok(())
    }
}
