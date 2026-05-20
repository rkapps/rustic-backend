use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by [`super::database::MongoDatabase`].
#[derive(Error, Debug)]
pub enum MongoDatabaseError {
    #[error("Repository for collection {path} could not get created")]
    CollectionRespositoryError { path: PathBuf },

    /// Returned when [`MongoDatabase::collection`](super::database::MongoDatabase::collection)
    /// is called for a name that was never registered.
    #[error("Repository for collection {path} is missing")]
    CollectionRepoisitoryMissingError { path: PathBuf },

    /// Returned when the stored `Arc<dyn Any>` cannot be downcast to the
    /// expected `(K, M)` pair — usually a programming error.
    #[error("Repository for collection {path} could not be downcast")]
    CollectionRepoisitoryDowncastError { path: PathBuf },
}

/// Errors produced by [`super::repository::MongoRepository`].
#[derive(Error, Debug)]
pub enum MongoRepositoryError {
    #[error("Failed to create directory: {path}")]
    DirectoryCreation { path: PathBuf },

    #[error("Failed to write file to: {path}")]
    FileCreation { path: PathBuf },

    #[error("Failed to delete file to: {path}")]
    FileDeletion { path: PathBuf },
}

/// Errors produced while reading or validating a binary record header
/// (shared definition with the file backend).
#[derive(Error, Debug)]
pub enum RecordHeaderError {
    #[error("Invalid magic: {magic}")]
    InvalidMagic { magic: u32 },

    #[error("Unsupported version: {version}")]
    UnsupportedVersion { version: u8 },

    #[error("Corruped Data: {offset}, {expected}, {actual}")]
    CorruptedData {
        offset: u64,
        expected: u32,
        actual: u32,
    },
}
