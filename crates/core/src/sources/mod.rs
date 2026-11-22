//! Transcript sources (adapters): where each agent keeps its conversations on
//! disk and how their lines turn into [`ParsedLine`] records.
//!
//! * [`claude`] — Claude Code `<projects>/<folder>/<uuid>.jsonl`;
//! * [`codex`] — Codex CLI `<sessions>/YYYY/MM/DD/rollout-…-<uuid>.jsonl`;
//! * [`grok`] — Grok Build `updates.jsonl` + `summary.json`, and grok-cli's `grok.db`.

use std::path::{Path, PathBuf};

use crate::models::{ParsedLine, ParsedMessage, Provider};

pub mod claude;
pub mod codex;
pub mod fs;
pub mod grok;
pub mod json;
pub mod lines;
pub mod registry;

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

/// Stateless line parsers (Claude, Codex): byte pre-filter + JSON parse.
struct Simple {
    wanted: fn(&[u8]) -> bool,
    parse: fn(&str) -> Vec<ParsedLine>,
}

impl LineParser for Simple {
    fn feed(&mut self, line: &[u8], _start: u64) -> Vec<ParsedLine> {
        if !(self.wanted)(line) {
            return vec![];
        }
        (self.parse)(&String::from_utf8_lossy(line))
    }
}

/// A fresh parser for one file of `provider`.
pub fn parser_for(provider: Provider) -> Box<dyn LineParser> {
    match provider {
        Provider::Claude => Box::new(Simple {
            wanted: claude::line_wanted,
            parse: claude::parse_line,
        }),
        Provider::Codex => Box::new(Simple {
            wanted: codex::line_wanted,
            parse: codex::parse_line,
        }),
        Provider::Grok => Box::new(grok::UpdatesParser::default()),
    }
}

/// A transcript file found on disk.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub provider: Provider,
    pub path: PathBuf,
    pub size: u64,
    pub mtime_ms: i64,
    /// Id from the file name (Codex refines it from `session_meta`).
    pub id: String,
    /// Metadata known before reading the file (Grok Build `summary.json`).
    pub meta: Vec<ParsedLine>,
}

/// All transcript files of a provider under `root`.
pub fn discover(provider: Provider, root: &Path) -> Vec<SourceFile> {
    match provider {
        Provider::Claude => claude::discover(root),
        Provider::Codex => codex::discover(root),
        Provider::Grok => grok::discover(root),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_parser_prefilters_lines() {
        let mut p = parser_for(Provider::Claude);
        assert!(p.feed(br#"{"type":"progress"}"#, 0).is_empty());
        let line = br#"{"type":"assistant","message":{"content":"hi"}}"#;
        assert_eq!(p.feed(line, 0).len(), 1);
        assert!(p.finish().is_none());
        assert!(discover(Provider::Codex, Path::new("/nonexistent/x")).is_empty());
    }
}
