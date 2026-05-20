# rustic-storage

rustic-storage provides a lightweight repository pattern. Binary file and Mongodb imlemenations.

## Features

- **Simple CRUD operations** - Insert, find, update, delete
- **Collection-based** - Organize data into named collections
- **Vector search capabilities** - Semantic similarity
- **Dynamic Search** - `SearchCriteria` struct and `Searchable` trait
- **Async-ready** - Trait supports both sync and async implementations

## Vector Search

- **Cosine similarity**: Measures semantic similarity between embeddings
- **Vector search**: Finds top-k most similar vectors from a collection
- **Generic implementation**: Works with any model implementing `VectorEmbedding`

## The Repository Trait

```rust
#[async_trait]
pub trait Repository<K, M>: Send + Sync {
    async fn insert(&mut self, repo: M) -> Result<()>;
    async fn insert_many(&mut self, models: Vec<M>) -> Result<()>;
    async fn bulk_update(&mut self, models: Vec<M>) -> Result<()>;
    async fn update(&mut self, repo: M) -> Result<()>;
    async fn delete(&mut self, repo: M) -> Result<()>;
    async fn delete_many(&mut self, criteria: Option<SearchCriteria>) -> Result<()>;

    async fn find_by_id(&mut self, id: K) -> Result<M>;
    async fn find_all(&mut self) -> Result<Vec<M>>;
    async fn find_one(&mut self, search: Option<SearchCriteria>) -> Result<M>;
    async fn find(&mut self, search: Option<SearchCriteria>) -> Result<Vec<M>>;

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

**`RepoModel<K>`**: Base model trait with `id()`
**`VectorEmbedding`**: Models with vector embeddings
**`Searchable`**: Models that support dynamic search with sort and limit


## Binary File features

- **Append-only log** - Write-optimized with crash safety
- **Fast lookups** - In-memory offset map for O(1) retrieval by ID
- **Forward-compatible format** - Versioned binary headers for future extensions

## File Format

Each collection is stored as a single `.bin` file with the following structure:

```
[RecordHeader: 32 bytes]  ← Version, type, length, timestamp, CRC32, flags
[BSON payload]            ← Serialized document

[RecordHeader: 32 bytes]
[BSON payload]
...
```

**Header fields:**

- Magic number (file format validation)
- Version (schema evolution)
- Record type (Active/Deleted)
- Length (total record size)
- Timestamp (write time)
- CRC32 (corruption detection)
- Flags (compression, encryption, etc.)

## Design Decisions

**Append-only log:**

- Writes always go to end of file
- Updates create new version (old data remains)
- Deletes write tombstone records
- Simple, crash-safe, no corruption risk

**In-memory offset map:**

- Built on startup by scanning file
- Maps ID → file offset
- O(1) lookups by ID
- Trade-off: startup time vs runtime speed

**BSON encoding:**

- Self-describing format
- Handles complex nested data
- Compatible with MongoDB
- Slightly larger than custom binary

**Binary headers:**

- Forward-compatible (version + flags)
- Corruption detection (CRC32)
- Metadata without parsing payload
- Fixed 32-byte size

## Limitations

- No built-in transactions
- No connection pooling
- File-based implementation is best for small-medium datasets
- **No Transactions** - Operations are not atomic across multiple calls

**Best For:**

- ✅ Applications needing flexible storage abstraction
- ✅ Prototyping and development
- ✅ Small to medium datasets
- ✅ Learning Rust async patterns

**Not Suitable For:**

- ❌ Complex queries or joins
- ❌ Very large datasets
- ❌ ACID transaction requirements

### Basic Example

examples/repo.rs - Simple single-threaded applications
cargo run --example repo

examples/database.rs - Multiple repositories, but accessed one at a time
cargo run --example database

examples/concurrent.rs - Web servers, concurrent applications, multiple threads/tasks
cargo run --example concurrent

## Future Work

- [ ] Compaction (remove old versions and tombstones)
- [ ] Persistent offset map (faster startup)
- [ ] Additional backends (PostgreSQL)
- [ ] Transactions
- [ ] Compression

## Contributing

Contributions welcome! Please open an issue or PR.

## License

MIT OR Apache-2.0
