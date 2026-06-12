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
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::{
    auth::firebase::{FirebaseClaims, firebase_auth_middleware},
    boot::BootState,
    conversation::{
        domain::{Conversation, ConversationRequest, ConversationUpdateRequest, Turn},
        dto::{ConversationsQuery, TurnRequest, TurnResponse},
    },
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

    let stream = service
        .send_turn_streaming(&user.sub, &id, request.clone())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // {
    //     Ok(stream) => stream,
    //     Err(e) => {
    //         error!("{:?}", e);
    //         return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    //     }
    // };
    let cservice = service.clone(); // ← clone Arc
    let final_content = Arc::new(Mutex::new(String::new()));
    let request = request.clone();

    let event_stream = stream.then(move |chunk_result| {
        // ✅ Clone handles into the async block
        let conversation_service = cservice.clone();
        let request = request.clone();
        let final_content = final_content.clone();
        let uid = user.sub.clone();
        let id = id.clone();

        async move {
            match chunk_result {
                Ok(chunk) => {
                    // ✅ Always accumulate content once (was being doubled before)
                    {
                        let mut fc = final_content.lock().await;
                        fc.push_str(&chunk.content);
                    }

                    // ✅ Save only on the final chunk
                    if chunk.is_final {
                        let fc = final_content.lock().await;
                        // info!("final_content: {:?}", *fc);
                        let unescaped: serde_json::Value = serde_json::from_str(&fc).unwrap();
                        info!("Final Content {}", unescaped);
                        // info!("Final Content {}", serde_json::to_string_pretty(&unescaped).unwrap());

                        // // ✅ .await now works inside .then()
                        match conversation_service
                            .save_turn(
                                &uid,
                                &id,
                                request.prompt,
                                fc.clone(),
                                Some(chunk.response_id.clone()),
                                chunk.usage.clone(),
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

                    match serde_json::to_string(&chunk) {
                        Ok(c) => Ok::<Event, Infallible>(Event::default().data(c).event("message")),
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
