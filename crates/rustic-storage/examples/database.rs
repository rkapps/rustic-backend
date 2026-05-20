mod common;

use anyhow::Result;
use rustic_core::logger::set_logger;
use rustic_storage::{core::repository::Repository, file::database::FileDatabase};
use crate::common::models::{Account, User};

#[tokio::main]
async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        "rustic_storage::examples,rustic-storage::file".to_string()
    });
    set_logger(filter);

    let mut fsdb = FileDatabase::new("mystore".to_string(), "data/mystoredb".to_string()).await?;
    fsdb.register_collection::<String, User>("user".to_string())
        .await?;

    {
        let urepo_guard = fsdb.collection::<String, User>("user".to_string()).await?;

        let user1 = User {
            id: "1".to_string(),
            name: "storage_test1".to_string(),
        };
        let user2 = User {
            id: "2".to_string(),
            name: "storage_test2".to_string(),
        };
        let cuser1 = User {
            id: "1".to_string(),
            name: "storage_test1111111111".to_string(),
        };
        let mut urepo = urepo_guard.lock().await;

        urepo.insert(user2).await?;
        urepo.find_by_id("1".to_string()).await;
        urepo.find_by_id("2".to_string()).await;
        urepo.update(cuser1).await?;
        urepo.find_by_id("1".to_string()).await;
        urepo.delete(user1).await?;

        let option = urepo.find_by_id("2".to_string()).await;
        println!("User {:?}", Some(option));
        let users = urepo.find_all().await;
        println!("User count {:?}", users);
    }

    {
        let _ = fsdb
            .register_collection::<String, Account>("account".to_string())
            .await?;
        let arepo_guard = fsdb.collection("account".to_string()).await?;

        let account1 = Account::new("1".to_string(), "1".to_string());
        let account2 = Account::new("2".to_string(), "2".to_string());

        let mut arepo = arepo_guard.lock().await;
        arepo.insert(account1).await?;
        arepo.insert(account2).await?;

        let accounts = arepo.find_all().await?;
        println!("account count {:?}", accounts.len());
    }
    Ok(())
}
