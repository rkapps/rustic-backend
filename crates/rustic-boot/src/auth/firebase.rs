use axum::{
    Json,
    extract::{FromRef, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, error, trace};

use crate::boot::BootState;

const FIREBASE_JWKS_URL: &str =
    "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com";

pub struct FirebaseKeyCache {
    pub keys: HashMap<String, String>,
    pub fetched_at: std::time::Instant,
}

impl FirebaseKeyCache {
    pub fn is_stale(&self) -> bool {
        // Firebase keys are valid for ~1 hour, refresh every 55 min
        self.fetched_at.elapsed().as_secs() > 55 * 60
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirebaseClaims {
    pub sub: String,
    pub email: Option<String>,
    pub aud: String,
    pub iat: usize,
    pub exp: usize,
}

pub async fn firebase_auth_middleware<S>(
    State(boot): State<Arc<BootState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<String>)>
where
    S: Clone + Send + Sync + 'static,
    Arc<BootState>: FromRef<S>,
{
    let token = extract_bearer_token(&request)?;

    let header = decode_header(&token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json("Invalid token header".into()),
        )
    })?;

    let kid = header.kid.ok_or((
        StatusCode::UNAUTHORIZED,
        Json("Missing kid in token header".into()),
    ))?;

    let keys = get_keys(&boot).await?; // ← boot not state

    let public_key = match keys.get(&kid) {
        Some(k) => k.clone(),
        None => {
            debug!("kid not found in cache, forcing key refresh");
            let fresh_keys = refresh_keys(&boot).await?; // ← boot not state
            fresh_keys.get(&kid).cloned().ok_or((
                StatusCode::UNAUTHORIZED,
                Json("Unknown token signing key".into()),
            ))?
        }
    };

    let claims = validate_token(&boot.firebase_project_id, &token, &public_key)?;
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn extract_bearer_token(request: &Request) -> Result<String, (StatusCode, Json<String>)> {
    request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.to_string())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json("Missing or invalid Authorization header".into()),
        ))
}

async fn get_keys(boot: &BootState) -> Result<HashMap<String, String>, (StatusCode, Json<String>)> {
    // ← BootState
    let cache = boot.firebase_keys.read().await; // ← boot not state
    if cache.is_stale() {
        drop(cache);
        return refresh_keys(boot).await;
    }
    trace!("Keys in cache: {:?}", cache.keys.keys().collect::<Vec<_>>());
    Ok(cache.keys.clone())
}

async fn refresh_keys(
    boot: &BootState, // ← BootState
) -> Result<HashMap<String, String>, (StatusCode, Json<String>)> {
    let fresh_keys = fetch_firebase_keys().await.map_err(|e| {
        error!("Failed to refresh Firebase keys: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json("Failed to refresh Firebase keys".into()),
        )
    })?;

    let mut cache = boot.firebase_keys.write().await; // ← boot not state
    cache.keys = fresh_keys.clone();
    cache.fetched_at = std::time::Instant::now();

    Ok(fresh_keys)
}

pub async fn fetch_firebase_keys() -> anyhow::Result<HashMap<String, String>> {
    let keys: HashMap<String, String> = reqwest::get(FIREBASE_JWKS_URL).await?.json().await?;
    Ok(keys)
}

fn validate_token(
    project_id: &str, // ← passed in
    token: &str,
    public_key_pem: &str,
) -> Result<FirebaseClaims, (StatusCode, Json<String>)> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[&project_id]);
    validation.set_issuer(&[format!("https://securetoken.google.com/{}", project_id)]);

    let decoding_key = DecodingKey::from_rsa_pem(public_key_pem.as_bytes()).map_err(|e| {
        error!("Failed to parse Firebase public key: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json("Failed to parse Firebase public key".into()),
        )
    })?;

    decode::<FirebaseClaims>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|e| {
            debug!("Token validation failed: {}", e);
            (
                StatusCode::UNAUTHORIZED,
                Json(format!("Invalid token: {}", e)),
            )
        })
}
