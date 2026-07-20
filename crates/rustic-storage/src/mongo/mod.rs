//! MongoDB storage backend.
//!
//! [`database::MongoDatabase`] wraps a MongoDB client and manages a set of
//! typed collections.  [`repository::MongoRepository`] implements
//! [`crate::core::repository::Repository`] using the official `mongodb` driver.
//!
//! Queries expressed as [`crate::core::search::SearchCriteria`] are translated
//! to BSON filter documents by [`MongoCriteriaBuilder`].

pub mod critera;
pub mod database;
pub mod error;
pub mod repository;
use anyhow::Result;
pub use critera::MongoCriteriaBuilder;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{Repository, core::index::IndexDefinition};

pub async fn create_indexes_safe<K, M, R>(
    repo: Arc<Mutex<R>>,
    indexes: Vec<IndexDefinition>,
) -> Result<()>
where
    R: Repository<K, M>,
    K: Send + Sync,
    M: Send + Sync,
{
    let mut repo = repo.lock().await;
    let collection_name = repo.collection_name().to_string();

    for index in indexes {
        match repo.create_index(index.clone()).await {
            Ok(_) => info!(
                "Collection {:?} Index {:?} created",
                collection_name, index.name
            ),
            Err(e) if e.to_string().contains("already exists") => {
                warn!("Index already exists, skipping: {}", e);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
