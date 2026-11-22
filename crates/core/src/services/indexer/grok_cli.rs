//! Import from grok-cli's database (`<grok root>/grok.db`, see
//! [`crate::sources::grok::cli`]). Incremental: per session we remember the
//! next source `seq` (files row `<db path>#<session id>`) and the session's
//! `updated_at`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use super::meta::title_from;
use crate::models::time::to_ms;
use crate::models::ParsedMessage;
use crate::models::{Provider, SyncResult};
use crate::services::command::valid_id;
use crate::sources::grok::cli::{db_fingerprint, CliDb, CliSession};
use crate::storage::files::{self, FileRow, FileState};
use crate::storage::messages;
use crate::storage::sessions::{self, DbSessionUpdate, SessionRow};

/// Imports new grok-cli messages. `known` are the file states loaded by the
/// caller (database fingerprint and per-session progress).
pub fn sync(
    conn: &mut Connection,
    path: &Path,
    known: &HashMap<String, FileState>,
) -> Result<SyncResult> {
    let db_key = path.to_string_lossy().into_owned();
    let (size, mtime) = db_fingerprint(path);
    if known
        .get(&db_key)
        .is_some_and(|s| s.size == size && s.mtime == mtime)
    {
        return Ok(SyncResult::default());
    }
    let src = CliDb::open(path)?;
    let cli_sessions = src.sessions()?;
    src.prepare_messages()?;
    let mut res = SyncResult::default();
    for s in cli_sessions {
        if s.id.is_empty() || !valid_id(&s.id) {
            continue;
        }
        let key = format!("{db_key}#{}", s.id);
        let state = known.get(&key);
        if state.is_some_and(|st| st.mtime == s.updated_key) {
            continue;
        }
        if let Some(added) = import_session(conn, &src, &db_key, &key, &s, state)? {
            res.updated += 1;
            res.messages += added;
        }
    }
    let row = FileRow {
        path: &db_key,
        provider: Provider::Grok,
        session_id: None,
        size,
        offset: 0,
        mtime,
        tail_seq: None,
    };
    files::upsert(conn, &row)?;
    Ok(res)
}

/// Imports one changed session in its own transaction. `None`: the id belongs
/// to another source and the session is skipped.
fn import_session(
    conn: &mut Connection,
    src: &CliDb,
    db_key: &str,
    key: &str,
    s: &CliSession,
    state: Option<&FileState>,
) -> Result<Option<usize>> {
    let from_seq = state.map_or(i64::MIN, |st| st.offset as i64);
    let read = src.messages(&s.id, from_seq)?;
    let next_src_seq = read
        .max_seq
        .map_or(state.map_or(0, |st| st.offset as i64), |m| m + 1);
    let tx = conn.transaction()?;
    let session = match sessions::get_session(&tx, &s.id)? {
        Some(existing) if existing.file_path != db_key => {
            tracing::warn!(
                "grok-cli session {} clashes with another source; skipped",
                s.id
            );
            return Ok(None);
        }
        Some(existing) => existing,
        None => {
            sessions::insert_interactive(&tx, &s.id, Provider::Grok, db_key)?;
            sessions::get_session(&tx, &s.id)?.expect("session inserted")
        }
    };
    let before = messages::count(&tx, &session.id)?;
    let count = store_messages(&tx, &session, s, &read.messages)?;
    let row = FileRow {
        path: key,
        provider: Provider::Grok,
        session_id: Some(&session.id),
        size: 0,
        offset: next_src_seq.max(0) as u64,
        mtime: s.updated_key,
        tail_seq: None,
    };
    files::upsert(&tx, &row)?;
    tx.commit()?;
    Ok(Some((count - before).max(0) as usize))
}

