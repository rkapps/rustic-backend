use std::{convert::Infallible, sync::Arc};

use axum::{
    Extension, Json, Router,
    extract::{FromRef, Path, Query, State},
    middleware::from_fn_with_state,
    response::{IntoResponse, Sse, sse::Event},
    routing::{delete, get, patch, post},
};
use futures::StreamExt;
use reqwest::StatusCode;
use rustic_agent::agents::domain::TurnChunkResponse;
use tracing::{debug, error};

use crate::{
    auth::firebase::{FirebaseClaims, firebase_auth_middleware},
    boot::BootState,
    conversation::{
        domain::{Conversation, ConversationRequest, ConversationUpdateRequest, Turn},
        dto::{ConversationsQuery, TurnRequest, TurnResponse},
    },
    routes::domain::UiChunk,
};

pub fn conversation_routes<S>(state: S) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    Arc<BootState>: FromRef<S>,
{
    Router::new()
        // conversations
        .route("/conversations", get(get_conversations_handler))
        .route("/conversations", post(create_conversation_handler))
        .route("/conversations/{id}", patch(update_conversation_handler))
        .route("/conversations/{id}", get(get_conversation_handler))
        .route("/conversations/{id}", delete(delete_conversation_handler))
        .route("/conversations/{id}/turns", get(get_turns_handler))
        .route("/conversations/{id}/turns", post(send_turn_handler))
        .route(
            "/conversations/{id}/turns/stream",
            post(send_turn_streaming_handler),
        )
        .route_layer(from_fn_with_state(state, firebase_auth_middleware::<S>))
}

pub async fn create_conversation_handler(
    State(boot_state): State<Arc<BootState>>,
    Extension(claims): Extension<FirebaseClaims>, // 👈 opt-in
    Json(payload): Json<ConversationRequest>,
) -> Result<Json<Conversation>, (StatusCode, String)> {
    debug!("config: {:?}", payload);

    let service = boot_state.conversation_service()?; // ← one line

    let conversation = service
        .create_conversation(claims.sub, payload)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Save Conversation error: {}", e),
            )
        })?;
    Ok(Json(conversation))
}

pub async fn delete_conversation_handler(
    State(boot_state): State<Arc<BootState>>,
    Extension(user): Extension<FirebaseClaims>, // 👈 opt-in
    Path(id): Path<String>,
) -> Result<(), (StatusCode, String)> {
    debug!("User sub: {:?} conversation id: {}", user.sub, id);

    let service = boot_state.conversation_service()?; // ← one line

    service
        .delete_conversation(&user.sub, &id)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Delete Conversation error: {}", e),
            )
        })?;
    Ok(())
}

pub async fn update_conversation_handler(
    State(boot_state): State<Arc<BootState>>,
    Extension(claims): Extension<FirebaseClaims>, // 👈 opt-in
    Path(id): Path<String>,
    Json(payload): Json<ConversationUpdateRequest>,
) -> Result<Json<Conversation>, (StatusCode, String)> {
    debug!("config: {:?}", payload);

    let service = boot_state.conversation_service()?; // ← one line

    let conversation = service
        .update_conversation(&claims.sub, &id, payload)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Save Conversation error: {}", e),
            )
        })?;
    Ok(Json(conversation))
}

pub async fn get_conversations_handler(
    State(boot_state): State<Arc<BootState>>,
    Extension(user): Extension<FirebaseClaims>, // 👈 opt-in
    Query(query): Query<ConversationsQuery>,
) -> Result<Json<Vec<Conversation>>, (StatusCode, String)> {
    debug!(
        "User sub: {:?} query: {:?}",
        user.sub, query.conversation_type
    );
    let service = boot_state.conversation_service()?; // ← one line

    let conversations = service
        .get_conversations(user.sub, query)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Get All Chats error: {}", e),
            )
        })?;
    Ok(Json(conversations))
}

