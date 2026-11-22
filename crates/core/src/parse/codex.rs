//! Codex CLI transcripts: `<CODEX_HOME>/sessions/YYYY/MM/DD/rollout-<date>-<uuid>.jsonl`.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::{str_field, to_date, ParsedLine, ParsedMessage, ParsedMeta, Role};

pub fn line_wanted(line: &[u8]) -> bool {
    use memchr::memmem::find;
    find(line, br#""type":"session_meta""#).is_some()
        || (find(line, br#""type":"response_item""#).is_some()
            && find(line, br#""type":"message""#).is_some())
}

/// Codex injections into "user messages": AGENTS.md, environment, plugins.
fn is_injected(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with('<') || t.starts_with("# AGENTS.md")
}

pub fn parse_line(line: &str) -> Vec<ParsedLine> {
    let Ok(d) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    let empty = Value::Object(Default::default());
    let p = d.get("payload").filter(|p| !p.is_null()).unwrap_or(&empty);
    let ty = d.get("type").and_then(Value::as_str);
    if ty == Some("session_meta") {
        let source = p.get("source").and_then(Value::as_str);
        let ts = p
            .get("timestamp")
            .filter(|v| !v.is_null())
            .or(d.get("timestamp"));
        return vec![ParsedLine::Meta(ParsedMeta {
            id: str_field(p, "id"),
            cwd: str_field(p, "cwd"),
            kind: Some(
                if source == Some("cli") {
                    "interactive"
                } else {
                    "headless"
                }
                .to_owned(),
            ),
            git_branch: p.get("git").and_then(|g| str_field(g, "branch")),
            title: None,
            at: to_date(ts),
        })];
    }
    if ty != Some("response_item") || p.get("type").and_then(Value::as_str) != Some("message") {
        return vec![];
    }
    let role = match p.get("role").and_then(Value::as_str) {
        Some("user") => Role::User,
        Some("assistant") => Role::Assistant,
        _ => return vec![],
    };
    let text = p
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| {
                    matches!(
                        b.get("type").and_then(Value::as_str),
                        Some("input_text" | "output_text")
                    )
                })
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .filter(|t| !(role == Role::User && is_injected(t)))
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_default();
    let text = text.trim();
    if text.is_empty() {
        return vec![];
    }
    vec![ParsedLine::Message(ParsedMessage {
        role,
        at: to_date(d.get("timestamp")),
        text: text.to_owned(),
        prompt_index: None,
    })]
}

static ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\.jsonl$")
        .expect("valid regex")
});

/// Conversation id from the file name `rollout-<date>-<uuid>.jsonl`.
pub fn id_from_file(file: &str) -> Option<String> {
    ID_RE.captures(file).map(|c| c[1].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn meta_and_messages_skip_environment_injections() {
        let meta = json!({
            "type": "session_meta",
            "payload": { "id": "abc", "cwd": "/w", "source": "cli", "git": { "branch": "dev" } },
        });
        match &parse_line(&meta.to_string())[0] {
            ParsedLine::Meta(m) => {
                assert_eq!(m.id.as_deref(), Some("abc"));
                assert_eq!(m.cwd.as_deref(), Some("/w"));
                assert_eq!(m.kind.as_deref(), Some("interactive"));
                assert_eq!(m.git_branch.as_deref(), Some("dev"));
            }
            other => panic!("unexpected {other:?}"),
        }
        let user = json!({
            "type": "response_item",
            "timestamp": "2026-09-14T00:05:11.483Z",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [
                    { "type": "input_text", "text": "<environment_context>…" },
                    { "type": "input_text", "text": "проверь сборку" },
                ],
            },
        });
        assert_eq!(
            parse_line(&user.to_string()),
            vec![ParsedLine::Message(ParsedMessage {
                role: Role::User,
                at: Some("2026-09-14T00:05:11.483Z".parse().unwrap()),
                text: "проверь сборку".into(),
                prompt_index: None,
            })]
        );
        assert_eq!(
            id_from_file("rollout-2026-09-14T03-05-07-01a09d3b-58db-7550-bf6b-93f273e29e1a.jsonl")
                .as_deref(),
            Some("01a09d3b-58db-7550-bf6b-93f273e29e1a")
        );
    }

    #[test]
    fn exec_source_is_headless_and_agents_md_skipped() {
        let meta = json!({ "type": "session_meta", "payload": { "id": "x", "source": "exec" } });
        match &parse_line(&meta.to_string())[0] {
            ParsedLine::Meta(m) => assert_eq!(m.kind.as_deref(), Some("headless")),
            other => panic!("unexpected {other:?}"),
        }
        let user = json!({
            "type": "response_item",
            "payload": { "type": "message", "role": "user",
                "content": [{ "type": "input_text", "text": "# AGENTS.md instructions" }] },
        });
        assert_eq!(parse_line(&user.to_string()), vec![]);
        let dev = json!({
            "type": "response_item",
            "payload": { "type": "message", "role": "developer",
                "content": [{ "type": "input_text", "text": "x" }] },
        });
        assert_eq!(parse_line(&dev.to_string()), vec![]);
        // Assistant text starting with "<" is kept.
        let answer = json!({
            "type": "response_item",
            "payload": { "type": "message", "role": "assistant",
                "content": [{ "type": "output_text", "text": "<b>ok</b>" }] },
        });
        assert_eq!(parse_line(&answer.to_string()).len(), 1);
    }

    #[test]
    fn wanted_filter() {
        assert!(line_wanted(br#"{"type":"session_meta","payload":{}}"#));
        assert!(line_wanted(
            br#"{"type":"response_item","payload":{"type":"message"}}"#
        ));
        assert!(!line_wanted(
            br#"{"type":"response_item","payload":{"type":"function_call"}}"#
        ));
    }
}
