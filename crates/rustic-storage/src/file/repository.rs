use anyhow::Result;
use async_trait::async_trait;
use rustic_ml::search::similarity::search;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::marker::PhantomData;
use std::path::Path;
use std::{fmt::Debug, path::PathBuf};
use tracing::{debug, info};

use crate::core::index::IndexDefinition;
use crate::core::repository::{RepoKey, RepoModel, Repository, Searchable, VectorEmbedding};
use crate::core::search::SearchCriteria;
use crate::file::record::{
    RECORD_TYPE_ACTIVE, RECORD_TYPE_DELETED, read_record, write_active_record,
};
use crate::file::sort::apply_sort;

/// Append-only flat-file implementation of [`Repository`].
///
/// Records are written sequentially to a single `.bin` file (one per
/// collection).  An in-memory `HashMap<K, u64>` (`offsetm`) maps each live
/// key to its byte offset, enabling O(1) point-lookups without a full scan.
///
/// Deletions append a [`RECORD_TYPE_DELETED`](super::file::RECORD_TYPE_DELETED)
/// tombstone and remove the key from `offsetm`; the original bytes remain on
/// disk until the file is compacted (not yet implemented).
///
/// The repository must be [`initialize`](FileRepository::initialize)d after
/// construction — this replays the log to rebuild `offsetm`.  When obtained
/// through [`FileDatabase`](super::database::FileDatabase) initialization
/// happens automatically.
#[derive(Debug)]
pub struct FileRepository<K, M>
where
    K: RepoKey,
    M: RepoModel<K>,
{
    pub name: String,
    file: File,
    /// Maps every live record key to its byte offset in `file`.
    offsetm: HashMap<K, u64>,
    _phantom: PhantomData<(K, M)>,
}

impl<K, M> FileRepository<K, M>
where
    K: RepoKey,
    M: RepoModel<K>,
{
    /// Open (or create) the collection log file at
    /// `<collection_path>/<name>.bin`.
    pub fn new(name: String, collection_path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .create(true) // Create the file if it doesn't exist
            .append(true) // Open in append mode
            .open(FileRepository::<K, M>::file_path(&name, &collection_path))?;

        Ok(Self {
            name: name.clone(),
            file,
            offsetm: HashMap::new(),
            _phantom: PhantomData,
        })
    }

    fn file_path(name: &str, collection_path: &Path) -> PathBuf {
        collection_path.join(format!("{}.bin", &name))
    }

    /// Replay the collection log to rebuild the in-memory offset map.
    ///
    /// Reads records from the beginning of the file: active records are added
    /// to `offsetm`; deleted records remove the corresponding entry.  Any
    /// read error (including a truncated final record) is treated as
    /// end-of-log and stops the replay.
    pub async fn initialize(&mut self) -> Result<()> {
        let mut offset = self.file.seek(SeekFrom::Start(0))?;
        info!("Initializing repo: {}...", self.name);
        loop {
            let (header, model) = match read_record::<M>(&mut self.file, offset) {
                Ok((header, model)) => (header, model),
                Err(_e) => {
                    // warn!("Read error: {}", e);
                    break;
                }
            };

            // debug!("Record Type: {:?}", header.record_type);
            match header.record_type {
                RECORD_TYPE_ACTIVE => {
                    self.offsetm.insert(model.id(), offset);
                }
                RECORD_TYPE_DELETED => {
                    self.offsetm.remove(&model.id());
                }
                _ => {
                    break;
                }
            }
            offset = self.file.stream_position()?;
        }
        info!("Initializing done.");
        Ok(())
    }
}

