//! `sessions` table: one row per conversation.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::models::Provider;

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String,
    pub provider: String,
    pub file_path: String,
    pub workdir: Option<String>,
    pub title: Option<String>,
    pub kind: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<i64>,
    pub last_at: Option<i64>,
    pub message_count: i64,
    pub next_seq: i64,
}

const SESSION_COLS: &str =
    "id, provider, file_path, workdir, title, kind, git_branch, started_at, last_at, message_count, next_seq";

fn session_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        id: r.get(0)?,
        provider: r.get(1)?,
        file_path: r.get(2)?,
        workdir: r.get(3)?,
        title: r.get(4)?,
        kind: r.get(5)?,
        git_branch: r.get(6)?,
        started_at: r.get(7)?,
        last_at: r.get(8)?,
        message_count: r.get(9)?,
        next_seq: r.get(10)?,
    })
}

pub fn get_session(conn: &Connection, id: &str) -> Result<Option<SessionRow>> {
    let sql = format!("SELECT {SESSION_COLS} FROM sessions WHERE id = ?1");
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([id], session_from_row)
        .optional()?)
}

pub fn get_sessions(conn: &Connection, ids: &[String]) -> Result<Vec<SessionRow>> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(s) = get_session(conn, id)? {
            out.push(s);
        }
    }
    Ok(out)
}

pub fn stats(conn: &Connection) -> Result<(i64, i64)> {
    Ok(conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(message_count), 0) FROM sessions",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?)
}

pub fn sessions_by_provider(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare("SELECT provider, COUNT(*) FROM sessions GROUP BY provider")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Creates a session for a transcript file.
pub fn insert(conn: &Connection, id: &str, provider: Provider, file_path: &str) -> Result<()> {
    conn.prepare_cached("INSERT INTO sessions (id, provider, file_path) VALUES (?1, ?2, ?3)")?
        .execute(params![id, provider.as_str(), file_path])?;
    Ok(())
}

/// Creates an interactive session imported from a database (grok-cli).
pub fn insert_interactive(
    conn: &Connection,
    id: &str,
    provider: Provider,
    file_path: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, provider, file_path, kind) VALUES (?1, ?2, ?3, 'interactive')",
        params![id, provider.as_str(), file_path],
    )?;
    Ok(())
}

/// Drops all messages of a session (its file is read again from the start).
pub fn reset(conn: &Connection, id: &str) -> Result<()> {
    conn.prepare_cached("DELETE FROM messages WHERE session_id = ?1")?
        .execute([id])?;
    conn.prepare_cached(
        "UPDATE sessions SET message_count = 0, next_seq = 0, title = NULL WHERE id = ?1",
    )?
    .execute([id])?;
    Ok(())
}

pub fn set_file_path(conn: &Connection, id: &str, path: &str) -> Result<()> {
    conn.prepare_cached("UPDATE sessions SET file_path = ?2 WHERE id = ?1")?
        .execute(params![id, path])?;
    Ok(())
}

pub fn set_next_seq(conn: &Connection, id: &str, seq: i64) -> Result<()> {
    conn.prepare_cached("UPDATE sessions SET next_seq = ?2 WHERE id = ?1")?
        .execute(params![id, seq])?;
    Ok(())
}

/// Metadata after indexing a transcript file. Workdir and kind are kept from
/// the first pass; branch and times are refreshed when known.
#[derive(Debug, Clone, Default)]
pub struct FileSessionUpdate {
    pub workdir: Option<String>,
    pub kind: Option<String>,
    pub git_branch: Option<String>,
    pub title: Option<String>,
    pub started_at: Option<i64>,
    pub last_at: Option<i64>,
    pub message_count: i64,
    pub next_seq: i64,
}

pub fn update_from_file(conn: &Connection, id: &str, u: &FileSessionUpdate) -> Result<()> {
    conn.prepare_cached(
        "UPDATE sessions SET
            workdir = COALESCE(workdir, ?2),
            kind = COALESCE(kind, ?3),
            git_branch = COALESCE(?4, git_branch),
            title = ?5,
            started_at = COALESCE(started_at, ?6),
            last_at = COALESCE(?7, last_at),
            message_count = ?8,
            next_seq = ?9
          WHERE id = ?1",
    )?
    .execute(params![
        id,
        u.workdir,
        u.kind,
        u.git_branch,
        u.title,
        u.started_at,
        u.last_at,
        u.message_count,
        u.next_seq
    ])?;
    Ok(())
}

/// Metadata after importing a grok-cli session: the source's workdir wins.
#[derive(Debug, Clone, Default)]
pub struct DbSessionUpdate {
    pub workdir: Option<String>,
    pub title: Option<String>,
    pub started_at: Option<i64>,
    /// Last activity: the newest of the last message and the session's `updated_at`.
    pub last_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub message_count: i64,
    pub next_seq: i64,
}

pub fn update_from_db(conn: &Connection, id: &str, u: &DbSessionUpdate) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET workdir = COALESCE(?2, workdir), title = ?3,
                started_at = COALESCE(started_at, ?4), last_at = COALESCE(?5, ?6, last_at),
                message_count = ?7, next_seq = ?8
          WHERE id = ?1",
        params![
            id,
            u.workdir,
            u.title,
            u.started_at,
            u.last_at,
            u.updated_at,
            u.message_count,
            u.next_seq
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_update_reset() {
        let dir = tempfile::tempdir().unwrap();
        let c = crate::storage::open(&dir.path().join("i.sqlite")).unwrap();
        insert(&c, "s1", Provider::Claude, "/a.jsonl").unwrap();
        update_from_file(
            &c,
            "s1",
            &FileSessionUpdate {
                workdir: Some("/w".into()),
                title: Some("t".into()),
                message_count: 2,
                next_seq: 2,
                ..Default::default()
            },
        )
        .unwrap();
        let s = get_session(&c, "s1").unwrap().unwrap();
        assert_eq!(
            (s.workdir.as_deref(), s.message_count, s.next_seq),
            (Some("/w"), 2, 2)
        );
        assert_eq!(stats(&c).unwrap(), (1, 2));
        assert_eq!(
            sessions_by_provider(&c).unwrap(),
            [("claude".to_owned(), 1)]
        );
        reset(&c, "s1").unwrap();
        let s = get_session(&c, "s1").unwrap().unwrap();
        assert_eq!((s.title, s.message_count, s.next_seq), (None, 0, 0));
        assert_eq!(
            get_sessions(&c, &["x".into(), "s1".into()]).unwrap().len(),
            1
        );
    }
}
