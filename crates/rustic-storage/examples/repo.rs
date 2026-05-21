pub mod common;
use anyhow::Result;
use rustic_core::logger::set_logger;
use rustic_storage::{core::repository::Repository, file::repository::FileRepository};
use std::path::PathBuf;

use crate::common::models::User;

#[tokio::main]
async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "rustic_storage::examples,rustic-storage::file".to_string());
    set_logger(filter);

    let pb = PathBuf::from("data/tests/users");
    let mut repo = FileRepository::<String, User>::new("users".to_string(), pb)?;
    repo.initialize().await?;
    // Insert
    let user = User {
        id: "5".to_string(),
        name: "Alice".to_string(),
    };

    let _ = repo.insert(user).await?;

    // Find by ID
    let found = repo.find_by_id("5".to_string()).await;
    println!("Found: {:?}", found);

    Ok(())
}
