//! MongoDB storage backend.
//!
//! [`database::MongoDatabase`] wraps a MongoDB client and manages a set of
//! typed collections.  [`repository::MongoRepository`] implements
//! [`crate::core::repository::Repository`] using the official `mongodb` driver.
//!
//! Queries expressed as [`crate::core::search::SearchCriteria`] are translated
//! to BSON filter documents by [`MongoCriteriaBuilder`].

pub mod critera;
pub mod database;
pub mod error;
pub mod repository;
pub use critera::MongoCriteriaBuilder;
