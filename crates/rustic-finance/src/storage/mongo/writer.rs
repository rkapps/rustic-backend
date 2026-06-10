use crate::storage::{
    mongo::manager::FinanceMongoStorageManager,
    writer::{
        StorageWriter, TickerControlStorageWriter, TickerEmbeddingStorageWriter, TickerHistoryStorageWriter, TickerIndicatorStorageWriter, TickerNewsStorageWriter, TickerSentimentStorageWriter, TickerStorageWriter
    },
};
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct FinanceMongoStorageWriter {
    pub(crate) manager: FinanceMongoStorageManager,
}
impl FinanceMongoStorageWriter {
    pub(crate) fn new(manager: FinanceMongoStorageManager) -> Self {
        Self { manager }
    }
}

// 2. The Blanket Implementation (The "Glue")
impl<T> StorageWriter for T where
    T: TickerControlStorageWriter
        + TickerStorageWriter
        + TickerHistoryStorageWriter
        + TickerIndicatorStorageWriter
        + TickerSentimentStorageWriter
        + TickerEmbeddingStorageWriter
        + TickerNewsStorageWriter
        + Send
        + Sync
        + Debug
{
}
