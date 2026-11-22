//! Grok transcripts. Two independent sources under the grok root:
//!
//! * **Grok Build** (official xAI CLI): `<root>/sessions/<encoded cwd>/<session id>/`
//!   with `summary.json` (metadata) and `updates.jsonl` (append-only ACP
//!   `session/update` notifications; streamed chunks). `chat_history.jsonl` is
//!   not indexed.
//! * **grok-cli** (community `grok-dev`): one SQLite database `<root>/grok.db`,
//!   see [`crate::grok_db`]; [`cli_message_text`] extracts text from its rows.

use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

use super::{to_date, truthy, LineParser, ParsedLine, ParsedMessage, ParsedMeta, Role};

// ---------------------------------------------------------------- summary.json

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

// ---------------------------------------------------------------- updates.jsonl

fn unix_seconds(v: Option<&Value>) -> Option<DateTime<Utc>> {
    let v = v?;
    if let Some(f) = v.as_f64() {
        return Utc.timestamp_millis_opt((f * 1000.0) as i64).single();
    }
    to_date(Some(v))
}

#[derive(Debug)]
enum Group {
    User {
        prompt_index: Option<i64>,
        parts: Vec<String>,
        at: Option<DateTime<Utc>>,
        start: u64,
    },
    Agent {
        prompt_index: Option<i64>,
        text: String,
        at: Option<DateTime<Utc>>,
        start: u64,
    },
}

impl Group {
    fn into_message(self) -> Option<(ParsedMessage, u64)> {
        let (role, text, at, prompt_index, start) = match self {
            Group::User {
                prompt_index,
                parts,
                at,
                start,
            } => (Role::User, parts.join("\n\n"), at, prompt_index, start),
            Group::Agent {
                prompt_index,
                text,
                at,
                start,
            } => (Role::Assistant, text, at, prompt_index, start),
        };
        let text = text.trim();
        (!text.is_empty()).then(|| {
            (
                ParsedMessage {
                    role,
                    at,
                    text: text.to_owned(),
                    prompt_index,
                },
                start,
            )
        })
    }
}

/// Stateful parser of Grok Build `updates.jsonl`: joins consecutive user chunks
/// of one prompt and consecutive agent chunks of one reply.
#[derive(Debug, Default)]
pub struct UpdatesParser {
    open: Option<Group>,
    last_prompt: Option<i64>,
}

impl UpdatesParser {
    fn close(&mut self) -> Vec<ParsedLine> {
        self.open
            .take()
            .and_then(Group::into_message)
            .map(|(m, _)| ParsedLine::Message(m))
            .into_iter()
            .collect()
    }
}

/// Lines that can carry text or a rewind; everything else just ends a group.
fn maybe_relevant(line: &[u8]) -> bool {
    use memchr::memmem::find;
    find(line, b"_message_chunk").is_some()
        || find(line, b"agent_thought_chunk").is_some()
        || find(line, b"rewind_marker").is_some()
}

