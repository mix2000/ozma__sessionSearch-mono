//! HTTP API (same paths and JSON as ozma Project Builder) + embedded UI.

use std::sync::Arc;

use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use session_search_core::Engine;

pub mod assets;
mod dto;
mod error;
mod handlers;
mod middleware;

use handlers::{health, not_found, transcripts};

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    /// Required bearer token for /api, if set.
    pub token: Option<String>,
    /// Reject requests whose Host/Origin is not loopback (DNS rebinding, CSRF).
    pub loopback_only: bool,
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health::health))
        .route("/agents/transcripts/search", get(transcripts::search))
        .route("/agents/transcripts/status", get(transcripts::status))
        .route("/agents/transcripts/sync", post(transcripts::sync))
        .route(
            "/agents/transcripts/{id}/context",
            get(transcripts::context),
        )
        .route("/agents/transcripts/{id}/resume", post(transcripts::resume))
        .fallback(not_found::api_not_found);
    Router::new()
        .nest("/api", api)
        .fallback(assets::serve)
        .layer(from_fn_with_state(state.clone(), middleware::guard))
        .with_state(state)
}
