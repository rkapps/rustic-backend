use std::fmt::Debug;

use crate::storage::{
    mongo::manager::EconomicMongoStorageManager,
    reader::{BeaStorageReader, CensusStorageReader, FredStorageReader, StorageReader},
};

#[derive(Debug, Clone)]
pub struct EconomicMongoStorageReader {
    pub(crate) manager: EconomicMongoStorageManager,
}
impl EconomicMongoStorageReader {
    pub fn new(manager: EconomicMongoStorageManager) -> Self {
        Self { manager }
    }
}

// 2. The Blanket Implementation (The "Glue")
impl<T> StorageReader for T where
    T: BeaStorageReader + FredStorageReader + CensusStorageReader + Send + Sync + Debug
{
}
