//! Request query parameters → core inputs; response DTOs are the core models.

use std::collections::HashMap;

use serde::Serialize;
use session_search_core::SearchParams;

use super::error::HttpError;

/// Raw query string parameters (the last value of a repeated key wins).
pub type QueryMap = HashMap<String, String>;

/// `true` / `1` turn a flag on.
fn flag(v: Option<&String>) -> bool {
    matches!(v.map(String::as_str), Some("true" | "1"))
}

/// `GET /agents/transcripts/search?q=&provider=&includeHeadless=&limit=`.
pub fn search_params(q: &QueryMap) -> SearchParams {
    SearchParams {
        q: q.get("q").cloned().unwrap_or_default(),
        provider: q.get("provider").cloned().filter(|p| !p.is_empty()),
        include_headless: flag(q.get("includeHeadless")),
        limit: q.get("limit").and_then(|l| l.parse().ok()),
    }
}

/// `GET /agents/transcripts/{id}/context?seq=` — `seq` must be a number.
pub fn context_seq(q: &QueryMap) -> Result<i64, HttpError> {
    q.get("seq")
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| HttpError::new(400, "Validation failed (numeric string is expected)"))
}

/// `GET /health`.
#[derive(Debug, Serialize)]
pub struct Health {
    pub ok: bool,
    pub version: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(pairs: &[(&str, &str)]) -> QueryMap {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn search_query() {
        let p = search_params(&q(&[
            ("q", "nginx"),
            ("provider", ""),
            ("includeHeadless", "1"),
            ("limit", "x"),
        ]));
        assert_eq!(p.q, "nginx");
        assert_eq!(p.provider, None);
        assert!(p.include_headless);
        assert_eq!(p.limit, None);
        let p = search_params(&q(&[("includeHeadless", "yes"), ("limit", "5")]));
        assert!(!p.include_headless);
        assert_eq!((p.q.as_str(), p.limit), ("", Some(5)));
    }

    #[test]
    fn context_query() {
        assert_eq!(context_seq(&q(&[("seq", " 7 ")])), Ok(7));
        assert_eq!(context_seq(&q(&[("seq", "x")])).unwrap_err().status, 400);
        assert!(context_seq(&q(&[])).is_err());
    }
}
