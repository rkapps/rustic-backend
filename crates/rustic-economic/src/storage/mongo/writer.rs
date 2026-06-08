use std::fmt::Debug;

use crate::storage::{
    mongo::manager::EconomicMongoStorageManager,
    writer::{BeaStorageWriter, CensusStorageWriter, FredStorageWriter, StorageWriter},
};

#[derive(Debug)]
pub struct EconomicMongoStorageWriter {
    pub(crate) manager: EconomicMongoStorageManager,
}
impl EconomicMongoStorageWriter {
    pub(crate) fn new(manager: EconomicMongoStorageManager) -> Self {
        Self { manager }
    }
}

// 2. The Blanket Implementation (The "Glue")
impl<T> StorageWriter for T where
    T: BeaStorageWriter + FredStorageWriter + CensusStorageWriter + Send + Sync + Debug
{
}
