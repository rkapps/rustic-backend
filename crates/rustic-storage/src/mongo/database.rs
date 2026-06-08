use std::{any::Any, collections::HashMap, sync::Arc};

use anyhow::Result;
use mongodb::Client;
use tokio::sync::Mutex;

use crate::{
    core::repository::{RepoKey, RepoModel},
    mongo::{error::MongoDatabaseError, repository::MongoRepository},
};

/// MongoDB-backed database that manages a set of typed collections.
///
/// Mirrors the API of [`super::super::file::database::FileDatabase`] so callers
/// can swap backends without changing application code.  Each collection is
/// stored in a `repos` entry keyed by collection name; entries are typed-erased
/// as `Arc<dyn Any>` and downcast on access.
#[derive(Debug, Clone)]
pub struct MongoDatabase {
    name: String,
    client: mongodb::Client,
    repos: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl MongoDatabase {
    /// Connect to `uri` and open (or create) the database named `name`.
    pub async fn new(uri: &str, name: &str) -> Result<Self> {
        let client = Client::with_uri_str(uri).await?;
        Ok(MongoDatabase {
            name: name.to_string(),
            client,
            repos: HashMap::new(),
        })
    }

    /// Register a collection, wrapping the MongoDB collection handle in a
    /// [`MongoRepository`] and storing it under `name`.
    ///
    /// Must be called before [`collection`](MongoDatabase::collection).
    pub async fn register_collection<K, M>(&mut self, name: String) -> Result<()>
    where
        K: RepoKey,
        M: RepoModel<K>,
    {
        let collection = self.client.database(&self.name).collection(&name);
        let repository = MongoRepository::<K, M>::new(collection)?;
        let arc = Arc::new(Mutex::new(repository));
        self.repos.insert(name, arc as Arc<dyn Any + Send + Sync>);

        Ok(())
    }

    /// Return a shared handle to a previously registered collection.
    ///
    /// Returns an error if the collection was not registered or if the stored
    /// `Arc` cannot be downcast to `(K, M)`.
    pub async fn collection<K, M>(&self, name: String) -> Result<Arc<Mutex<MongoRepository<K, M>>>>
    where
        K: RepoKey,
        M: RepoModel<K>,
    {
        let v = self
            .repos
            .get(&name)
            .ok_or(MongoDatabaseError::CollectionRespositoryError {
                path: name.clone().into(),
            })?;

        let repo = Arc::clone(v)
            .downcast::<Mutex<MongoRepository<K, M>>>()
            .map_err(|_| {
                anyhow::anyhow!(MongoDatabaseError::CollectionRepoisitoryDowncastError {
                    path: name.into()
                })
            })?;

        Ok(repo)
    }
}
