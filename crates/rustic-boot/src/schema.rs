use anyhow::Result;
use rustic_storage::{core::index::IndexDefinition, mongo::create_indexes_safe};
use tracing::info;

use crate::BootStorageManager;


pub async fn update_rustic_platform(mongo_uri: &str, mongo_db: &str) -> Result<()> {
    // let mongo_db = env::var("RUSTIC_PLATFORM_DB_NAME")
    //     .expect("RUSTIC_PLATFORM_DB_NAME envrionment variable not set");
    info!("Updating schema for {} ...", mongo_db);
    let manager = BootStorageManager::new(mongo_uri, &mongo_db).await?;

    let repo = manager.conversations().await?;
    let indexes = get_conversation_index_definitions();
    let _ = create_indexes_safe(repo, indexes).await;

    let repo = manager.turns().await?;
    let indexes = get_turn_index_definitions();
    let _ = create_indexes_safe(repo, indexes).await;

    Ok(())
}

fn get_conversation_index_definitions() -> Vec<IndexDefinition> {
    vec![
        IndexDefinition::new(vec![("id", 1)])
            .unique()
            .named("idx_id"),
        IndexDefinition::new(vec![
            ("uid", 1),
            ("conversation_type", 1),
            ("llm", 1),
            ("last_update", -1),
        ])
        .named("idx_uid_type_llm_last_updated_at"),
        IndexDefinition::new(vec![("uid", 1), ("llm", 1), ("last_update", -1)])
            .named("idx_uid_llm_last_updated_at"),
        IndexDefinition::new(vec![("uid", 1), ("last_update", -1)])
            .named("idx_uid_last_updated_at"),
    ]
}

fn get_turn_index_definitions() -> Vec<IndexDefinition> {
    vec![
        IndexDefinition::new(vec![("id", 1)])
            .unique()
            .named("idx_id"),
        IndexDefinition::new(vec![("uid", 1), ("conversation_id", 1)]).named("idx_conversation"),
    ]
}
