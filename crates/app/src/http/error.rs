//! The single place where errors become HTTP responses: NestJS-style body
//! `{statusCode, message, error}` (+ `command` for resume).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpError {
    pub status: u16,
    pub message: String,
    /// For resume errors: the command to run by hand.
    pub command: Option<String>,
}

impl HttpError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        HttpError {
            status,
            message: message.into(),
            command: None,
        }
    }

    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self::new(500, e.to_string())
    }
}

impl From<session_search_core::Error> for HttpError {
    fn from(e: session_search_core::Error) -> Self {
        HttpError {
            status: e.status(),
            message: e.message().to_owned(),
            command: e.command().map(str::to_owned),
        }
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        if status.is_server_error() {
            tracing::warn!("{}", self.message);
        }
        let mut body = json!({
            "statusCode": status.as_u16(),
            "message": self.message,
            "error": status.canonical_reason().unwrap_or("Error"),
        });
        if let Some(c) = self.command {
            body["command"] = json!(c);
        }
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    async fn body(e: HttpError) -> (u16, serde_json::Value) {
        let res = e.into_response();
        let status = res.status().as_u16();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn renders_nest_style_body() {
        let core = session_search_core::Error::conflict("busy", "cd /w && x");
        assert_eq!(
            body(core.into()).await,
            (
                409,
                json!({"statusCode": 409, "message": "busy", "error": "Conflict", "command": "cd /w && x"})
            )
        );
        assert_eq!(
            body(HttpError::new(403, "Forbidden host")).await,
            (
                403,
                json!({"statusCode": 403, "message": "Forbidden host", "error": "Forbidden"})
            )
        );
        assert_eq!(body(HttpError::new(1000, "x")).await.0, 500);
    }
}
