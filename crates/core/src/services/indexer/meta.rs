//! Session metadata gathered while reading a transcript, and the title rule.

use chrono::{DateTime, Utc};

use crate::models::{ParsedLine, ParsedMessage};

/// Metadata of the session being indexed: first id/cwd/kind win, the latest
/// branch/title win, times span the first and the last record.
#[derive(Debug, Default)]
pub struct Meta {
    pub id: Option<String>,
    pub cwd: Option<String>,
    pub kind: Option<String>,
    pub git_branch: Option<String>,
    pub title: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_at: Option<DateTime<Utc>>,
}

pub fn note_at(meta: &mut Meta, at: Option<DateTime<Utc>>) {
    if let Some(at) = at {
        meta.started_at.get_or_insert(at);
        meta.last_at = Some(at);
    }
}

pub fn collect(
    items: Vec<ParsedLine>,
    meta: &mut Meta,
    msgs: &mut Vec<ParsedMessage>,
    rewinds: &mut Vec<i64>,
) {
    for item in items {
        match item {
            ParsedLine::Meta(m) => {
                if meta.id.is_none() {
                    meta.id = m.id;
                }
                if meta.cwd.is_none() {
                    meta.cwd = m.cwd;
                }
                if meta.kind.is_none() {
                    meta.kind = m.kind;
                }
                if m.git_branch.is_some() {
                    meta.git_branch = m.git_branch;
                }
                if m.title.is_some() {
                    meta.title = m.title;
                }
                note_at(meta, m.at);
            }
            ParsedLine::Message(m) => {
                note_at(meta, m.at);
                msgs.push(m);
            }
            ParsedLine::Rewind(target) => {
                msgs.retain(|m| !matches!(m.prompt_index, Some(p) if p > target));
                rewinds.push(target);
            }
        }
    }
}

pub fn title_from(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ParsedMeta, Role};

    #[test]
    fn title_collapses_whitespace_and_clips() {
        assert_eq!(title_from("  a\n\n b\tc "), "a b c");
        assert_eq!(title_from(&"я".repeat(500)).chars().count(), 200);
    }

    fn msg(text: &str, prompt_index: Option<i64>) -> ParsedLine {
        ParsedLine::Message(ParsedMessage {
            role: Role::User,
            at: "2026-11-01T10:00:00Z".parse().ok(),
            text: text.into(),
            prompt_index,
        })
    }

    #[test]
    fn collect_merges_meta_and_applies_rewinds() {
        let mut meta = Meta::default();
        let mut msgs = Vec::new();
        let mut rewinds = Vec::new();
        let first = ParsedMeta {
            id: Some("a".into()),
            cwd: Some("/w".into()),
            git_branch: Some("main".into()),
            ..Default::default()
        };
        let later = ParsedMeta {
            id: Some("b".into()),
            git_branch: Some("dev".into()),
            title: Some("T".into()),
            at: "2026-11-02T10:00:00Z".parse().ok(),
            ..Default::default()
        };
        collect(
            vec![
                ParsedLine::Meta(first),
                msg("p0", Some(0)),
                msg("p1", Some(1)),
                ParsedLine::Rewind(0),
                ParsedLine::Meta(later),
            ],
            &mut meta,
            &mut msgs,
            &mut rewinds,
        );
        assert_eq!(
            (meta.id.as_deref(), meta.cwd.as_deref()),
            (Some("a"), Some("/w"))
        );
        assert_eq!(
            (meta.git_branch.as_deref(), meta.title.as_deref()),
            (Some("dev"), Some("T"))
        );
        assert_eq!(meta.started_at, "2026-11-01T10:00:00Z".parse().ok());
        assert_eq!(meta.last_at, "2026-11-02T10:00:00Z".parse().ok());
        assert_eq!(msgs.len(), 1);
        assert_eq!(rewinds, [0]);
    }
}
