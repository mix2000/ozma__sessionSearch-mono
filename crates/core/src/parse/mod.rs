//! Parsing of transcript lines into indexable records.
//!
//! Only the text of user prompts and assistant answers is indexed: tool output,
//! thinking, service injections and subagent conversations never reach search.

use chrono::{DateTime, Utc};
use serde_json::Value;

pub mod claude;
pub mod codex;
pub mod grok;

/// Role of an indexed message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// A user prompt or an assistant answer.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMessage {
    pub role: Role,
    pub at: Option<DateTime<Utc>>,
    pub text: String,
    /// Grok: index of the prompt this message belongs to (for rewinds).
    pub prompt_index: Option<i64>,
}

/// Session metadata found in a line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedMeta {
    pub id: Option<String>,
    pub cwd: Option<String>,
    /// `interactive` | `headless`
    pub kind: Option<String>,
    pub git_branch: Option<String>,
    pub title: Option<String>,
    pub at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedLine {
    Message(ParsedMessage),
    Meta(ParsedMeta),
    /// Grok: prompts with a greater index were rewound (a dead branch).
    Rewind(i64),
}

/// Turns raw transcript lines into records. Stateful parsers (Grok joins
/// streamed chunks) may keep the last group open: [`LineParser::finish`] returns
/// it with the byte offset where it started, so the next incremental pass
/// re-reads it instead of splitting a message that is still being written.
pub trait LineParser {
    /// `start` is the byte offset of the line in the file.
    fn feed(&mut self, line: &[u8], start: u64) -> Vec<ParsedLine>;
    fn finish(&mut self) -> Option<(ParsedMessage, u64)> {
        None
    }
    /// Continuing a file: the prompt index of the last stored message.
    fn resume_prompt(&mut self, _prompt_index: i64) {}
}

/// Same semantics as `new Date(value)` for the ISO strings agents write.
pub(crate) fn to_date(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let s = value?.as_str()?;
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.with_timezone(&Utc));
    }
    // ISO without zone: JS treats it as local time; we treat it as UTC.
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .map(|n| n.and_utc())
}

/// JavaScript truthiness of a JSON value.
pub(crate) fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

pub(crate) fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}
