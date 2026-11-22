//! Grok Build sessions on disk.

use std::fs;
use std::path::Path;

use super::summary::{decode_cwd, parse_summary};
use crate::models::Provider;
use crate::sources::fs::{read_dir_sorted, stat};
use crate::sources::SourceFile;

/// Grok Build: `<root>/sessions/<encoded cwd>/<session id>/updates.jsonl` with
/// `summary.json` next to it. Hidden and subagent sessions are skipped (ids
/// that are unsafe in a command are dropped by the indexer). The
/// file's mtime also covers `summary.json`, so title changes are picked up.
pub fn discover(root: &Path) -> Vec<SourceFile> {
    let mut out = Vec::new();
    for group in read_dir_sorted(&root.join("sessions")) {
        if !group.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let group_path = group.path();
        let group_cwd = group.file_name().to_str().and_then(decode_cwd).or_else(|| {
            fs::read_to_string(group_path.join(".cwd"))
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        });
        for session in read_dir_sorted(&group_path) {
            if !session.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let dir = session.path();
            let path = dir.join("updates.jsonl");
            let Some((size, mut mtime_ms)) = stat(&path) else {
                continue;
            };
            let summary_path = dir.join("summary.json");
            let summary = fs::read_to_string(&summary_path)
                .ok()
                .and_then(|s| parse_summary(&s))
                .unwrap_or_default();
            if summary.skipped() {
                continue;
            }
            if let Some((_, m)) = stat(&summary_path) {
                mtime_ms = mtime_ms.max(m);
            }
            let id = summary
                .id
                .clone()
                .or_else(|| session.file_name().to_str().map(str::to_owned));
            let Some(id) = id else { continue };
            let meta = summary.meta(&id, group_cwd.clone());
            out.push(SourceFile {
                provider: Provider::Grok,
                path,
                size,
                mtime_ms,
                id,
                meta,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(dir: &Path, summary: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("updates.jsonl"), "{}\n").unwrap();
        std::fs::write(dir.join("summary.json"), summary).unwrap();
    }

    #[test]
    fn finds_visible_sessions_with_their_workdir() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        session(
            &sessions.join("%2Fw%2Fp/019a-1"),
            r#"{"generated_title":"Т"}"#,
        );
        session(&sessions.join("%2Fw%2Fp/019a-2"), r#"{"hidden":true}"#);
        session(
            &sessions.join("%2Fw%2Fp/019a-3"),
            r#"{"session_kind":"subagent"}"#,
        );
        session(&sessions.join("%2Fw%2Fp/-bad"), "{}");
        let hashed = sessions.join("long-0123456789abcdef");
        session(&hashed.join("019a-4"), r#"{"info":{"id":"019a-4b"}}"#);
        std::fs::write(hashed.join(".cwd"), " /long/path \n").unwrap();

        let found = discover(dir.path());
        let ids: Vec<_> = found.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["-bad", "019a-1", "019a-4b"]);
        let cwd = |i: usize| match &found[i].meta[0] {
            crate::models::ParsedLine::Meta(m) => m.cwd.clone(),
            _ => None,
        };
        assert_eq!(cwd(1).as_deref(), Some("/w/p"));
        assert_eq!(cwd(2).as_deref(), Some("/long/path"));
        assert!(discover(&dir.path().join("missing")).is_empty());
    }
}
