mod common;
use std::sync::Arc;

use anyhow::Result;
use rustic_storage::{core::repository::Repository, file::database::FileDatabase};
use tokio::{sync::Mutex, task::JoinHandle};

use crate::common::models::{Account, Service, User};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let db =
        Mutex::new(FileDatabase::new("mystore".to_string(), "data/mystoredb".to_string()).await?);
    let service = Service::new(db);

    // Clone the Arc before spawning each thread
    // Moved the cloned Arc into the thread's closure
    let db1 = Arc::clone(&service.db);
    let db2 = Arc::clone(&service.db);

    let handle1: JoinHandle<Result<(), anyhow::Error>> = tokio::spawn(async move {
        // Lock the mutex inside the thread to read and write
        let mut guard = db1.lock().await;
        let _ = guard.register_collection::<String, User>("user".to_string());
        {
            let urepo_guard = guard
                .collection::<String, User>("users".to_string())
                .await
                .unwrap();
            let mut urepo = urepo_guard.lock().await;

            println!("Starting user thread");
            for i in 0..4 {
                let id = i.to_string();
                let user1 = User {
                    id: id.clone(),
                    name: ["storage_test".to_string() + "-" + &id].concat(),
                };

                let _ = urepo.insert(user1.clone()).await;
                println!("User {:?} inserted", user1);
            }
            let users = urepo.find_all().await?;
            println!("Users count {:?}", users.len());
        }
        Ok(())
    });

    let handle2: JoinHandle<Result<(), anyhow::Error>> = tokio::spawn(async move {
        let mut guard = db2.lock().await;
        let _ = guard.register_collection::<String, Account>("account".to_string());

        println!("Starting account thread");
        for i in 0..4 {
            let id = i.to_string();

            // drop the guard as soon as we get the repo so that it can borrowed again below
            let user = {
                let repo_result = guard.collection::<String, User>("users".to_string()).await;
                let urepo_guard = match repo_result {
                    Ok(c) => c,
                    Err(e) => {
                        return Err(anyhow::anyhow!(format!(
                            "Collection {:} not found: {:?}",
                            "users", e
                        )));
                    }
                };
                let mut urepo = urepo_guard.lock().await;
                urepo.find_by_id(id.clone()).await
            }?;

            // let user = match user_option {
            //     Some(user) => user,
            //     None => return Err(anyhow::anyhow!(format!("user {:} not found", id))),
            // };

            let arepo_guard = guard
                .collection::<String, Account>("account".to_string())
                .await
                .unwrap();
            let mut arepo = arepo_guard.lock().await;

            // if the user is available then create the accounts
            for j in 0..4 {
                let id = j.to_string();
                let account = Account::new(user.id.to_string(), id);
                let _ = arepo.insert(account.clone()).await;
                println!("Account {:?} for user {:?} created", account.id, user.id);
            }
            let accounts = arepo.find_all().await?;
            println!("Accounts count {:?}", accounts.len());
        }

        println!("Done");
        Ok(())
    });

    handle1.await??;
    handle2.await??;

    Ok(())
}
