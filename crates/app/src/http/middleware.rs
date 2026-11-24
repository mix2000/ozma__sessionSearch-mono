//! Request guard: Host/Origin checks for every request (DNS rebinding, CSRF),
//! no cross-site writes, and the optional bearer token for `/api`.

use std::net::Ipv4Addr;

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::error::HttpError;
use super::AppState;

fn err(status: u16, message: &str) -> Response {
    HttpError::new(status, message).into_response()
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
    // Any 127.0.0.0/8 literal: the server can be bound to each of them.
    matches!(name.to_ascii_lowercase().as_str(), "localhost" | "[::1]")
        || name.parse::<Ipv4Addr>().is_ok_and(|ip| ip.is_loopback())
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
    form_urlencoded::parse(query?.as_bytes())
        .find(|(k, _)| k == "token")
        .map(|(_, v)| v.into_owned())
}

/// Host / Origin checks for every request, bearer token for /api.
pub async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    if st.loopback_only {
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !is_loopback_host(host) {
            return err(403, "Forbidden host");
        }
        if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            if !origin_host(origin).is_some_and(is_loopback_host) {
                return err(403, "Forbidden origin");
            }
        }
    }
    let is_api = req.uri().path() == "/api" || req.uri().path().starts_with("/api/");
    if is_api && req.method() != Method::GET && req.method() != Method::HEAD {
        // No CORS: block cross-site form posts (simple requests skip preflight).
        if headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) == Some("cross-site") {
            return err(403, "Cross-site request");
        }
    }
    if is_api {
        if let Some(expected) = &st.token {
            let got = token_from(headers, req.uri().query());
            if !got.is_some_and(|g| constant_eq(g.as_bytes(), expected.as_bytes())) {
                return err(401, "Unauthorized");
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
            "127.0.0.2:4477",
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
            "127.evil.com",
            "128.0.0.1",
            "localhost.evil.com:80",
            "[::1]x",
            "10.0.0.1:4477",
        ] {
            assert!(!is_loopback_host(bad), "{bad}");
        }
        assert_eq!(origin_host("http://localhost:4477"), Some("localhost:4477"));
        assert_eq!(origin_host("null"), None);
    }

    #[test]
    fn query_token_is_form_decoded() {
        let t = |q: &str| token_from(&HeaderMap::new(), Some(q));
        assert_eq!(t("token=a%2Bb+c%zz%4").as_deref(), Some("a+b c%zz%4"));
        assert_eq!(t("x=1").as_deref(), None);
    }

    #[test]
    fn tokens_from_header_or_query() {
        let mut h = HeaderMap::new();
        assert_eq!(
            token_from(&h, Some("a=1&token=s3cr%2Bt")).as_deref(),
            Some("s3cr+t")
        );
        h.insert(header::AUTHORIZATION, "bearer  abc ".parse().unwrap());
        assert_eq!(token_from(&h, Some("token=x")).as_deref(), Some("abc"));
        assert_eq!(token_from(&HeaderMap::new(), None), None);
        assert!(constant_eq(b"abc", b"abc"));
        assert!(!constant_eq(b"abc", b"abd") && !constant_eq(b"ab", b"abc"));
    }
}
