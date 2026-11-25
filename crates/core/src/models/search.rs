//! Search, context and resume API responses (JSON contract of the UI).

use serde::Serialize;

/// Lowercase used both for the stored search copy and for query terms.
pub fn fold(s: &str) -> String {
    s.to_lowercase()
}

#[derive(Debug, Clone, Default)]
pub struct SearchParams {
    pub q: String,
    pub provider: Option<String>,
    pub include_headless: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LiveState {
    External { pid: u32 },
}

/// The paragraph with a match and its neighbours.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snippet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(rename = "match")]
    pub matched: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptHit {
    pub message_id: i64,
    pub seq: i64,
    pub role: String,
    pub at: Option<String>,
    pub snippet: Snippet,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptResult {
    pub id: String,
    pub provider: String,
    pub title: Option<String>,
    pub workdir: Option<String>,
    pub kind: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<String>,
    pub last_at: Option<String>,
    pub message_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<LiveState>,
    pub workdir_exists: bool,
    /// Command to continue the conversation in one's own terminal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_command: Option<String>,
    /// The command leaves out the (existing) folder: it must be run there.
    pub resume_needs_folder: bool,
    pub matches: usize,
    pub hits: Vec<TranscriptHit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub terms: Vec<String>,
    pub results: Vec<TranscriptResult>,
    pub truncated: bool,
}

/// A message around a hit (full text).
#[derive(Debug, Clone, Serialize)]
pub struct ContextMessage {
    pub id: i64,
    pub seq: i64,
    pub role: String,
    pub at: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeResponse {
    pub ok: bool,
    pub command: String,
    pub workdir: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_serializes_like_original() {
        let v = serde_json::to_value(Snippet {
            before: None,
            matched: "m".into(),
            after: None,
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({ "match": "m" }));
    }

    #[test]
    fn live_state_is_tagged() {
        let v = serde_json::to_value(LiveState::External { pid: 7 }).unwrap();
        assert_eq!(v, serde_json::json!({ "kind": "external", "pid": 7 }));
    }
}
