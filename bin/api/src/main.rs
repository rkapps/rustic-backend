use std::{env, sync::Arc};

use anyhow::Result;
use rustic_ai_api::state::AppState;
use rustic_boot::{
    boot,
    routes::{
        agents::agent_routes, conversation::conversation_routes, providers::provider_routes,
        templates::template_routes,
    },
};
use rustic_core::{Tool, logger::set_logger_with_telemetry};
use rustic_economic::service::EconomicService;
use rustic_finance::service::FinanceService;
use rustic_ml::embeddings::openai::OpenAIEmbeddingClient;
use tracing::info;

#[tokio::main]

async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        "rustic_ai_api=debug,rustic_boot=info,rustic_core=info,fin_analyse=info".to_string()
    });

    let firebase_project_id = env::var("RUSTIC_AI_PROJECT_ID")
        .expect("RUSTIC_AI_PROJECT_ID envrionment variable not set");

    let endpoint = std::env::var("OTEL_ENDPOINT")?;
    let _ = set_logger_with_telemetry(filter, "rustic-ai-api", &firebase_project_id, &endpoint).await?;

    // Get Embedding client
    let openai_api_key: String =
        env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");
    let embedding_client = Arc::new(OpenAIEmbeddingClient::new(&openai_api_key)?);

    let config_dir = env::var("RUSTIC_AI_CONFIG_PATH")
        .expect("RUSTIC_AI_CONFIG_PATH envrionment variable not set");

    let mongo_uri = env::var("MONGO_URI").expect("MONGO_URI envrionment variable not set");

    let mongo_db = env::var("RUSTIC_FINANCE_DB_NAME")
        .expect("RUSTIC_FINANCE_DB_NAME envrionment variable not set");
    info!("Finance Data Mongo uri: {:?} db: {:?}", mongo_uri, mongo_db);
    let finance_service =
        FinanceService::new_reader(&mongo_uri, &mongo_db, embedding_client).await?;

    // economic data
    let mongo_db = env::var("RUSTIC_ECONOMIC_DB_NAME")
        .expect("RUSTIC_ECONOMIC_DB_NAME envrionment variable not set");
    info!(
        "Economic data Mongo uri: {:?} db: {:?}",
        mongo_uri, mongo_db
    );
    let economic_service = EconomicService::new_reader(&mongo_uri, &mongo_db).await?;

    let mongo_db = env::var("RUSTIC_PLATFORM_DB_NAME")
        .expect("RUSTIC_AI_DB_NAME envrionment variable not set");
    info!(
        "Platform Data Mongo uri: {:?} db: {:?}",
        mongo_uri, mongo_db
    );
    let mut tools: Vec<Arc<dyn Tool>> = vec![];
    tools.extend(economic_service.tools());
    tools.extend(finance_service.tools());

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    let origins = [
        "http://localhost:4200",
        "http://localhost:4201",
        "http://localhost:4202",
        "https://rustic-ai-rkapps.web.app",
        "https://ijn-rustic-ai.web.app",
    ];

    boot::AgenticBootBuilder::new()
        .config_dir(config_dir.to_string())
        .firebase_project_id(&firebase_project_id)
        .chat_templates("chat_templates.json".to_string())
        .providers("providers.json".to_string())
        .agents_config("agents.json".to_string())
        .mcp_config("mcp_servers_config.json".to_string())
        .mongo_database(mongo_uri, mongo_db)
        .cors_origins(origins.to_vec())
        .tools(tools)
        .serve(
            &addr,
            |boot| AppState {
                boot_state: Arc::new(boot),
            },
            |router, _| {
                router
                    .merge(agent_routes())
                    .merge(template_routes())
                    .merge(provider_routes())
            },
            |router, state| router.merge(conversation_routes(state.clone())), // .nest("/finance", finance_routes(state)),
        )
        .await?;

    Ok(())
}
