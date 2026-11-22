//! Import from grok-cli (community `grok-dev`): `<grok root>/grok.db`.
//!
//! The database belongs to grok-cli and is opened strictly read-only (WAL is
//! tolerated). Schema v3: `sessions(id, workspace_id, title, recap_text,
//! cwd_at_start, cwd_last, created_at, updated_at, …)`, `workspaces(id,
//! canonical_path, …)`, `messages(session_id, seq, role, message_json,
//! created_at)`. Incremental: per session we remember the next source `seq`
//! (files row `<db path>#<session id>`) and the session's `updated_at`.

use std::collections::HashMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, Connection, OpenFlags};

use crate::db::{self, to_ms, FileState};
use crate::indexer::{count_messages, insert_message, title_from, upsert_file_row, SyncResult};
use crate::parse::grok::{cli_message_text, cli_time};
use crate::parse::ParsedMessage;
use crate::provider::Provider;

fn stat(path: &Path) -> (u64, i64) {
    match std::fs::metadata(path) {
        Ok(m) => (
            m.len(),
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as i64),
        ),
        Err(_) => (0, 0),
    }
}

/// Size and mtime of the database together with its WAL file.
fn db_fingerprint(path: &Path) -> (u64, i64) {
    let (s1, m1) = stat(path);
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    let (s2, m2) = stat(Path::new(&wal));
    (s1 + s2, m1.max(m2))
}

struct CliSession {
    id: String,
    title: Option<String>,
    workdir: Option<String>,
    created: Option<i64>,
    updated: Option<i64>,
    updated_key: i64,
}

fn text(v: SqlValue) -> Option<String> {
    match v {
        SqlValue::Text(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}

fn read_sessions(src: &Connection) -> Result<Vec<CliSession>> {
    let mut stmt = src.prepare(
        "SELECT s.id, s.title, s.recap_text, s.cwd_last, s.cwd_at_start, w.canonical_path,
                s.created_at, s.updated_at
           FROM sessions s LEFT JOIN workspaces w ON w.id = s.workspace_id",
    )?;
    let rows = stmt.query_map([], |r| {
        let id: SqlValue = r.get(0)?;
        let id = match id {
            SqlValue::Text(s) => s,
            SqlValue::Integer(n) => n.to_string(),
            _ => String::new(),
        };
        let updated_raw: SqlValue = r.get(7)?;
        let updated = to_ms(cli_time(&updated_raw));
        Ok(CliSession {
            id,
            title: text(r.get(1)?).or(text(r.get(2)?)),
            workdir: text(r.get(3)?).or(text(r.get(4)?)).or(text(r.get(5)?)),
            created: to_ms(cli_time(&r.get(6)?)),
            updated,
            updated_key: updated.unwrap_or(0),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

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
    let src = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open {} read-only", path.display()))?;
    src.busy_timeout(std::time::Duration::from_secs(5))?;
    let sessions = read_sessions(&src)?;
    let mut res = SyncResult::default();
    let mut msg_stmt = src.prepare(
        "SELECT seq, role, message_json, created_at FROM messages
          WHERE session_id = ?1 AND seq >= ?2 ORDER BY seq",
    )?;
    for s in sessions {
        if s.id.is_empty() || !crate::terminal::valid_id(&s.id) {
            continue;
        }
        let key = format!("{db_key}#{}", s.id);
        let state = known.get(&key);
        if state.is_some_and(|st| st.mtime == s.updated_key) {
            continue;
        }
        let from_seq = state.map_or(i64::MIN, |st| st.offset as i64);
        let mut new_msgs: Vec<ParsedMessage> = Vec::new();
        let mut max_seq: Option<i64> = None;
        let rows = msg_stmt.query_map(params![s.id, from_seq], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, SqlValue>(3)?,
            ))
        })?;
        for row in rows {
            let (seq, role, json, created) = row?;
            max_seq = Some(max_seq.map_or(seq, |m| m.max(seq)));
            if let Some((role, text)) = json.as_deref().and_then(|j| cli_message_text(&role, j)) {
                new_msgs.push(ParsedMessage {
                    role,
                    at: cli_time(&created),
                    text,
                    prompt_index: None,
                });
            }
        }
        let next_src_seq = max_seq.map_or(state.map_or(0, |st| st.offset as i64), |m| m + 1);

        let tx = conn.transaction()?;
        let session = match db::get_session(&tx, &s.id)? {
            Some(existing) if existing.file_path != db_key => {
                tracing::warn!(
                    "grok-cli session {} clashes with another source; skipped",
                    s.id
                );
                continue;
            }
            Some(existing) => existing,
            None => {
                tx.execute(
                    "INSERT INTO sessions (id, provider, file_path, kind) VALUES (?1, ?2, ?3, 'interactive')",
                    params![s.id, Provider::Grok.as_str(), db_key],
                )?;
                db::get_session(&tx, &s.id)?.expect("session inserted")
            }
        };
        let before = count_messages(&tx, &session.id)?;
        let mut seq = session.next_seq;
        for m in &new_msgs {
            insert_message(&tx, &session.id, seq, m)?;
            seq += 1;
        }
        let last_msg_at = new_msgs.iter().rev().find_map(|m| to_ms(m.at));
        let title = s.title.clone().or(session.title.clone()).or_else(|| {
            new_msgs
                .iter()
                .find(|m| m.role.as_str() == "user")
                .map(|m| title_from(&m.text))
        });
        let count = count_messages(&tx, &session.id)?;
        tx.execute(
            "UPDATE sessions SET workdir = COALESCE(?2, workdir), title = ?3,
                    started_at = COALESCE(started_at, ?4), last_at = COALESCE(?5, ?6, last_at),
                    message_count = ?7, next_seq = ?8
              WHERE id = ?1",
            params![
                session.id,
                s.workdir,
                title,
                s.created,
                last_msg_at.max(s.updated),
                s.updated,
                count,
                seq
            ],
        )?;
        upsert_file_row(
            &tx,
            &key,
            Provider::Grok,
            Some(&session.id),
            0,
            next_src_seq.max(0) as u64,
            s.updated_key,
            None,
        )?;
        tx.commit()?;
        res.updated += 1;
        res.messages += (count - before).max(0) as usize;
    }
    upsert_file_row(conn, &db_key, Provider::Grok, None, size, 0, mtime, None)?;
    Ok(res)
}
