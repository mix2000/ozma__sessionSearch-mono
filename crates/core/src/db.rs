//! SQLite storage: sessions, messages and per-file indexing state.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
    id            TEXT PRIMARY KEY,
    provider      TEXT NOT NULL,
    file_path     TEXT NOT NULL,
    workdir       TEXT,
    title         TEXT,
    kind          TEXT,
    git_branch    TEXT,
    started_at    INTEGER,
    last_at       INTEGER,
    message_count INTEGER NOT NULL DEFAULT 0,
    -- seq for the next message (messages can be removed by Grok rewinds).
    next_seq      INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS messages (
    id         INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    seq        INTEGER NOT NULL,
    role       TEXT NOT NULL,
    at         INTEGER,
    text       TEXT NOT NULL,
    -- Unicode lowercase computed in Rust (SQLite LOWER is ASCII-only).
    text_lc    TEXT NOT NULL,
    -- Grok: prompt index, to drop rewound branches.
    prompt_index INTEGER
);
CREATE INDEX IF NOT EXISTS messages_session_seq ON messages(session_id, seq);
CREATE TABLE IF NOT EXISTS files (
    path       TEXT PRIMARY KEY,
    provider   TEXT NOT NULL,
    session_id TEXT,
    size       INTEGER NOT NULL,
    "offset"   INTEGER NOT NULL,
    mtime      INTEGER NOT NULL,
    -- seq of a provisional last message (still being streamed); it is
    -- deleted and re-read from "offset" on the next pass.
    tail_seq   INTEGER
);
"#;

/// Opens (and creates/migrates) the index database.
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        anyhow::bail!(
            "index {} was created by a newer version (schema {version})",
            path.display()
        );
    }
    if version == 1 {
        conn.execute_batch(
            "ALTER TABLE sessions ADD COLUMN next_seq INTEGER NOT NULL DEFAULT 0;
             UPDATE sessions SET next_seq = message_count;
             ALTER TABLE messages ADD COLUMN prompt_index INTEGER;
             ALTER TABLE files ADD COLUMN tail_seq INTEGER;",
        )?;
    }
    conn.execute_batch(SCHEMA)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(conn)
}

pub fn to_ms(d: Option<DateTime<Utc>>) -> Option<i64> {
    d.map(|d| d.timestamp_millis())
}

/// ISO string exactly like JavaScript `Date#toISOString`.
pub fn iso(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|d| d.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_default()
}

pub fn iso_opt(ms: Option<i64>) -> Option<String> {
    ms.map(iso)
}

#[derive(Debug, Clone)]
pub struct FileState {
    pub session_id: Option<String>,
    pub size: u64,
    pub offset: u64,
    pub mtime: i64,
    pub tail_seq: Option<i64>,
}

pub fn file_states(conn: &Connection) -> Result<std::collections::HashMap<String, FileState>> {
    let mut stmt =
        conn.prepare(r#"SELECT path, session_id, size, "offset", mtime, tail_seq FROM files"#)?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            FileState {
                session_id: r.get(1)?,
                size: r.get::<_, i64>(2)? as u64,
                offset: r.get::<_, i64>(3)? as u64,
                mtime: r.get(4)?,
                tail_seq: r.get(5)?,
            },
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

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

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: i64,
    pub session_id: String,
    pub seq: i64,
    pub role: String,
    pub at: Option<i64>,
    pub text: String,
}

/// Messages containing all `terms_lc` (already lowercased), freshest first.
pub fn find_messages(
    conn: &Connection,
    terms_lc: &[String],
    provider: Option<&str>,
    include_headless: bool,
    max_rows: usize,
) -> Result<Vec<MessageRow>> {
    let mut where_: Vec<String> = Vec::new();
    let mut args: Vec<&dyn rusqlite::ToSql> = Vec::new();
    for t in terms_lc {
        args.push(t);
        where_.push(format!("instr(m.text_lc, ?{}) > 0", args.len()));
    }
    if let Some(p) = provider.as_ref() {
        args.push(p);
        where_.push(format!("t.provider = ?{}", args.len()));
    }
    if !include_headless {
        where_.push("(t.kind IS NULL OR t.kind = 'interactive')".into());
    }
    if where_.is_empty() {
        where_.push("1".into());
    }
    let sql = format!(
        "SELECT m.id, m.session_id, m.seq, m.role, m.at, m.text
           FROM messages m JOIN sessions t ON t.id = m.session_id
          WHERE {}
          ORDER BY m.at DESC, m.id DESC
          LIMIT {max_rows}",
        where_.join(" AND ")
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(args.as_slice(), |r| {
        Ok(MessageRow {
            id: r.get(0)?,
            session_id: r.get(1)?,
            seq: r.get(2)?,
            role: r.get(3)?,
            at: r.get(4)?,
            text: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn context(conn: &Connection, session_id: &str, from: i64, to: i64) -> Result<Vec<MessageRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, session_id, seq, role, at, text FROM messages
          WHERE session_id = ?1 AND seq BETWEEN ?2 AND ?3 ORDER BY seq ASC",
    )?;
    let rows = stmt.query_map(params![session_id, from, to], |r| {
        Ok(MessageRow {
            id: r.get(0)?,
            session_id: r.get(1)?,
            seq: r.get(2)?,
            role: r.get(3)?,
            at: r.get(4)?,
            text: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_matches_js() {
        let d: DateTime<Utc> = "2026-11-10T08:55:37.856Z".parse().unwrap();
        assert_eq!(iso(d.timestamp_millis()), "2026-11-10T08:55:37.856Z");
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn open_creates_schema_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/index.sqlite");
        drop(open(&p).unwrap());
        let c = open(&p).unwrap();
        assert_eq!(stats(&c).unwrap(), (0, 0));
    }
}
