//! Claude Code transcripts: `<projects>/<folder>/<uuid>.jsonl`.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::{str_field, to_date, truthy, ParsedLine, ParsedMessage, ParsedMeta, Role};

static SERVICE_BLOCKS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?s)<system-reminder>.*?</system-reminder>|<local-command-caveat>.*?</local-command-caveat>",
    )
    .expect("valid regex")
});

/// Service injections of Claude Code inside user messages.
fn clean_text(text: &str) -> String {
    SERVICE_BLOCKS.replace_all(text, "").trim().to_owned()
}

/// Fast pre-filter: lines that are surely not needed are not parsed.
pub fn line_wanted(line: &[u8]) -> bool {
    use memchr::memmem::find;
    find(line, br#""type":"user""#).is_some()
        || find(line, br#""type":"assistant""#).is_some()
        || find(line, br#""type":"ai-title""#).is_some()
        || find(line, br#""type":"summary""#).is_some()
}

pub fn parse_line(line: &str) -> Vec<ParsedLine> {
    let Ok(d) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    let ty = d.get("type").and_then(Value::as_str).unwrap_or("");
    if ty == "ai-title" {
        if let Some(t) = str_field(&d, "aiTitle") {
            return vec![ParsedLine::Meta(ParsedMeta {
                title: Some(t),
                ..Default::default()
            })];
        }
        return vec![];
    }
    if ty == "summary" {
        if let Some(t) = str_field(&d, "summary") {
            return vec![ParsedLine::Meta(ParsedMeta {
                title: Some(t),
                ..Default::default()
            })];
        }
        return vec![];
    }
    let role = match ty {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        _ => return vec![],
    };
    // isCompactSummary: retelling of the conversation after context compaction —
    // duplicates already indexed messages.
    if truthy(d.get("isMeta")) || truthy(d.get("isSidechain")) || truthy(d.get("isCompactSummary"))
    {
        return vec![];
    }
    let at = to_date(d.get("timestamp"));
    let mut out = Vec::new();
    if role == Role::User {
        out.push(ParsedLine::Meta(ParsedMeta {
            id: None,
            cwd: str_field(&d, "cwd"),
            kind: d.get("entrypoint").and_then(Value::as_str).map(|e| {
                if e == "cli" {
                    "interactive".to_owned()
                } else {
                    "headless".to_owned()
                }
            }),
            git_branch: str_field(&d, "gitBranch"),
            title: None,
            at,
        }));
    }
    let content = d.get("message").and_then(|m| m.get("content"));
    let text = match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    };
    let text = clean_text(&text);
    if !text.is_empty() {
        out.push(ParsedLine::Message(ParsedMessage {
            role,
            at,
            text,
            prompt_index: None,
        }));
    }
    out
}

/// Session id from a Claude transcript file name (`<uuid>.jsonl`).
pub fn id_from_file(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".jsonl")?;
    (stem.len() == 36 && stem.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'))
        .then(|| stem.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn date(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        Some(s.parse().unwrap())
    }

    #[test]
    fn user_message_without_system_reminder_with_meta() {
        let line = json!({
            "type": "user",
            "entrypoint": "cli",
            "cwd": "/w",
            "gitBranch": "master",
            "timestamp": "2026-11-10T08:55:37.856Z",
            "message": {
                "role": "user",
                "content": "почини деплой<system-reminder>секрет</system-reminder>",
            },
        })
        .to_string();
        assert_eq!(
            parse_line(&line),
            vec![
                ParsedLine::Meta(ParsedMeta {
                    cwd: Some("/w".into()),
                    kind: Some("interactive".into()),
                    git_branch: Some("master".into()),
                    at: date("2026-11-10T08:55:37.856Z"),
                    ..Default::default()
                }),
                ParsedLine::Message(ParsedMessage {
                    role: Role::User,
                    at: date("2026-11-10T08:55:37.856Z"),
                    text: "почини деплой".into(),
                    prompt_index: None,
                }),
            ]
        );
    }

    #[test]
    fn assistant_only_text_blocks_no_tool_output() {
        let answer = json!({
            "type": "assistant",
            "message": { "content": [
                { "type": "thinking", "thinking": "x" },
                { "type": "text", "text": "Готово" },
                { "type": "tool_use", "name": "Bash" },
            ]},
        })
        .to_string();
        assert_eq!(
            parse_line(&answer),
            vec![ParsedLine::Message(ParsedMessage {
                role: Role::Assistant,
                at: None,
                text: "Готово".into(),
                prompt_index: None,
            })]
        );
        let tool_result = json!({
            "type": "user",
            "message": { "content": [{ "type": "tool_result", "content": "много" }] },
        })
        .to_string();
        assert!(parse_line(&tool_result)
            .iter()
            .all(|p| !matches!(p, ParsedLine::Message(_))));
        let meta = json!({ "type": "user", "isMeta": true, "message": { "content": "x" } });
        assert_eq!(parse_line(&meta.to_string()), vec![]);
        let title = json!({ "type": "ai-title", "aiTitle": "Заголовок" });
        assert_eq!(
            parse_line(&title.to_string()),
            vec![ParsedLine::Meta(ParsedMeta {
                title: Some("Заголовок".into()),
                ..Default::default()
            })]
        );
    }

    #[test]
    fn compact_summary_and_sidechain_not_indexed() {
        let line = json!({
            "type": "user",
            "isCompactSummary": true,
            "message": { "content": "This session is being continued…" },
        });
        assert_eq!(parse_line(&line.to_string()), vec![]);
        let side =
            json!({ "type": "assistant", "isSidechain": true, "message": { "content": "x" } });
        assert_eq!(parse_line(&side.to_string()), vec![]);
    }

    #[test]
    fn headless_entrypoint_and_local_command_caveat() {
        let line = json!({
            "type": "user",
            "entrypoint": "sdk-cli",
            "message": { "content": "<local-command-caveat>c</local-command-caveat>  " },
        });
        let parsed = parse_line(&line.to_string());
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            ParsedLine::Meta(m) => assert_eq!(m.kind.as_deref(), Some("headless")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn wanted_filter_and_bad_json() {
        assert!(line_wanted(br#"{"type":"user"}"#));
        assert!(!line_wanted(br#"{"type":"file-history-snapshot"}"#));
        assert_eq!(parse_line("{not json"), vec![]);
    }

    #[test]
    fn id_from_file_name() {
        assert_eq!(
            id_from_file("0a1b2c3d-1111-2222-3333-444455556666.jsonl").as_deref(),
            Some("0a1b2c3d-1111-2222-3333-444455556666")
        );
        assert_eq!(id_from_file("agent-123.jsonl"), None);
    }
}
