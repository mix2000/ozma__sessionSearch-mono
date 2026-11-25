//! Turning matched message rows into the search response: grouping by
//! session, session cards, hits with snippets.

use std::collections::HashMap;

use std::path::Path;

use super::live::LiveMap;
use super::resume;
use super::snippet::build_snippet;
use crate::models::time::iso_opt;
use crate::models::{LiveState, SearchResponse, TargetOs, TranscriptHit, TranscriptResult};
use crate::platform::FsProbe;
use crate::storage::messages::MessageRow;
use crate::storage::sessions::SessionRow;

pub const MAX_ROWS: usize = 600;
pub const HITS_PER_SESSION: usize = 3;
pub const DEFAULT_LIMIT: usize = 40;

/// Rows grouped by session, sessions in the order of their freshest row.
#[derive(Debug, Default)]
pub struct Grouped {
    pub order: Vec<String>,
    pub by_session: HashMap<String, Vec<MessageRow>>,
}

pub fn group_by_session(rows: &[MessageRow]) -> Grouped {
    let mut g = Grouped::default();
    for r in rows {
        let list = g.by_session.entry(r.session_id.clone()).or_insert_with(|| {
            g.order.push(r.session_id.clone());
            Vec::new()
        });
        list.push(r.clone());
    }
    g
}

/// Requested number of sessions (default 40, at most 100).
pub fn effective_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).min(100)
}

/// A session card without hits.
pub fn to_result(
    t: SessionRow,
    live: &LiveMap,
    workdir_exists: bool,
    os: TargetOs,
) -> TranscriptResult {
    TranscriptResult {
        live: live
            .get(&t.id.to_lowercase())
            .map(|&pid| LiveState::External { pid }),
        resume_command: resume::command(&t, os),
        resume_needs_folder: workdir_exists && resume::needs_folder(&t, os),
        id: t.id,
        provider: t.provider,
        title: t.title,
        workdir: t.workdir,
        kind: t.kind,
        git_branch: t.git_branch,
        started_at: iso_opt(t.started_at),
        last_at: iso_opt(t.last_at),
        message_count: t.message_count,
        workdir_exists,
        matches: 0,
        hits: Vec::new(),
    }
}

/// Fills in the match count and the first hits of a session card.
pub fn with_hits(
    mut result: TranscriptResult,
    rows: &[MessageRow],
    terms: &[String],
) -> TranscriptResult {
    result.matches = rows.len();
    result.hits = rows
        .iter()
        .take(HITS_PER_SESSION)
        .map(|h| TranscriptHit {
            message_id: h.id,
            seq: h.seq,
            role: h.role.clone(),
            at: iso_opt(h.at),
            snippet: build_snippet(&h.text, terms),
        })
        .collect();
    result
}

/// What a card needs besides the stored session.
pub struct CardContext<'a> {
    pub live: &'a LiveMap,
    pub fs: &'a dyn FsProbe,
    pub os: TargetOs,
}

/// The response: one card per found session (in `sessions` order) with its hits.
pub fn assemble(
    terms: Vec<String>,
    rows: usize,
    grouped: &Grouped,
    sessions: Vec<SessionRow>,
    limit: usize,
    ctx: &CardContext<'_>,
) -> SearchResponse {
    let results = sessions
        .into_iter()
        .map(|s| {
            let hits = &grouped.by_session[&s.id];
            let exists = s
                .workdir
                .as_deref()
                .is_some_and(|w| ctx.fs.is_dir(Path::new(w)));
            with_hits(to_result(s, ctx.live, exists, ctx.os), hits, &terms)
        })
        .collect();
    SearchResponse {
        truncated: is_truncated(rows, grouped.by_session.len(), limit),
        terms,
        results,
    }
}

