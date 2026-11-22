//! Index database: schema, migrations and connection settings.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_schema_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/index.sqlite");
        drop(open(&p).unwrap());
        let c = open(&p).unwrap();
        assert_eq!(crate::storage::sessions::stats(&c).unwrap(), (0, 0));
    }
}
