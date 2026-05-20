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
//! # Quick start
//!
//! ```no_run
//! use rustic_storage::file::database::FileDatabase;
//! use rustic_storage::core::repository::Repository;
//! ```

pub mod core;
pub mod file;
pub mod mongo;
