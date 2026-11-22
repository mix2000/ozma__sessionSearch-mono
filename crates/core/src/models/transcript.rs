//! Records produced by transcript sources and consumed by the indexer.
//!
//! Only the text of user prompts and assistant answers is indexed: tool output,
//! thinking, service injections and subagent conversations never reach search.

use chrono::{DateTime, Utc};

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