impl LineParser for UpdatesParser {
    fn feed(&mut self, line: &[u8], start: u64) -> Vec<ParsedLine> {
        if !maybe_relevant(line) {
            return self.close();
        }
        let Ok(d) = serde_json::from_slice::<Value>(line) else {
            return self.close();
        };
        let method = d.get("method").and_then(Value::as_str).unwrap_or("");
        let params = d.get("params");
        let update = params.and_then(|p| p.get("update"));
        let tag = update
            .and_then(|u| u.get("sessionUpdate"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let at = unix_seconds(d.get("timestamp"));
        let content = update.and_then(|u| u.get("content"));
        let content_meta = content.and_then(|c| c.get("_meta"));
        let update_meta = update.and_then(|u| u.get("_meta"));
        let is_text = content.and_then(|c| c.get("type")).and_then(Value::as_str) == Some("text");

        match (method, tag) {
            ("session/update", "agent_thought_chunk") => vec![],
            ("session/update", "user_message_chunk") => {
                if content_meta
                    .and_then(|m| m.get("bash_command"))
                    .is_some_and(|v| !v.is_null())
                    || truthy(update_meta.and_then(|m| m.get("hostTurn")))
                {
                    return self.close();
                }
                if !is_text {
                    // Images/resources belong to the prompt but carry no text.
                    return vec![];
                }
                let text = if truthy(content_meta.and_then(|m| m.get("interjection"))) {
                    content_meta
                        .and_then(|m| m.get("displayText"))
                        .and_then(Value::as_str)
                } else {
                    content.and_then(|c| c.get("text")).and_then(Value::as_str)
                }
                .unwrap_or("")
                .to_owned();
                let pi = update_meta
                    .and_then(|m| m.get("promptIndex"))
                    .and_then(Value::as_i64);
                if pi.is_some() {
                    self.last_prompt = pi;
                }
                if let Some(Group::User {
                    prompt_index,
                    parts,
                    ..
                }) = &mut self.open
                {
                    if *prompt_index == pi {
                        parts.push(text);
                        return vec![];
                    }
                }
                let out = self.close();
                self.open = Some(Group::User {
                    prompt_index: pi,
                    parts: vec![text],
                    at,
                    start,
                });
                out
            }
            ("session/update", "agent_message_chunk") => {
                if !is_text {
                    return vec![];
                }
                let chunk = content
                    .and_then(|c| c.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if let Some(Group::Agent { text, .. }) = &mut self.open {
                    text.push_str(chunk);
                    return vec![];
                }
                let out = self.close();
                self.open = Some(Group::Agent {
                    prompt_index: self.last_prompt,
                    text: chunk.to_owned(),
                    at,
                    start,
                });
                out
            }
            ("_x.ai/session/update", "rewind_marker") => {
                let mut out = self.close();
                let target = update
                    .and_then(|u| u.get("target_prompt_index"))
                    .or_else(|| update_meta.and_then(|m| m.get("target_prompt_index")))
                    .or_else(|| params.and_then(|p| p.get("target_prompt_index")))
                    .and_then(Value::as_i64);
                if let Some(t) = target {
                    out.push(ParsedLine::Rewind(t));
                    self.last_prompt = Some(t);
                }
                out
            }
            _ => self.close(),
        }
    }

    fn finish(&mut self) -> Option<(ParsedMessage, u64)> {
        self.open.take().and_then(Group::into_message)
    }

    fn resume_prompt(&mut self, prompt_index: i64) {
        self.last_prompt = Some(prompt_index);
    }
}

// ---------------------------------------------------------------- grok-cli rows

/// Text of a grok-cli message row (Vercel AI SDK `ModelMessage` JSON): user
/// text parts and assistant `text` parts; system/tool/reasoning are skipped.
pub fn cli_message_text(role: &str, message_json: &str) -> Option<(Role, String)> {
    let role = match role {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        _ => return None,
    };
    let v: Value = serde_json::from_str(message_json).ok()?;
    let content = v.get("content").unwrap_or(&v);
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|p| p.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => return None,
    };
    let text = text.trim();
    (!text.is_empty()).then(|| (role, text.to_owned()))
}

/// Timestamps in grok.db: ISO text, SQLite `YYYY-MM-DD HH:MM:SS`, or unix s/ms.
pub fn cli_time(v: &rusqlite::types::Value) -> Option<DateTime<Utc>> {
    use rusqlite::types::Value as V;
    match v {
        V::Integer(n) => {
            let ms = if n.abs() > 100_000_000_000 {
                *n
            } else {
                n * 1000
            };
            Utc.timestamp_millis_opt(ms).single()
        }
        V::Real(f) => Utc.timestamp_millis_opt((f * 1000.0) as i64).single(),
        V::Text(s) => to_date(Some(&Value::String(s.clone()))).or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
                .ok()
                .map(|n| n.and_utc())
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn feed_all(lines: &[Value]) -> (Vec<ParsedLine>, Option<(ParsedMessage, u64)>) {
        let mut p = UpdatesParser::default();
        let mut out = Vec::new();
        let mut pos = 0u64;
        for l in lines {
            let s = l.to_string();
            out.extend(p.feed(s.as_bytes(), pos));
            pos += s.len() as u64 + 1;
        }
        let tail = p.finish();
        (out, tail)
    }

    fn texts(v: &[ParsedLine]) -> Vec<(Role, String)> {
        v.iter()
            .filter_map(|l| match l {
                ParsedLine::Message(m) => Some((m.role, m.text.clone())),
                _ => None,
            })
            .collect()
    }

    fn upd(tag: &str, content: Value, meta: Value) -> Value {
        json!({"timestamp": 1_700_000_000, "method": "session/update",
            "params": {"sessionId": "s", "update": {"sessionUpdate": tag, "content": content, "_meta": meta}}})
    }

    #[test]
    fn examples_from_research() {
        let lines = [
            json!({"timestamp":1,"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"hello world"}}}}),
            json!({"timestamp":1,"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hi there"}}}}),
            json!({"timestamp":1,"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"! echo \"hi\"","_meta":{"bash_command":"echo \"hi\""}}}}}),
            json!({"timestamp":1,"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"tool_call","toolCallId":"tc1","title":"Read file","kind":"read","locations":[{"path":"/tmp/foo.rs"}]}}}),
        ];
        let (out, tail) = feed_all(&lines);
        assert_eq!(
            texts(&out),
            [
                (Role::User, "hello world".into()),
                (Role::Assistant, "hi there".into())
            ]
        );
        assert!(tail.is_none());
        match &out[0] {
            ParsedLine::Message(m) => assert_eq!(m.at, Utc.timestamp_opt(1, 0).single()),
            _ => unreachable!(),
        }
    }

    #[test]
    fn chunks_are_joined_thoughts_do_not_split_tools_do() {
        let t = |s: &str| json!({"type": "text", "text": s});
        let pi = |i: i64| json!({"promptIndex": i});
        let lines = [
            upd("user_message_chunk", t("почини"), pi(0)),
            upd("user_message_chunk", t("деплой"), pi(0)),
            upd("agent_message_chunk", t("Смотрю "), json!({})),
            upd("agent_thought_chunk", t("hidden"), json!({})),
            upd("agent_message_chunk", t("конфиг."), json!({})),
            json!({"timestamp": 2, "method": "session/update", "params": {"update": {"sessionUpdate": "tool_call"}}}),
            upd("agent_message_chunk", t("Готово."), json!({})),
            upd(
                "user_message_chunk",
                t("ещё"),
                json!({"promptIndex": 1, "hostTurn": true}),
            ),
            upd(
                "user_message_chunk",
                json!({"type": "text", "text": "raw", "_meta": {"interjection": true, "displayText": "стоп"}}),
                pi(1),
            ),
            upd("agent_message_chunk", t("Ок"), json!({})),
        ];
        let (out, tail) = feed_all(&lines);
        assert_eq!(
            texts(&out),
            [
                (Role::User, "почини\n\nдеплой".into()),
                (Role::Assistant, "Смотрю конфиг.".into()),
                (Role::Assistant, "Готово.".into()),
                (Role::User, "стоп".into()),
            ]
        );
        // The reply still being streamed stays open, with the offset of its first line.
        let (m, start) = tail.unwrap();
        assert_eq!(
            (m.role, m.text.as_str(), m.prompt_index),
            (Role::Assistant, "Ок", Some(1))
        );
        let before: u64 = lines[..9]
            .iter()
            .map(|l| l.to_string().len() as u64 + 1)
            .sum();
        assert_eq!(start, before);
    }

    #[test]
    fn rewind_marker() {
        let lines = [
            upd(
                "user_message_chunk",
                json!({"type": "text", "text": "a"}),
                json!({"promptIndex": 3}),
            ),
            json!({"timestamp": 1, "method": "_x.ai/session/update",
                "params": {"update": {"sessionUpdate": "rewind_marker", "target_prompt_index": 2}}}),
            json!({"timestamp": 1, "method": "_x.ai/session/update",
                "params": {"update": {"sessionUpdate": "turn_completed"}}}),
        ];
        let (out, _) = feed_all(&lines);
        assert!(matches!(out.last(), Some(ParsedLine::Rewind(2))));
        assert_eq!(texts(&out), [(Role::User, "a".into())]);
    }

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

    #[test]
    fn cli_rows() {
        assert_eq!(
            cli_message_text("user", r#"{"role":"user","content":"привет"}"#),
            Some((Role::User, "привет".into()))
        );
        assert_eq!(
            cli_message_text(
                "assistant",
                r#"{"role":"assistant","content":[{"type":"reasoning","text":"x"},{"type":"text","text":"ответ"},{"type":"tool-call","toolName":"bash"}]}"#
            ),
            Some((Role::Assistant, "ответ".into()))
        );
        assert_eq!(cli_message_text("tool", r#"{"content":"x"}"#), None);
        assert_eq!(cli_message_text("system", r#"{"content":"x"}"#), None);
        use rusqlite::types::Value as V;
        assert_eq!(
            cli_time(&V::Text("2026-11-01 10:00:00".into())),
            Utc.with_ymd_and_hms(2026, 11, 1, 10, 0, 0).single()
        );
        assert_eq!(
            cli_time(&V::Integer(1_700_000_000)),
            Utc.timestamp_opt(1_700_000_000, 0).single()
        );
    }
}
