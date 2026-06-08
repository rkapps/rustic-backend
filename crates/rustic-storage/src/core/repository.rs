use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::core::{index::IndexDefinition, search::SearchCriteria};

/// Blanket bound that every repository key type must satisfy.
///
/// A blanket `impl` covers all types that meet the constraints, so callers
/// never need to explicitly implement this trait — it is purely a convenient
/// alias used in generic bounds throughout the crate.
pub trait RepoKey:
    Eq + Hash + Send + Sync + Clone + Debug + Display + Serialize + DeserializeOwned + 'static
{
}
impl<T> RepoKey for T where
    T: Eq + Hash + Send + Sync + Clone + Debug + Display + Serialize + DeserializeOwned + 'static
{
}

/// A domain model that can be persisted in a [`Repository`].
///
/// Implementors must be able to report their unique key (`id`) and the name
/// of the backing collection (`collection`).  The collection name is used by
/// both the file backend (directory name) and the MongoDB backend (collection
/// name).
pub trait RepoModel<K>:
    Send + Sync + Clone + Serialize + Debug + DeserializeOwned + 'static
{
    /// Returns the model's unique identifier.
    fn id(&self) -> K;

    /// Returns the canonical collection name for this model type.
    fn collection(&self) -> &'static str;
}

/// Async CRUD interface implemented by every storage backend.
///
/// The `K` type parameter is the key type (must satisfy [`RepoKey`]) and `M`
/// is the model type (must satisfy [`RepoModel<K>`]).
///
/// All operations take `&mut self` because both the file and MongoDB backends
/// may need to mutate internal state (e.g. offset maps, connection handles).
#[async_trait]
pub trait Repository<K, M>: Send + Sync {
    /// Run an aggregation pipeline and return the raw results as JSON values.
    ///
    /// Each element of `pipeline` is a pipeline stage document (e.g.
    /// `{"$match": ...}`, `{"$group": ...}`).  The MongoDB backend forwards
    /// the pipeline directly to the driver; the file backend ignores the
    /// pipeline and always returns an empty `Vec`.
    async fn aggregate(&mut self, pipeline: Vec<Value>) -> Result<Vec<Value>>;

    /// Upsert multiple models in a single batch operation.
    async fn bulk_update(&mut self, models: Vec<M>) -> Result<()>;
    /// Create a single index.
    async fn create_index(&mut self, index: IndexDefinition) -> Result<()>;
    /// Create multiple indexes in one operation.
    async fn create_indexes(&mut self, indexes: Vec<IndexDefinition>) -> Result<()>;
    /// Append a tombstone record (file) or issue a delete query (MongoDB).
    async fn delete(&mut self, repo: M) -> Result<()>;
    /// Delete all models matching `criteria`, or every model when `None`.
    async fn delete_many(&mut self, criteria: Option<SearchCriteria>) -> Result<()>;
    /// Return all models that match `criteria`.  `None` returns everything.
    async fn find(&mut self, search: Option<SearchCriteria>) -> Result<Vec<M>>;
    /// Convenience wrapper around `find(None)`.
    async fn find_all(&mut self) -> Result<Vec<M>>;
    /// Return the first model matching `criteria`.
    async fn find_one(&mut self, search: Option<SearchCriteria>) -> Result<M>;
    /// Look up a model by its primary key.
    async fn find_by_id(&mut self, id: K) -> Result<M>;
    /// Persist a new model.
    async fn insert(&mut self, repo: M) -> Result<()>;
    /// Persist multiple models.
    async fn insert_many(&mut self, models: Vec<M>) -> Result<()>;

    /// Return the `top_k` most similar models to `query_vector`.
    ///
    /// An optional `criteria` pre-filters candidates before similarity ranking,
    /// which is useful for scoping vector search to a subset of records (e.g.
    /// only documents belonging to a particular user or workspace).
    async fn semantic_search(
        &mut self,
        query_vector: &[f32],
        top_k: usize,
        criteria: Option<SearchCriteria>,
    ) -> Result<Vec<(M, f32)>>
    where
        M: VectorEmbedding + RepoModel<K>;

    /// Overwrite an existing model.  Both backends use upsert semantics, so
    /// calling `update` on a model that does not yet exist will insert it.
    async fn update(&mut self, repo: M) -> Result<()>;

    
}

/// A model that carries a dense float vector, used for similarity search.
pub trait VectorEmbedding: Send + Sync + Debug {
    /// Returns the embedding vector stored on this model.
    fn vector(&self) -> &[f32];
}

/// A model that supports in-memory filtering and field-based sorting.
///
/// The file backend evaluates queries entirely in-process, so models must
/// implement this trait to participate in `find` / `find_one` calls.
/// The default implementations pass every record and return no sortable
/// field values, which is correct for models that do not need filtering.
pub trait Searchable {
    /// Returns `true` when this record satisfies all conditions in `criteria`.
    fn matches_filter(&self, _criteria: &SearchCriteria) -> bool {
        true
    }

    /// Returns the comparable value for `field`, used during sort.
    fn get_field_value(&self, _field: &str) -> Option<SortValue> {
        None
    }
}

/// A comparable scalar value used for sorting and in-memory comparisons.
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub enum SortValue {
    String(String),
    Decimal(rust_decimal::Decimal),
    Int(i64),
}