/// Appends messages and refreshes session metadata; returns the message count.
fn store_messages(
    tx: &Connection,
    session: &SessionRow,
    s: &CliSession,
    new_msgs: &[ParsedMessage],
) -> Result<i64> {
    let mut seq = session.next_seq;
    for m in new_msgs {
        messages::insert(tx, &session.id, seq, m)?;
        seq += 1;
    }
    let last_msg_at = new_msgs.iter().rev().find_map(|m| to_ms(m.at));
    let title = s.title.clone().or(session.title.clone()).or_else(|| {
        new_msgs
            .iter()
            .find(|m| m.role.as_str() == "user")
            .map(|m| title_from(&m.text))
    });
    let count = messages::count(tx, &session.id)?;
    let update = DbSessionUpdate {
        workdir: s.workdir.clone(),
        title,
        started_at: s.created,
        last_at: last_msg_at.max(s.updated),
        updated_at: s.updated,
        message_count: count,
        next_seq: seq,
    };
    sessions::update_from_db(tx, &session.id, &update)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::files::file_states;

    fn run(conn: &mut Connection, db: &Path) -> Result<SyncResult> {
        let known = file_states(conn)?;
        sync(conn, db, &known)
    }

    fn source(path: &Path, updated: &str, messages: &[(i64, &str, &str)]) {
        let _ = std::fs::remove_file(path);
        let c = Connection::open(path).unwrap();
        c.execute_batch(
            "CREATE TABLE workspaces(id TEXT, canonical_path TEXT);
             CREATE TABLE sessions(id TEXT, workspace_id TEXT, title TEXT, recap_text TEXT,
                 cwd_at_start TEXT, cwd_last TEXT, created_at TEXT, updated_at TEXT);
             CREATE TABLE messages(session_id TEXT, seq INTEGER, role TEXT, message_json TEXT, created_at TEXT);
             INSERT INTO workspaces VALUES ('w', '/proj');",
        )
        .unwrap();
        c.execute(
            "INSERT INTO sessions VALUES ('s-1', 'w', NULL, NULL, NULL, NULL, '2026-11-01 10:00:00', ?1)",
            [updated],
        )
        .unwrap();
        for (seq, role, text) in messages {
            let json = serde_json::json!({"role": role, "content": text}).to_string();
            c.execute(
                "INSERT INTO messages VALUES ('s-1', ?1, ?2, ?3, '2026-11-01 10:00:00')",
                rusqlite::params![seq, role, json],
            )
            .unwrap();
        }
    }

    #[test]
    fn imports_new_messages_incrementally() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("grok.db");
        let mut conn = crate::storage::open(&dir.path().join("i.sqlite")).unwrap();
        source(
            &db,
            "2026-11-01 10:05:00",
            &[(0, "user", "вопрос"), (1, "tool", "x")],
        );
        let r = run(&mut conn, &db).unwrap();
        assert_eq!((r.updated, r.messages), (1, 1));
        let s = sessions::get_session(&conn, "s-1").unwrap().unwrap();
        assert_eq!(
            (s.workdir.as_deref(), s.title.as_deref()),
            (Some("/proj"), Some("вопрос"))
        );
        assert_eq!(s.kind.as_deref(), Some("interactive"));

        // Unchanged database: nothing to do.
        let r = run(&mut conn, &db).unwrap();
        assert_eq!(r, SyncResult::default());

        source(
            &db,
            "2026-11-01 10:09:00",
            &[
                (0, "user", "вопрос"),
                (1, "tool", "x"),
                (2, "assistant", "ответ"),
            ],
        );
        let r = run(&mut conn, &db).unwrap();
        assert_eq!((r.updated, r.messages), (1, 1));
        assert_eq!(messages::count(&conn, "s-1").unwrap(), 2);
    }

    #[test]
    fn missing_messages_table_fails_before_import() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("grok.db");
        Connection::open(&db)
            .unwrap()
            .execute_batch(
                "CREATE TABLE workspaces(id TEXT, canonical_path TEXT);
                 CREATE TABLE sessions(id TEXT, workspace_id TEXT, title TEXT, recap_text TEXT,
                     cwd_at_start TEXT, cwd_last TEXT, created_at TEXT, updated_at TEXT);",
            )
            .unwrap();
        let mut conn = crate::storage::open(&dir.path().join("i.sqlite")).unwrap();
        assert!(run(&mut conn, &db).is_err());
        assert!(file_states(&conn).unwrap().is_empty());
    }
}
