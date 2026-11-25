//! HTTP API through the whole router (`tower::ServiceExt::oneshot`) on a
//! temporary index: status codes, JSON shapes, Host/Origin guards, token.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::Router;
use http_body_util::BodyExt;
use serde_json::json;
use session_search_core::{Engine, EngineConfig, MacTerminal, Provider, Roots};
use tower::ServiceExt;

use session_search_app::http::{router, AppState};

const ID: &str = "11111111-2222-3333-4444-555555555555";

/// A router over a temporary index with one Claude conversation.
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

fn keys(v: &serde_json::Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

/// A fresh app without a token.
fn open_app() -> (tempfile::TempDir, Router) {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path(), None);
    (tmp, app)
}

const SEARCH_DEPLOY: &str = "/api/agents/transcripts/search?q=%D0%B4%D0%B5%D0%BF%D0%BB%D0%BE%D0%B9";

#[tokio::test]
async fn health_and_unknown_api_routes() {
    let (_t, app) = open_app();
    let (s, v, _) = call(&app, "GET", "/api/health", &[]).await;
    assert_eq!((s, &v["ok"]), (200, &json!(true)));
    let (s, v, _) = call(&app, "GET", "/api/nope", &[]).await;
    assert_eq!((s, &v["statusCode"]), (404, &json!(404)));
    assert_eq!(v["message"], "Cannot GET /api/nope");
    let (s, _, _) = call(&app, "DELETE", "/api/agents/transcripts/status", &[]).await;
    assert_eq!(s, 405);
}

#[tokio::test]
async fn search_finds_and_validates() {
    let (_t, app) = open_app();
    let (s, v, ct) = call(&app, "GET", SEARCH_DEPLOY, &[]).await;
    assert_eq!(s, 200);
    assert!(ct.starts_with("application/json"));
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
}

#[tokio::test]
async fn search_json_shape() {
    let (_t, app) = open_app();
    let (_, v, _) = call(&app, "GET", SEARCH_DEPLOY, &[]).await;
    assert_eq!(keys(&v), ["results", "terms", "truncated"]);
    let r = &v["results"][0];
    assert_eq!(
        keys(r),
        [
            "gitBranch",
            "hits",
            "id",
            "kind",
            "lastAt",
            "matches",
            "messageCount",
            "provider",
            "resumeCommand",
            "resumeNeedsFolder",
            "startedAt",
            "title",
            "workdir",
            "workdirExists"
        ]
    );
    assert_eq!(
        keys(&r["hits"][0]),
        ["at", "messageId", "role", "seq", "snippet"]
    );
    assert_eq!(keys(&r["hits"][0]["snippet"]), ["match"]);
}

#[tokio::test]
async fn status_and_sync() {
    let (_t, app) = open_app();
    let (s, v, _) = call(&app, "GET", "/api/agents/transcripts/status", &[]).await;
    assert_eq!(s, 200);
    assert_eq!(
        keys(&v),
        [
            "index",
            "messages",
            "minQueryLength",
            "providers",
            "sessions"
        ]
    );
    assert_eq!(
        (v["sessions"].as_i64(), v["messages"].as_i64()),
        (Some(1), Some(1))
    );
    assert_eq!(v["minQueryLength"], 2);
    assert_eq!(
        keys(&v["index"]),
        ["lastDurationMs", "lastError", "lastRunAt", "running"]
    );
    assert_eq!(
        keys(&v["providers"][0]),
        ["exists", "provider", "root", "sessions"]
    );
    assert_eq!(v["providers"].as_array().unwrap().len(), 3);
    let (s, v, _) = call(&app, "POST", "/api/agents/transcripts/sync", &[]).await;
    assert_eq!((s, &v["updated"]), (200, &json!(0)));
    assert_eq!(keys(&v), ["files", "messages", "updated"]);
}

#[tokio::test]
async fn context_requires_a_numeric_seq() {
    let (_t, app) = open_app();
    let uri = format!("/api/agents/transcripts/{ID}/context");
    let (s, v, _) = call(&app, "GET", &format!("{uri}?seq=0"), &[]).await;
    assert_eq!(s, 200);
    assert_eq!(v[0]["text"], "Почини ДЕПЛОЙ");
    assert_eq!(keys(&v[0]), ["at", "id", "role", "seq", "text"]);
    let bad = json!({"statusCode": 400, "message": "Validation failed (numeric string is expected)", "error": "Bad Request"});
    for q in ["?seq=x", ""] {
        let (s, v, _) = call(&app, "GET", &format!("{uri}{q}"), &[]).await;
        assert_eq!((s, v), (400, bad.clone()), "{q}");
    }
}

