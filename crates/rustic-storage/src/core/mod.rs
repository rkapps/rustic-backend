//! Core storage abstractions.
//!
//! [`repository`] defines the trait contract that every backend must satisfy.
//! [`search`] provides the backend-agnostic query DSL used to express filters,
//! sorts, and limits without coupling callers to a specific store.

pub mod repository;
pub mod search;
