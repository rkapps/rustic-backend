use crate::storage::{
    mongo::manager::FinanceMongoStorageManager,
    reader::{
        StorageReader, TickerControlStorageReader, TickerEmbeddingStorageReader,
        TickerHistoryStorageReader, TickerIndicatorStorageReader, TickerNewsStorageReader,
        TickerSentimentStorageReader, TickerStorageReader,
    },
};
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct FinanceMongoStorageReader {
    pub(crate) manager: FinanceMongoStorageManager,
}
impl FinanceMongoStorageReader {
    pub fn new(manager: FinanceMongoStorageManager) -> Self {
        Self { manager }
    }
}

// 2. The Blanket Implementation (The "Glue")
impl<T> StorageReader for T where
    T: TickerControlStorageReader
        + TickerStorageReader
        + TickerHistoryStorageReader
        + TickerIndicatorStorageReader
        + TickerSentimentStorageReader
        + TickerEmbeddingStorageReader
        + TickerNewsStorageReader
        + Send
        + Sync
        + Debug
{
}
