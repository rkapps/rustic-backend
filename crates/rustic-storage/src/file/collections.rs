use serde::{Deserialize, Serialize};

/// Persisted metadata for a single collection registered with [`super::database::FileDatabase`].
///
/// Stored inside the database's top-level JSON manifest file so that the list
/// of known collections (and their directory paths) survives process restarts.
#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionMetadata {
    pub name: String,
}
