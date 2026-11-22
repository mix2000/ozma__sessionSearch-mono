//! HTTP API (same paths and JSON as ozma Project Builder) + embedded UI.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{OriginalUri, Path, Query, Request, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use session_search_core::{ApiError, Engine, SearchParams};

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    /// Required bearer token for /api, if set.
    pub token: Option<String>,
    /// Reject requests whose Host/Origin is not loopback (DNS rebinding, CSRF).
    pub loopback_only: bool,
}

/// NestJS-style error body: `{statusCode, message, error}` (+ `command` for resume).
pub struct HttpError(pub ApiError);

impl From<ApiError> for HttpError {
    fn from(e: ApiError) -> Self {
        HttpError(e)
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.0.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        if status.is_server_error() {
            tracing::warn!("{}", self.0.message);
        }
        let mut body = json!({
            "statusCode": status.as_u16(),
            "message": self.0.message,
            "error": status.canonical_reason().unwrap_or("Error"),
        });
        if let Some(c) = self.0.command {
            body["command"] = json!(c);
        }
        (status, Json(body)).into_response()
    }
}

fn err(status: u16, message: impl Into<String>) -> HttpError {
    HttpError(ApiError::new(status, message))
}

type ApiResult<T> = Result<Json<T>, HttpError>;

/// Runs blocking engine work on the blocking pool.
async fn blocking<T, F>(f: F) -> Result<T, HttpError>
where
    F: FnOnce() -> Result<T, ApiError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| HttpError(ApiError::internal(e)))?
        .map_err(HttpError)
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/agents/transcripts/search", get(search))
        .route("/agents/transcripts/status", get(status))
        .route("/agents/transcripts/sync", post(sync))
        .route("/agents/transcripts/{id}/context", get(context))
        .route("/agents/transcripts/{id}/resume", post(resume))
        .fallback(api_not_found);
    Router::new()
        .nest("/api", api)
        .fallback(crate::assets::serve)
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") }))
}

async fn api_not_found(method: Method, OriginalUri(uri): OriginalUri) -> HttpError {
    err(404, format!("Cannot {method} {}", uri.path()))
}

fn flag(v: Option<&String>) -> bool {
    matches!(v.map(String::as_str), Some("true" | "1"))
}