#[tokio::test]
async fn resume_success_and_errors() {
    let (_t, app) = open_app();
    let json_ct = [("content-type", "application/json")];
    let (s, v, _) = call(
        &app,
        "POST",
        &format!("/api/agents/transcripts/{ID}/resume"),
        &json_ct,
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["ok"], true);
    assert!(v["command"]
        .as_str()
        .unwrap()
        .contains(&format!("claude --resume {ID}")));
    let missing = "/api/agents/transcripts/00000000-0000-0000-0000-000000000000/resume";
    assert_eq!(call(&app, "POST", missing, &[]).await.0, 404);
    let bad = "/api/agents/transcripts/bad%20id/resume";
    assert_eq!(call(&app, "POST", bad, &[]).await.0, 400);
}

#[tokio::test]
async fn resume_conflict_carries_the_command() {
    let (tmp, app) = open_app();
    // The transcript file is gone: the text stays searchable, resume is refused.
    std::fs::remove_file(tmp.path().join(format!("claude/-w/{ID}.jsonl"))).unwrap();
    let (s, v, _) = call(
        &app,
        "POST",
        &format!("/api/agents/transcripts/{ID}/resume"),
        &[],
    )
    .await;
    assert_eq!(s, 409);
    assert_eq!(v["error"], "Conflict");
    assert!(v["command"]
        .as_str()
        .unwrap()
        .contains(&format!("claude --resume {ID}")));
}

#[tokio::test]
async fn ui_and_spa_fallback() {
    let (_t, app) = open_app();
    for path in ["/", "/some/route"] {
        let (s, _, ct) = call(&app, "GET", path, &[]).await;
        assert!(s == 200 && ct.starts_with("text/html"), "{path}");
    }
    assert_eq!(call(&app, "GET", "/assets/missing.js", &[]).await.0, 404);
}

#[tokio::test]
async fn foreign_host_and_origin_are_forbidden() {
    let (_t, app) = open_app();
    let sync = "/api/agents/transcripts/sync";
    let (s, v, _) = call(&app, "GET", "/api/health", &[("host", "evil.com")]).await;
    assert_eq!((s, &v["message"]), (403, &json!("Forbidden host")));
    let (s, v, _) = call(&app, "GET", "/", &[("host", "127.0.0.1.evil.com")]).await;
    assert_eq!((s, &v["error"]), (403, &json!("Forbidden")));
    let (s, v, _) = call(&app, "POST", sync, &[("origin", "https://evil.com")]).await;
    assert_eq!((s, &v["message"]), (403, &json!("Forbidden origin")));
    let (s, v, _) = call(&app, "POST", sync, &[("sec-fetch-site", "cross-site")]).await;
    assert_eq!((s, &v["message"]), (403, &json!("Cross-site request")));
    let (s, _, _) = call(&app, "POST", sync, &[("origin", "http://localhost:4477")]).await;
    assert_eq!(s, 200);
    let (s, _, _) = call(&app, "GET", "/api/health", &[("host", "[::1]:4477")]).await;
    assert_eq!(s, 200);
}

#[tokio::test]
async fn token_required_when_set() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path(), Some("s3cr+t"));
    let status = "/api/agents/transcripts/status";
    let (s, v, _) = call(&app, "GET", status, &[]).await;
    assert_eq!((s, &v["message"]), (401, &json!("Unauthorized")));
    assert_eq!(
        call(&app, "GET", status, &[("authorization", "Bearer wrong")])
            .await
            .0,
        401
    );
    assert_eq!(
        call(&app, "GET", status, &[("authorization", "Bearer s3cr+t")])
            .await
            .0,
        200
    );
    assert_eq!(
        call(&app, "GET", &format!("{status}?token=s3cr%2Bt"), &[])
            .await
            .0,
        200
    );
    assert_eq!(
        call(&app, "GET", &format!("{status}?token=s3cr+t"), &[])
            .await
            .0,
        401,
        "+ is a space"
    );
    // The UI itself loads without a token.
    assert_eq!(call(&app, "GET", "/", &[]).await.0, 200);
}
