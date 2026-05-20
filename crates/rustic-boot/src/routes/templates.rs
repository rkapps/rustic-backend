use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{FromRef, State},
    routing::get,
};
use reqwest::StatusCode;

use crate::{boot::BootState, config::load::ChatTemplate};


pub fn template_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    Arc<BootState>: FromRef<S>,
{
    Router::new().route("/chat-templates", get(get_chat_templates_handler))
}

async fn get_chat_templates_handler(
    State(boot): State<Arc<BootState>>,
) -> Result<Json<Vec<ChatTemplate>>, (StatusCode, String)> {
    Ok(Json(boot.chat_templates.clone()))
}