#[async_trait]
impl<K, M> Repository<K, M> for FileRepository<K, M>
where
    K: RepoKey,
    M: RepoModel<K> + Searchable,
{
    /// Not supported by the file backend — always returns an empty `Vec`.
    async fn aggregate(&mut self, _pipeline: Vec<Value>) -> Result<Vec<Value>> {
        Ok(Vec::new())
    }
    async fn bulk_update(&mut self, _models: Vec<M>) -> Result<()> {
        Ok(())
    }

    async fn create_index(&mut self, _index: IndexDefinition) -> Result<()> {
        Ok(())
    }
    async fn create_indexes(&mut self, _indexes: Vec<IndexDefinition>) -> Result<()> {
        Ok(()) // no-op for file backend
    }

    // insert creates a new json file for the chat id. creates the directory structure too.
    async fn insert(&mut self, model: M) -> Result<()> {
        let offset = write_active_record(&mut self.file, RECORD_TYPE_ACTIVE, &model, false)?;
        self.offsetm.insert(model.id(), offset);
        debug!("Insert id:{} at offset:{}", model.id(), offset);
        Ok(())
    }

    async fn insert_many(&mut self, _models: Vec<M>) -> Result<()> {
        Ok(())
    }

    // delete appends the delete record
    async fn delete(&mut self, model: M) -> Result<()> {
        let _ = write_active_record(&mut self.file, RECORD_TYPE_DELETED, &model, false)?;
        self.offsetm.remove(&model.id());
        Ok(())
    }

    async fn delete_many(&mut self, _search: Option<SearchCriteria>) -> Result<()> {
        Ok(())
    }

    // find_by_id finds the json file for the id, marshalls that into the object
    async fn find_by_id(&mut self, id: K) -> Result<M> {
        let Some(offset) = self.offsetm.get(&id) else {
            return Err(anyhow::anyhow!("Error with offset"));
        };
        debug!("Find_by_id Id:{} offset:{}", id, offset);
        let (_, model) = read_record::<M>(&mut self.file, *offset)?;
        Ok(model)
    }

    // find_all returns all values from offset map
    async fn find_all(&mut self) -> Result<Vec<M>> {
        let mut values = Vec::<M>::new();
        debug!("Find_all Offset map length: {}", self.offsetm.len());
        for offset in self.offsetm.values() {
            if let Ok((_, model)) = read_record::<M>(&mut self.file, *offset) {
                values.push(model);
            };
        }
        Ok(values)
    }

    // find one value that matches
    async fn find_one(&mut self, search: Option<SearchCriteria>) -> Result<M> {
        let items = self.find(search).await?;
        Ok(items[0].clone())
    }

    // find_finds filtered values
    async fn find(&mut self, criteria: Option<SearchCriteria>) -> Result<Vec<M>>
    where
        M: Searchable,
    {
        let mut items = self.find_all().await?;
        debug!("Criteria: {:?} items: {:?}", criteria, items.len());

        if let Some(f) = criteria {
            // Apply conditions
            items.retain(|item| item.matches_filter(&f));
            debug!("Filter items: {:?}", items.len());

            // Apply sort
            if let Some(sort_fields) = f.sort_fields {
                debug!("sort_fields: {:?}", sort_fields);
                items = apply_sort(items, &sort_fields);
            }

            // Apply limit
            if let Some(limit) = f.limit {
                items.truncate(limit);
            }
        }
        Ok(items)
    }

    async fn semantic_search(
        &mut self,
        query_vector: &[f32],
        top_k: usize,
        criteria: Option<SearchCriteria>,
    ) -> Result<Vec<(M, f32)>>
    where
        M: VectorEmbedding + Searchable + RepoModel<K>,
    {
        let items = self.find(criteria).await?;

        // create vector with tuple
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

    // update appends the udpated record
    async fn update(&mut self, model: M) -> Result<()> {
        let offset = write_active_record(&mut self.file, RECORD_TYPE_ACTIVE, &model, false)?;
        self.offsetm.insert(model.id(), offset);
        // debug!("Update id:{} at offset:{}", model.id(), offset);
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use once_cell::sync::Lazy;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct TestUser {
        id: String,
        name: String,
    }

    impl RepoModel<String> for TestUser {
        fn id(&self) -> String {
            self.id.clone()
        }
        fn collection(&self) -> &'static str {
            "user"
        }
    }

    impl Searchable for TestUser {}

    static USER1: Lazy<TestUser> = Lazy::new(|| TestUser {
        id: "1".to_string(),
        name: "Test1".to_string(),
    });
    static USER2: Lazy<TestUser> = Lazy::new(|| TestUser {
        id: "2".to_string(),
        name: "Test1".to_string(),
    });

    #[tokio::test]
    async fn test_insert_1() -> Result<()> {
        let pb = PathBuf::from("data/tests/users");
        let mut repo = FileRepository::<String, TestUser>::new("users".to_string(), pb)?;
        let user1 = &*USER1;
        repo.insert(user1.clone())
            .await
            .expect("Failed to create user");

        let user2 = &*USER2;
        repo.insert(user2.clone())
            .await
            .expect("Failed to create user");

        Ok(())
    }

    #[tokio::test]
    async fn test_find_all() -> Result<()> {
        let pb = PathBuf::from("data/tests/users");
        let mut repo = FileRepository::<String, TestUser>::new("users".to_string(), pb)?;
        repo.initialize().await?;
        let values = repo.find_all().await?;
        println!("{}", values.len());
        // assert_eq!(values.len(), 2);
        Ok(())
    }
}
