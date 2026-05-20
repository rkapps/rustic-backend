//! Append-only flat-file storage backend.
//!
//! Records are serialised as BSON and written sequentially to a `.bin` file,
//! one file per collection.  Deletes append a tombstone record rather than
//! physically removing data, keeping the file write-path simple and crash-safe.
//!
//! At startup [`repository::FileRepository`] replays the log to rebuild an
//! in-memory offset map (`id → byte offset`), which is used for O(1)
//! point-lookups during the process lifetime.
//!
//! # Module layout
//! - [`collections`] – persisted metadata for registered collections.
//! - [`database`] – [`database::FileDatabase`] manages multiple collections.
//! - [`errors`] – error types specific to this backend.
//! - [`file`] – low-level binary record format (header, CRC, BSON payload).
//! - [`repository`] – [`repository::FileRepository`] implements [`crate::core::repository::Repository`].
//! - [`sort`] – in-memory multi-key sort helper.
//! - [`utils`] – path-building utilities.

pub mod collections;
pub mod database;
pub mod errors;
pub mod file;
pub mod repository;
pub mod sort;
pub mod utils;
