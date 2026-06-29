# rustic-storage

Backend-agnostic persistence layer for the rustic-ai workspace. Provides a `Repository<K, M>` trait with two concrete implementations: an append-only flat-file backend (no database required) and a MongoDB backend.

## Architecture

```text
core/           — Repository trait and query DSL (application code depends only here)
  repository.rs — Repository<K,M>, RepoModel<K>, VectorEmbedding, Searchable, SortValue
  search.rs     — SearchCriteria builder
  index.rs      — IndexDefinition for create_index

file/           — Append-only BSON flat-file backend
  database.rs   — FileDatabase handle (opens / creates .bin files per collection)
  repository.rs — Repository impl with in-memory offset map for O(1) ID lookups
  record.rs     — RecordHeader: 32-byte fixed header (magic, version, CRC32, flags)

mongo/          — MongoDB backend
  database.rs   — MongoDatabase handle
  repository.rs — Repository impl via the official mongodb driver
```

## The `Repository` trait

```rust
#[async_trait]
pub trait Repository<K, M>: Send + Sync {
    async fn insert(&mut self, model: M) -> Result<()>;
    async fn insert_many(&mut self, models: Vec<M>) -> Result<()>;
    async fn bulk_update(&mut self, models: Vec<M>) -> Result<()>;
    async fn update(&mut self, model: M) -> Result<()>;
    async fn delete(&mut self, model: M) -> Result<()>;
    async fn delete_many(&mut self, criteria: Option<SearchCriteria>) -> Result<()>;

    async fn find_by_id(&mut self, id: K) -> Result<M>;
    async fn find_all(&mut self) -> Result<Vec<M>>;
    async fn find_one(&mut self, search: Option<SearchCriteria>) -> Result<M>;
    async fn find(&mut self, search: Option<SearchCriteria>) -> Result<Vec<M>>;

    async fn aggregate(&mut self, pipeline: Vec<Value>) -> Result<Vec<Value>>;
    async fn create_index(&mut self, index: IndexDefinition) -> Result<()>;
    async fn create_indexes(&mut self, indexes: Vec<IndexDefinition>) -> Result<()>;

    async fn semantic_search(
        &mut self,
        query_vector: &[f32],
        top_k: usize,
        criteria: Option<SearchCriteria>,
    ) -> Result<Vec<(M, f32)>>
    where
        M: VectorEmbedding + RepoModel<K>;
}
```

Supporting model traits:

| Trait | Required methods |
|-------|-----------------|
| `RepoModel<K>` | `id() -> K`, `collection() -> &'static str` |
| `VectorEmbedding` | `vector() -> &[f32]` — enables `semantic_search` |
| `Searchable` | `matches_filter`, `get_field_value` — used by the file backend for in-process filtering |

## File backend

Each collection is stored as a single `.bin` file. Every write appends a fixed 32-byte `RecordHeader` (magic number, version, record type, length, CRC32, flags) followed by a BSON-encoded payload. Updates and deletes also append — updates create a new version of the record; deletes append a tombstone.

An in-memory offset map is built on `FileDatabase::open` by scanning the file once. Subsequent `find_by_id` calls are O(1); filtered queries scan the offset map in-process using the `Searchable` trait.

Intended for development, edge deployments, and tests where no database server is available.

## MongoDB backend

`MongoDatabase` wraps the official `mongodb` driver. `Repository` operations map directly to driver calls. `aggregate` forwards the pipeline verbatim; the file backend always returns `[]` for aggregations.

## Re-exports

```rust
use rustic_storage::{Repository, RepoModel, RepoKey, Searchable, VectorEmbedding};
use rustic_storage::SearchCriteria;
use rustic_storage::{FileDatabase, MongoDatabase};
```
