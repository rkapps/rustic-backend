use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by [`super::database::FileDatabase`].
#[derive(Error, Debug)]
pub enum FileDatabaseError {
    #[error("Repository for collection {path} could not get created")]
    CollectionRespositoryError { path: PathBuf },

    /// Returned when [`FileDatabase::collection`](super::database::FileDatabase::collection)
    /// is called for a name that was never registered.
    #[error("Repository for collection {path} is missing")]
    CollectionRepoisitoryMissingError { path: PathBuf },

    /// Returned when the stored `Arc<dyn Any>` cannot be downcast to the
    /// expected `(K, M)` type pair — usually a programming error.
    #[error("Repository for collection {path} could not be downcast")]
    CollectionRepoisitoryDowncastError { path: PathBuf },
}

/// Errors produced by [`super::repository::FileRepository`].
#[derive(Error, Debug)]
pub enum FileRepositoryError {
    #[error("Failed to create directory: {path}")]
    DirectoryCreation { path: PathBuf },

    #[error("Failed to write file to: {path}")]
    FileCreation { path: PathBuf },

    #[error("Failed to delete file to: {path}")]
    FileDeletion { path: PathBuf },
}

/// Errors produced while reading or validating a binary record header.
#[derive(Error, Debug)]
pub enum RecordHeaderError {
    /// The magic bytes did not match `0xDEADBEEF`, indicating a corrupt or
    /// foreign file.
    #[error("Invalid magic: {magic}")]
    InvalidMagic { magic: u32 },

    /// The record was written by a newer version of the library.
    #[error("Unsupported version: {version}")]
    UnsupportedVersion { version: u8 },

    /// The CRC32 of the payload did not match the value stored in the header.
    #[error("Corruped Data: {offset}, {expected}, {actual}")]
    CorruptedData {
        offset: u64,
        expected: u32,
        actual: u32,
    },
}
