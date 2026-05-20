//! `rustic-storage` provides a backend-agnostic repository layer for persisting
//! and querying domain models.
//!
//! # Architecture
//!
//! The crate is split into three layers:
//!
//! - **`core`** – Traits that define the storage contract ([`core::repository::Repository`],
//!   [`core::repository::RepoModel`], etc.) and the query DSL ([`core::search::SearchCriteria`]).
//!   Application code should depend only on this layer.
//!
//! - **`file`** – Append-only, BSON-encoded flat-file backend.  Each collection is a single
//!   `.bin` file; deletes are recorded as tombstone records rather than physical removal.
//!   Intended for local / edge deployments and testing without a running database.
//!
//! - **`mongo`** – MongoDB backend that implements the same `Repository` trait using the
//!   official `mongodb` driver.
//!
//! # Re-exports
//!
//! Commonly used types are available directly at the crate root:
//!
//! ```no_run
//! use rustic_storage::{Repository, RepoModel, RepoKey, Searchable, VectorEmbedding};
//! use rustic_storage::SearchCriteria;
//! use rustic_storage::{FileDatabase, MongoDatabase};
//! ```

pub mod core;
pub mod file;
pub mod mongo;

// Core traits
pub use core::repository::{RepoKey, RepoModel, Repository, Searchable, SortValue, VectorEmbedding};

// Query DSL — only the builder is part of the public API
pub use core::search::SearchCriteria;

// Database handles
pub use file::database::FileDatabase;
pub use mongo::database::MongoDatabase;
