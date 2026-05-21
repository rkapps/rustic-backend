use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{FromRef, State},
    routing::get,
};
use reqwest::StatusCode;
use rustic_agent::client::llm::LlmProvider;

use crate::boot::BootState;

pub fn provider_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    Arc<BootState>: FromRef<S>,
{
    Router::new().route("/llm-providers", get(get_providers_handler))
}

async fn get_providers_handler(
    State(boot): State<Arc<BootState>>,
) -> Result<Json<Vec<LlmProvider>>, (StatusCode, String)> {
    Ok(Json(boot.agent_service.provider_registry.llm_providers()))
}
