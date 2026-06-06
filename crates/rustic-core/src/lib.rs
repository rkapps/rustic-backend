//! Core HTTP utilities and shared infrastructure for the rustic-ai platform.
//!
//! # Re-exports
//!
//! ```no_run
//! use rustic_core::{HttpClient, HttpError, HttpResult, HttpResponse};
//! use rustic_core::set_logger;
//! ```

pub mod agents;
pub mod config;
pub mod http;
pub mod logger;

pub use agents::tools::Tool;
pub use config::load::load_content;
pub use http::error::HttpError;
pub use http::http::{HttpClient, HttpResponse, HttpResult};
pub use logger::set_logger;