/// Whether more rows or sessions matched than are returned.
pub fn is_truncated(rows: usize, sessions: usize, limit: usize) -> bool {
    rows >= MAX_ROWS || sessions > limit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, session: &str, text: &str) -> MessageRow {
        MessageRow {
            id,
            session_id: session.into(),
            seq: id,
            role: "user".into(),
            at: Some(0),
            text: text.into(),
        }
    }

    fn session(id: &str, provider: &str, workdir: Option<&str>) -> SessionRow {
        SessionRow {
            id: id.into(),
            provider: provider.into(),
            file_path: "/f.jsonl".into(),
            workdir: workdir.map(str::to_owned),
            title: Some("t".into()),
            kind: None,
            git_branch: None,
            started_at: Some(0),
            last_at: None,
            message_count: 5,
            next_seq: 5,
        }
    }

    #[test]
    fn grouping_keeps_freshest_first() {
        let rows = [row(3, "b", "x"), row(2, "a", "x"), row(1, "b", "x")];
        let g = group_by_session(&rows);
        assert_eq!(g.order, ["b", "a"]);
        assert_eq!(g.by_session["b"].len(), 2);
        assert_eq!(effective_limit(None), 40);
        assert_eq!(effective_limit(Some(500)), 100);
        assert!(is_truncated(MAX_ROWS, 1, 40));
        assert!(is_truncated(1, 41, 40));
        assert!(!is_truncated(1, 40, 40));
    }

    #[test]
    fn result_card_and_hits() {
        let mut live = LiveMap::new();
        live.insert("abc".into(), 42);
        let r = to_result(
            session("ABC", "claude", Some("/my dir")),
            &live,
            true,
            TargetOs::Mac,
        );
        assert_eq!(r.live, Some(LiveState::External { pid: 42 }));
        assert_eq!(
            r.resume_command.as_deref(),
            Some("cd '/my dir' && claude --resume ABC")
        );
        assert!(!r.resume_needs_folder);
        assert_eq!(r.started_at.as_deref(), Some("1970-01-01T00:00:00.000Z"));
        let rows: Vec<_> = (0..5).map(|i| row(i, "ABC", "a nginx b")).collect();
        let r = with_hits(r, &rows, &["nginx".into()]);
        assert_eq!((r.matches, r.hits.len()), (5, HITS_PER_SESSION));
        assert_eq!(r.hits[0].snippet.matched, "a nginx b");
    }

    #[test]
    fn folder_hint_only_while_the_folder_exists() {
        let card = |exists| {
            let s = session("ABC", "claude", Some(r"C:\100%"));
            to_result(s, &LiveMap::new(), exists, TargetOs::Windows)
        };
        assert!(card(true).resume_needs_folder);
        assert!(!card(false).resume_needs_folder);
    }

    struct Dirs(&'static [&'static str]);
    impl FsProbe for Dirs {
        fn is_dir(&self, p: &Path) -> bool {
            self.0.iter().any(|d| Path::new(d) == p)
        }
        fn is_file(&self, _: &Path) -> bool {
            false
        }
    }

    #[test]
    fn assemble_cards_in_session_order() {
        let rows = [
            row(3, "b", "x nginx"),
            row(2, "a", "nginx"),
            row(1, "b", "y"),
        ];
        let g = group_by_session(&rows);
        let live = LiveMap::new();
        let ctx = CardContext {
            live: &live,
            fs: &Dirs(&["/w"]),
            os: TargetOs::Linux,
        };
        let sessions = vec![
            session("b", "codex", Some("/w")),
            session("a", "claude", Some("/gone")),
        ];
        let res = assemble(vec!["nginx".into()], rows.len(), &g, sessions, 1, &ctx);
        assert!(res.truncated);
        assert_eq!(res.results.len(), 2);
        assert_eq!(
            (res.results[0].matches, res.results[0].workdir_exists),
            (2, true)
        );
        assert!(!res.results[1].workdir_exists);
        assert_eq!(
            res.results[0].resume_command.as_deref(),
            Some("cd /w && codex resume b")
        );
    }
}
