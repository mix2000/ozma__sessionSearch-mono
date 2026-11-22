//! Unknown `/api/*` routes, worded like NestJS.

use axum::extract::OriginalUri;
use axum::http::Method;

use crate::http::error::HttpError;

pub async fn api_not_found(method: Method, OriginalUri(uri): OriginalUri) -> HttpError {
    HttpError::new(404, format!("Cannot {method} {}", uri.path()))
}
