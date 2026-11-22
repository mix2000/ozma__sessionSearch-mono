//! The web UI (`web/dist`) embedded into the binary, with SPA fallback.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../web/dist"]
#[allow_missing = true]
struct Assets;

const PLACEHOLDER: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Session Search</title></head>
<body style="font-family:system-ui,sans-serif;max-width:40rem;margin:3rem auto;padding:0 1rem">
<h1>ozma Session Search</h1>
<p>The web UI was not built into this binary. Build it with <code>npm run build</code> in <code>web/</code> and rebuild.</p>
<p>The API is available: <a href="/api/agents/transcripts/status">/api/agents/transcripts/status</a></p>
</body></html>"#;

fn file_response(path: &str, data: std::borrow::Cow<'static, [u8]>) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mut res = Response::new(Body::from(data.into_owned()));
    let headers = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(mime.as_ref()) {
        headers.insert(header::CONTENT_TYPE, v);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    res
}

fn index() -> Response {
    match Assets::get("index.html") {
        Some(f) => file_response("index.html", f.data),
        None => {
            let mut res = file_response("index.html", PLACEHOLDER.as_bytes().into());
            *res.status_mut() = StatusCode::OK;
            res
        }
    }
}

/// Static file or `index.html` for client-side routes.
pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.is_empty() || path == "index.html" {
        return index();
    }
    if let Some(f) = Assets::get(path) {
        return file_response(path, f.data);
    }
    // Missing asset files are a real 404; everything else is an SPA route.
    let last = path.rsplit('/').next().unwrap_or("");
    if path.starts_with("assets/") || last.contains('.') {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }
    index()
}

/// Whether a real UI build is embedded.
pub fn has_ui() -> bool {
    Assets::get("index.html").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Uri;

    async fn get(path: &str) -> Response {
        serve(path.parse::<Uri>().unwrap()).await
    }

    #[tokio::test]
    async fn index_spa_fallback_and_missing_assets() {
        for path in ["/", "/index.html", "/some/route"] {
            let res = get(path).await;
            assert_eq!(res.status(), StatusCode::OK, "{path}");
            assert!(res.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/html"));
            assert_eq!(res.headers()[header::CACHE_CONTROL], "no-cache");
            assert_eq!(res.headers()["x-frame-options"], "DENY");
        }
        assert_eq!(
            get("/assets/missing.js").await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get("/favicon-missing.ico").await.status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn immutable_cache_for_hashed_assets() {
        let res = file_response("assets/app.js", b"x".as_slice().into());
        assert_eq!(
            res.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        assert!(res.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .contains("javascript"));
    }
}
