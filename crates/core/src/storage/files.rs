//! `files` table: how far each transcript file (or imported database
//! session) has been indexed.

use std::collections::HashMap;

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::models::Provider;

#[derive(Debug, Clone)]
pub struct FileState {
    pub session_id: Option<String>,
    pub size: u64,
    pub offset: u64,
    pub mtime: i64,
    pub tail_seq: Option<i64>,
}

pub fn file_states(conn: &Connection) -> Result<HashMap<String, FileState>> {
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

/// A new value of a `files` row.
#[derive(Debug, Clone)]
pub struct FileRow<'a> {
    pub path: &'a str,
    pub provider: Provider,
    pub session_id: Option<&'a str>,
    pub size: u64,
    pub offset: u64,
    pub mtime: i64,
    pub tail_seq: Option<i64>,
}

pub fn upsert(conn: &Connection, row: &FileRow<'_>) -> Result<()> {
    conn.prepare_cached(
        r#"INSERT INTO files (path, provider, session_id, size, "offset", mtime, tail_seq)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
           ON CONFLICT(path) DO UPDATE SET provider = ?2, session_id = ?3, size = ?4,
               "offset" = ?5, mtime = ?6, tail_seq = ?7"#,
    )?
    .execute(params![
        row.path,
        row.provider.as_str(),
        row.session_id,
        row.size as i64,
        row.offset as i64,
        row.mtime,
        row.tail_seq
    ])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_replaces_state() {
        let dir = tempfile::tempdir().unwrap();
        let c = crate::storage::open(&dir.path().join("i.sqlite")).unwrap();
        let mut row = FileRow {
            path: "/a.jsonl",
            provider: Provider::Claude,
            session_id: Some("s"),
            size: 10,
            offset: 5,
            mtime: 1,
            tail_seq: Some(3),
        };
        upsert(&c, &row).unwrap();
        row.offset = 10;
        row.tail_seq = None;
        upsert(&c, &row).unwrap();
        let st = &file_states(&c).unwrap()["/a.jsonl"];
        assert_eq!(
            (st.size, st.offset, st.mtime, st.tail_seq),
            (10, 10, 1, None)
        );
        assert_eq!(st.session_id.as_deref(), Some("s"));
    }
}