pub async fn get_conversation_handler(
    State(boot_state): State<Arc<BootState>>,
    Extension(user): Extension<FirebaseClaims>, // 👈 opt-in
    Path(id): Path<String>,
) -> Result<Json<Conversation>, (StatusCode, String)> {
    debug!("User sub: {:?} conversation id: {}", user.sub, id);

    let service = boot_state.conversation_service()?; // ← one line
    let conversation = service
        .get_conversation(&user.sub, &id)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Get Conversation error: {}", e),
            )
        })?;
    Ok(Json(conversation))
}

pub async fn get_turns_handler(
    State(boot_state): State<Arc<BootState>>,
    Extension(user): Extension<FirebaseClaims>, // 👈 opt-in
    Path(id): Path<String>,
) -> Result<Json<Vec<Turn>>, (StatusCode, String)> {
    debug!("User sub: {:?}", user.sub);

    let service = boot_state.conversation_service()?; // ← one line

    let conversations = service.get_turns(&user.sub, &id).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Get Conversation Turns error: {}", e),
        )
    })?;
    Ok(Json(conversations))
}

pub async fn send_turn_handler(
    State(boot_state): State<Arc<BootState>>,
    Extension(user): Extension<FirebaseClaims>, // 👈 opt-in
    Path(id): Path<String>,
    Json(request): Json<TurnRequest>,
) -> Result<Json<TurnResponse>, (StatusCode, String)> {
    let service = boot_state.conversation_service()?; // ← one line

    let response = service
        .send_turn(&user.sub, &id, request)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Send Turn error: {}", e)))?;

    Ok(Json(response))
}

#[axum::debug_handler]
pub async fn send_turn_streaming_handler(
    State(boot_state): State<Arc<BootState>>,
    Extension(user): Extension<FirebaseClaims>, // 👈 opt-in
    Path(id): Path<String>,
    Json(request): Json<TurnRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    debug!("User sub: {:?} conversation id: {}", user.sub, id);

    let service = boot_state.conversation_service()?; // ← one line
    let start = std::time::Instant::now();

    let stream = service
        .send_turn_streaming(&user.sub, &id, request.clone())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let cservice = service.clone(); // ← clone Arc
    let request = request.clone();

    let event_stream = stream.then(move |chunk_result| {
        // ✅ Clone handles into the async block
        let conversation_service = cservice.clone();
        let request = request.clone();
        let uid = user.sub.clone();
        let id = id.clone();

        async move {
            match chunk_result {
                Ok(chunk) => {
                    if let TurnChunkResponse::Final { response } = &chunk {
                        // already handled above — send done event
                        let turn_response = response.clone();
                        debug!("Turn Response: {}", turn_response);
                        let elapsed = start.elapsed();
                        match conversation_service
                            .save_turn(
                                &uid,
                                &id,
                                request.prompt,
                                turn_response.content.clone(),
                                turn_response.response_id.clone(),
                                Some(turn_response.usage.clone()),
                                Some(elapsed.as_millis() as u64),
                            )
                            .await
                        {
                            Ok(c) => c,
                            // ❌ Don't silently swallow errors
                            Err(e) => {
                                error!("Failed to save turn: {}", e);
                                // emit error event to UI
                                return Ok(Event::default()
                                    .data(format!("{{\"error\": \"Failed to save turn: {}\"}}", e))
                                    .event("error"));
                            }
                        };
                    }

                    let ui_chunk = UiChunk::from(&chunk);
                    match serde_json::to_string(&ui_chunk) {
                        Ok(data) => {
                            Ok::<Event, Infallible>(Event::default().data(data).event("message"))
                        }
                        Err(e) => Ok::<Event, Infallible>(
                            Event::default().data(format!("{}", e)).event("error"),
                        ),
                    }
                }
                Err(e) => {
                    error!("error: {:?}", e);
                    Ok::<Event, Infallible>(Event::default().data(format!("{}", e)).event("error"))
                }
            }
        }
    });

    Ok(Sse::new(event_stream).into_response())
}
