use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;
use tracing::debug;

use crate::{
    core::repository::{RepoKey, RepoModel},
    file::{
        collections::CollectionMetadata, errors::FileDatabaseError, repository::FileRepository,
        utils,
    },
};

/// A lightweight, file-backed "database" that manages a set of named collections.
///
/// Each `FileDatabase` is represented on disk as a JSON manifest file that
/// records which collections exist.  Individual collections live in
/// sub-directories under `file_path`, each containing one `.bin` log file.
///
/// `repos` is not serialised — it is rebuilt in memory when
/// [`register_collection`](FileDatabase::register_collection) is called.
/// Callers must register every collection they intend to use after constructing
/// the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileDatabase {
    name: String,
    file_path: String,
    collections: HashMap<String, CollectionMetadata>,

    /// Runtime repository handles — not persisted.
    #[serde(skip)]
    repos: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl FileDatabase {
    /// Open (or create) a database stored at `file_path` with the given `name`.
    ///
    /// If the manifest JSON already exists it is loaded; otherwise a fresh
    /// database is created.
    pub async fn new(name: String, file_path: String) -> Result<Self> {
        let mut db = FileDatabase::load_from_file(&name, &file_path)?;
        db.initialize().await?;
        Ok(db)
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Deserialise the database manifest from `<file_path>/<name>.json`,
    /// returning an empty database if the file does not exist yet.
    fn load_from_file(name: &str, file_path: &str) -> Result<Self> {
        let pathbuf = utils::build_json_file_path(&PathBuf::from(&file_path), name);

        debug!("Loading database from {:?}", pathbuf);
        if Path::new(&pathbuf).exists() {
            let contents = fs::read_to_string(&pathbuf)?;
            debug!("contents: {:#?}", contents);
            Ok(serde_json::from_str(&contents)?)
        } else {
            debug!("File does not exist");
            Ok(Self {
                name: name.to_string(),
                file_path: file_path.to_string(),
                collections: HashMap::new(),
                repos: HashMap::new(),
            })
        }
    }

    /// Serialise the manifest to `<file_path>/<name>.json`.
    async fn save_to_file(&self) -> Result<()> {
        let pathbuf =
            utils::build_json_file_path(&PathBuf::from(&self.file_path), self.name.clone());
        debug!("Saving Database to {:?}", pathbuf);
        let json = serde_json::to_string_pretty(&self)?;
        fs::write(&pathbuf, json)?;
        Ok(())
    }

    /// Register a collection, creating its directory and initialising its
    /// [`FileRepository`] if the collection does not already exist.
    ///
    /// Must be called for each collection before calling [`collection`](FileDatabase::collection).
    pub async fn register_collection<K, M>(&mut self, name: String) -> Result<()>
    where
        K: RepoKey,
        M: RepoModel<K>,
    {
        let full_path = PathBuf::from(&self.file_path).join(&name);

        if !self.collections.contains_key(&name) {
            let metadata = CollectionMetadata { name: name.clone() };
            _ = self.collections.insert(name.clone(), metadata);
            fs::create_dir_all(full_path.clone())?;
            self.save_to_file().await?;
        }

        let mut repository = FileRepository::<K, M>::new(name.clone(), full_path)?;
        repository.initialize().await?;

        let arc = Arc::new(Mutex::new(repository));
        self.repos.insert(name, arc as Arc<dyn Any + Send + Sync>);

        Ok(())
    }

    /// Return a shared handle to an already-registered collection.
    ///
    /// Returns an error if the collection was not registered via
    /// [`register_collection`](FileDatabase::register_collection) or if the
    /// stored `Arc` cannot be downcast to the requested `(K, M)` pair.
    pub async fn collection<K, M>(&self, name: String) -> Result<Arc<Mutex<FileRepository<K, M>>>>
    where
        K: RepoKey,
        M: RepoModel<K>,
    {
        let v =
            self.repos
                .get(&name)
                .ok_or(FileDatabaseError::CollectionRepoisitoryMissingError {
                    path: name.clone().into(),
                })?;

        let repo = Arc::clone(v)
            .downcast::<Mutex<FileRepository<K, M>>>()
            .map_err(|_| {
                anyhow::anyhow!(FileDatabaseError::CollectionRepoisitoryDowncastError {
                    path: name.into()
                })
            })?;

        Ok(repo)
    }
}
