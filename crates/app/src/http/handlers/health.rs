//! `GET /api/health`.

use axum::Json;

use crate::http::dto::Health;

pub async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
    })
}
