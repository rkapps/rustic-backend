use std::sync::Arc;

use agentic_boot::startup::boot::BootState;
use anyhow::Result;
use axum::extract::FromRef;

#[derive(Clone)]
pub struct AppState {
    pub boot_state: Arc<BootState>,
}

impl AppState {
    pub async fn new(boot_state: Arc<BootState>) -> Result<Self> {
        Ok(Self { boot_state })
    }
}

impl FromRef<AppState> for Arc<BootState> {
    fn from_ref(state: &AppState) -> Arc<BootState> {
        state.boot_state.clone()
    }
}
