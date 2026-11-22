//! Grok Build `summary.json` (session metadata) and the work directory encoded
//! in a session group folder name.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::models::{ParsedLine, ParsedMeta};
use crate::sources::json::to_date;

/// What we need from a Grok Build `summary.json`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub id: Option<String>,
    pub cwd: Option<String>,
    pub title: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub last_at: Option<DateTime<Utc>>,
    pub branch: Option<String>,
    pub session_kind: Option<String>,
    pub hidden: Option<bool>,
}

fn non_empty(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub fn parse_summary(json: &str) -> Option<Summary> {
    let d: Value = serde_json::from_str(json).ok()?;
    let info = d.get("info");
    Some(Summary {
        id: non_empty(info.and_then(|i| i.get("id"))),
        cwd: non_empty(info.and_then(|i| i.get("cwd"))),
        title: non_empty(d.get("generated_title")).or_else(|| non_empty(d.get("session_summary"))),
        created_at: to_date(d.get("created_at")),
        last_at: to_date(d.get("last_active_at")).or_else(|| to_date(d.get("updated_at"))),
        branch: non_empty(d.get("head_branch")),
        session_kind: non_empty(d.get("session_kind")),
        hidden: d.get("hidden").and_then(Value::as_bool),
    })
}

impl Summary {
    /// Hidden sessions and subagent sessions are not indexed.
    pub fn skipped(&self) -> bool {
        match self.hidden {
            Some(h) => h,
            None => self
                .session_kind
                .as_deref()
                .is_some_and(|k| k.starts_with("subagent")),
        }
    }

    pub fn kind(&self) -> &'static str {
        if self.session_kind.as_deref() == Some("headless") {
            "headless"
        } else {
            "interactive"
        }
    }

    /// Metadata records fed to the indexer before the transcript lines.
    pub fn meta(&self, id: &str, fallback_cwd: Option<String>) -> Vec<ParsedLine> {
        let mut out = vec![ParsedLine::Meta(ParsedMeta {
            id: Some(id.to_owned()),
            cwd: self.cwd.clone().or(fallback_cwd),
            kind: Some(self.kind().to_owned()),
            git_branch: self.branch.clone(),
            title: self.title.clone(),
            at: self.created_at,
        })];
        if self.last_at.is_some() {
            out.push(ParsedLine::Meta(ParsedMeta {
                at: self.last_at,
                ..Default::default()
            }));
        }
        out
    }
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = b.get(i + 1..i + 3)?;
            let v = u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The work directory encoded in a session group folder name
/// (`%2FUsers%2Fa%2Fp`, `C%3A%5Cproj`). `None` for hashed long names
/// (`<slug>-<16 hex>`), whose real path is in the `.cwd` file.
pub fn decode_cwd(name: &str) -> Option<String> {
    let d = percent_decode(name)?;
    let b = d.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    (d.starts_with('/') || drive).then_some(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_rules() {
        let s = parse_summary(
            r#"{"info":{"id":"019a","cwd":"/w"},"generated_title":"","session_summary":"Сводка",
                "created_at":"2026-11-01T10:00:00Z","last_active_at":"2026-11-02T10:00:00Z",
                "head_branch":"main","session_kind":"headless"}"#,
        )
        .unwrap();
        assert_eq!(s.title.as_deref(), Some("Сводка"));
        assert_eq!(s.kind(), "headless");
        assert!(!s.skipped());
        let sub = parse_summary(r#"{"session_kind":"subagent_fork"}"#).unwrap();
        assert!(sub.skipped());
        let shown = parse_summary(r#"{"session_kind":"subagent","hidden":false}"#).unwrap();
        assert!(!shown.skipped());
        assert!(parse_summary(r#"{"hidden":true}"#).unwrap().skipped());
        assert_eq!(
            parse_summary(r#"{"session_kind":"fork"}"#).unwrap().kind(),
            "interactive"
        );
    }

    #[test]
    fn cwd_decoding() {
        assert_eq!(
            decode_cwd("%2FUsers%2Fa%2Fp").as_deref(),
            Some("/Users/a/p")
        );
        assert_eq!(decode_cwd("C%3A%5Cproj%20x").as_deref(), Some(r"C:\proj x"));
        assert_eq!(decode_cwd("users-a-p-0123456789abcdef"), None);
        assert_eq!(decode_cwd("%ZZ"), None);
    }
}
