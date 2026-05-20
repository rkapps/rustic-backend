use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{FromRef, State},
    routing::get,
};
use reqwest::StatusCode;
use rustic_agent::services::config::agent::AgentConfig;

use crate::boot::BootState;


pub fn agent_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    Arc<BootState>: FromRef<S>,
{
    Router::new().route("/agents", get(get_agents_handler))
}

async fn get_agents_handler(
    State(boot): State<Arc<BootState>>,
) -> Result<Json<Vec<AgentConfig>>, (StatusCode, String)> {
    Ok(Json(boot.agent_service.agent_registry.catalog()))
}
