//! Grok Build `updates.jsonl`: append-only ACP `session/update` notifications
//! with streamed chunks, joined into whole prompts and replies.

use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

use crate::models::{ParsedLine, ParsedMessage, Role};
use crate::sources::json::{to_date, truthy};
use crate::sources::LineParser;

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

/// The parts of one `session/update` line the parser looks at.
struct Update<'a> {
    params: Option<&'a Value>,
    update: Option<&'a Value>,
    content: Option<&'a Value>,
    content_meta: Option<&'a Value>,
    update_meta: Option<&'a Value>,
    is_text: bool,
    at: Option<DateTime<Utc>>,
    start: u64,
}

impl<'a> Update<'a> {
    fn new(d: &'a Value, start: u64) -> Self {
        let params = d.get("params");
        let update = params.and_then(|p| p.get("update"));
        let content = update.and_then(|u| u.get("content"));
        Update {
            params,
            update,
            content,
            content_meta: content.and_then(|c| c.get("_meta")),
            update_meta: update.and_then(|u| u.get("_meta")),
            is_text: content.and_then(|c| c.get("type")).and_then(Value::as_str) == Some("text"),
            at: unix_seconds(d.get("timestamp")),
            start,
        }
    }

    fn tag(&self) -> &'a str {
        self.update
            .and_then(|u| u.get("sessionUpdate"))
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    /// Shell commands (`! cmd`) and host turns are not the user's prompt.
    fn is_not_prompt(&self) -> bool {
        self.content_meta
            .and_then(|m| m.get("bash_command"))
            .is_some_and(|v| !v.is_null())
            || truthy(self.update_meta.and_then(|m| m.get("hostTurn")))
    }

    /// Prompt text; an interjection shows its `displayText`.
    fn user_text(&self) -> String {
        if truthy(self.content_meta.and_then(|m| m.get("interjection"))) {
            self.content_meta
                .and_then(|m| m.get("displayText"))
                .and_then(Value::as_str)
        } else {
            self.content
                .and_then(|c| c.get("text"))
                .and_then(Value::as_str)
        }
        .unwrap_or("")
        .to_owned()
    }

    fn rewind_target(&self) -> Option<i64> {
        let key = "target_prompt_index";
        self.update
            .and_then(|u| u.get(key))
            .or_else(|| self.update_meta.and_then(|m| m.get(key)))
            .or_else(|| self.params.and_then(|p| p.get(key)))
            .and_then(Value::as_i64)
    }
}

impl UpdatesParser {
    fn user_chunk(&mut self, u: &Update<'_>) -> Vec<ParsedLine> {
        if u.is_not_prompt() {
            return self.close();
        }
        if !u.is_text {
            // Images/resources belong to the prompt but carry no text.
            return vec![];
        }
        let text = u.user_text();
        let pi = u
            .update_meta
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
            at: u.at,
            start: u.start,
        });
        out
    }

    fn agent_chunk(&mut self, u: &Update<'_>) -> Vec<ParsedLine> {
        if !u.is_text {
            return vec![];
        }
        let chunk = u
            .content
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
            at: u.at,
            start: u.start,
        });
        out
    }

    fn rewind(&mut self, u: &Update<'_>) -> Vec<ParsedLine> {
        let mut out = self.close();
        if let Some(t) = u.rewind_target() {
            out.push(ParsedLine::Rewind(t));
            self.last_prompt = Some(t);
        }
        out
    }
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
        let u = Update::new(&d, start);
        match (method, u.tag()) {
            ("session/update", "agent_thought_chunk") => vec![],
            ("session/update", "user_message_chunk") => self.user_chunk(&u),
            ("session/update", "agent_message_chunk") => self.agent_chunk(&u),
            ("_x.ai/session/update", "rewind_marker") => self.rewind(&u),
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
}
