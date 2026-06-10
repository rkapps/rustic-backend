use std::sync::Arc;

use anyhow::Result;
use rustic_storage::{Repository, core::index::IndexDefinition};
use tokio::sync::Mutex;
use tracing::{info, warn};

pub mod rustic_economic;
pub mod rustic_finance;
pub mod rustic_platform;

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
    for index in indexes {
        match repo.create_index(index).await {
            Ok(_) => info!("Index created"),
            Err(e) if e.to_string().contains("already exists") => {
                warn!("Index already exists, skipping: {}", e);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