async fn search(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<session_search_core::SearchResponse> {
    let params = SearchParams {
        q: q.get("q").cloned().unwrap_or_default(),
        provider: q.get("provider").cloned().filter(|p| !p.is_empty()),
        include_headless: flag(q.get("includeHeadless")),
        limit: q.get("limit").and_then(|l| l.parse().ok()),
    };
    let engine = st.engine.clone();
    blocking(move || engine.search(&params)).await.map(Json)
}

async fn status(State(st): State<AppState>) -> ApiResult<session_search_core::Status> {
    let engine = st.engine.clone();
    blocking(move || engine.status()).await.map(Json)
}

async fn sync(State(st): State<AppState>) -> ApiResult<session_search_core::SyncResult> {
    let engine = st.engine.clone();
    blocking(move || Ok(engine.sync())).await.map(Json)
}

async fn context(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Vec<session_search_core::ContextMessage>> {
    let seq: i64 = q
        .get("seq")
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| err(400, "Validation failed (numeric string is expected)"))?;
    let engine = st.engine.clone();
    blocking(move || engine.context(&id, seq)).await.map(Json)
}

async fn resume(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<session_search_core::ResumeResponse> {
    let engine = st.engine.clone();
    blocking(move || engine.resume(&id)).await.map(Json)
}

fn is_loopback_host(host: &str) -> bool {
    // Strip the port: "[::1]:4477", "127.0.0.1:4477", "localhost".
    let name = if let Some(rest) = host.strip_prefix('[') {
        match rest.split_once(']') {
            Some((h, tail)) if tail.is_empty() || tail.starts_with(':') => format!("[{h}]"),
            _ => return false,
        }
    } else {
        host.rsplit_once(':').map_or(host, |(h, _)| h).to_owned()
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "[::1]"
    )
}

fn origin_host(origin: &str) -> Option<&str> {
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))?;
    Some(rest.split('/').next().unwrap_or(rest))
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn token_from(headers: &HeaderMap, query: Option<&str>) -> Option<String> {
    if let Some(v) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(t) = v
            .strip_prefix("Bearer ")
            .or_else(|| v.strip_prefix("bearer "))
        {
            return Some(t.trim().to_owned());
        }
    }
    let q = query?;
    q.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == "token").then(|| percent_decode(v))
    })
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Host / Origin checks for every request, bearer token for /api.
async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    if st.loopback_only {
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !is_loopback_host(host) {
            return err(403, "Forbidden host").into_response();
        }
        if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            if !origin_host(origin).is_some_and(is_loopback_host) {
                return err(403, "Forbidden origin").into_response();
            }
        }
    }
    let is_api = req.uri().path() == "/api" || req.uri().path().starts_with("/api/");
    if is_api && req.method() != Method::GET && req.method() != Method::HEAD {
        // No CORS: block cross-site form posts (simple requests skip preflight).
        if headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) == Some("cross-site") {
            return err(403, "Cross-site request").into_response();
        }
    }
    if is_api {
        if let Some(expected) = &st.token {
            let got = token_from(headers, req.uri().query());
            if !got.is_some_and(|g| constant_eq(g.as_bytes(), expected.as_bytes())) {
                return err(401, "Unauthorized").into_response();
            }
        }
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts() {
        for ok in [
            "localhost",
            "localhost:4477",
            "127.0.0.1:1",
            "[::1]:4477",
            "[::1]",
            "LOCALHOST",
        ] {
            assert!(is_loopback_host(ok), "{ok}");
        }
        for bad in [
            "",
            "evil.com",
            "127.0.0.1.evil.com",
            "localhost.evil.com:80",
            "[::1]x",
            "10.0.0.1:4477",
        ] {
            assert!(!is_loopback_host(bad), "{bad}");
        }
        assert_eq!(origin_host("http://localhost:4477"), Some("localhost:4477"));
        assert_eq!(origin_host("null"), None);
    }

    use axum::body::Body;
    use http_body_util::BodyExt;
    use session_search_core::{EngineConfig, MacTerminal, Provider, Roots};
    use tower::ServiceExt;

    const ID: &str = "11111111-2222-3333-4444-555555555555";

    fn app(tmp: &std::path::Path, token: Option<&str>) -> Router {
        let project = tmp.join("claude/-w");
        std::fs::create_dir_all(&project).unwrap();
        let line = json!({
            "type": "user", "entrypoint": "cli", "cwd": tmp.to_string_lossy(),
            "timestamp": "2026-11-10T08:55:37.856Z",
            "message": { "content": "Почини ДЕПЛОЙ" },
        });
        std::fs::write(project.join(format!("{ID}.jsonl")), format!("{line}\n")).unwrap();
        let engine = Engine::open(EngineConfig {
            db_path: tmp.join("data/index.sqlite"),
            roots: Roots {
                claude: tmp.join("claude"),
                codex: tmp.join("codex"),
                grok: tmp.join("grok"),
            },
            providers: Provider::ALL.to_vec(),
            mac_terminal: MacTerminal::Terminal,
        })
        .unwrap()
        .with_launcher(Box::new(|_, _| Ok(())));
        engine.sync();
        router(AppState {
            engine: Arc::new(engine),
            token: token.map(str::to_owned),
            loopback_only: true,
        })
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        headers: &[(&str, &str)],
    ) -> (u16, serde_json::Value, String) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        req.headers_mut()
            .insert("host", "127.0.0.1:4477".parse().unwrap());
        for (k, v) in headers {
            let name = axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap();
            req.headers_mut().insert(name, v.parse().unwrap());
        }
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status().as_u16();
        let ct = res
            .headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap().to_owned())
            .unwrap_or_default();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json, ct)
    }

    #[tokio::test]
    async fn api_contract_and_guards() {
        let tmp = tempfile::tempdir().unwrap();
        let app = app(tmp.path(), None);

        let (s, v, _) = call(&app, "GET", "/api/health", &[]).await;
        assert_eq!((s, &v["ok"]), (200, &json!(true)));

        let (s, v, _) = call(
            &app,
            "GET",
            "/api/agents/transcripts/search?q=%D0%B4%D0%B5%D0%BF%D0%BB%D0%BE%D0%B9",
            &[],
        )
        .await;
        assert_eq!(s, 200);
        assert_eq!(v["terms"], json!(["деплой"]));
        assert_eq!(v["results"][0]["id"], ID);
        assert_eq!(
            v["results"][0]["hits"][0]["snippet"]["match"],
            "Почини ДЕПЛОЙ"
        );
        assert_eq!(v["truncated"], false);

        let (s, v, _) = call(&app, "GET", "/api/agents/transcripts/search?q=a", &[]).await;
        assert_eq!(s, 400);
        assert_eq!(
            v,
            json!({ "statusCode": 400, "message": "Введите хотя бы 2 символа", "error": "Bad Request" })
        );

        let (s, v, _) = call(&app, "GET", "/api/agents/transcripts/status", &[]).await;
        assert_eq!(s, 200);
        assert_eq!(
            (v["sessions"].as_i64(), v["messages"].as_i64()),
            (Some(1), Some(1))
        );
        assert!(v["index"].get("lastDurationMs").is_some());

        let (s, v, _) = call(&app, "POST", "/api/agents/transcripts/sync", &[]).await;
        assert_eq!((s, &v["updated"]), (200, &json!(0)));

        let (s, v, _) = call(
            &app,
            "GET",
            &format!("/api/agents/transcripts/{ID}/context?seq=0"),
            &[],
        )
        .await;
        assert_eq!(s, 200);
        assert_eq!(v[0]["text"], "Почини ДЕПЛОЙ");
        let (s, _, _) = call(
            &app,
            "GET",
            &format!("/api/agents/transcripts/{ID}/context?seq=x"),
            &[],
        )
        .await;
        assert_eq!(s, 400);

        let (s, v, _) = call(
            &app,
            "POST",
            &format!("/api/agents/transcripts/{ID}/resume"),
            &[("content-type", "application/json")],
        )
        .await;
        assert_eq!(s, 200, "{v}");
        assert_eq!(v["ok"], true);
        assert!(v["command"]
            .as_str()
            .unwrap()
            .ends_with(&format!("claude --resume {ID}")));
        let (s, _, _) = call(
            &app,
            "POST",
            "/api/agents/transcripts/00000000-0000-0000-0000-000000000000/resume",
            &[],
        )
        .await;
        assert_eq!(s, 404);
        let (s, _, _) = call(&app, "POST", "/api/agents/transcripts/bad%20id/resume", &[]).await;
        assert_eq!(s, 400);

        let (s, v, _) = call(&app, "GET", "/api/nope", &[]).await;
        assert_eq!((s, &v["statusCode"]), (404, &json!(404)));
        assert_eq!(v["message"], "Cannot GET /api/nope");

        // UI and SPA fallback.
        let (s, _, ct) = call(&app, "GET", "/", &[]).await;
        assert_eq!(s, 200);
        assert!(ct.starts_with("text/html"));
        let (s, _, ct) = call(&app, "GET", "/some/route", &[]).await;
        assert!(s == 200 && ct.starts_with("text/html"));
        let (s, _, _) = call(&app, "GET", "/assets/missing.js", &[]).await;
        assert_eq!(s, 404);

        // DNS rebinding / CSRF.
        let (s, _, _) = call(&app, "GET", "/api/health", &[("host", "evil.com")]).await;
        assert_eq!(s, 403);
        let (s, _, _) = call(
            &app,
            "POST",
            "/api/agents/transcripts/sync",
            &[("origin", "https://evil.com")],
        )
        .await;
        assert_eq!(s, 403);
        let (s, _, _) = call(
            &app,
            "POST",
            "/api/agents/transcripts/sync",
            &[("sec-fetch-site", "cross-site")],
        )
        .await;
        assert_eq!(s, 403);
        let (s, _, _) = call(
            &app,
            "POST",
            "/api/agents/transcripts/sync",
            &[("origin", "http://localhost:4477")],
        )
        .await;
        assert_eq!(s, 200);
    }

    #[tokio::test]
    async fn token_required_when_set() {
        let tmp = tempfile::tempdir().unwrap();
        let app = app(tmp.path(), Some("s3cr+t"));
        assert_eq!(
            call(&app, "GET", "/api/agents/transcripts/status", &[])
                .await
                .0,
            401
        );
        assert_eq!(
            call(
                &app,
                "GET",
                "/api/agents/transcripts/status",
                &[("authorization", "Bearer wrong")]
            )
            .await
            .0,
            401
        );
        assert_eq!(
            call(
                &app,
                "GET",
                "/api/agents/transcripts/status",
                &[("authorization", "Bearer s3cr+t")]
            )
            .await
            .0,
            200
        );
        assert_eq!(
            call(
                &app,
                "GET",
                "/api/agents/transcripts/status?token=s3cr%2Bt",
                &[]
            )
            .await
            .0,
            200
        );
        // The UI itself loads without a token.
        assert_eq!(call(&app, "GET", "/", &[]).await.0, 200);
    }

    #[test]
    fn decode() {
        assert_eq!(percent_decode("a%2Bb+c%zz%4"), "a+b c%zz%4");
    }
}
