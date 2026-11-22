//! Route handlers: parse the request (`dto`), call the engine on the blocking
//! pool, map errors (`error`).

use axum::Json;

use super::error::HttpError;

pub mod health;
pub mod not_found;
pub mod transcripts;

type ApiResult<T> = Result<Json<T>, HttpError>;

/// Runs blocking engine work on the blocking pool.
async fn blocking<T, F>(f: F) -> Result<T, HttpError>
where
    F: FnOnce() -> session_search_core::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(HttpError::internal)?
        .map_err(HttpError::from)
}
