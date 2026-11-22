//! grok-cli (community `grok-dev`): one SQLite database `<grok root>/grok.db`.
//!
//! The database belongs to grok-cli and is opened strictly read-only (WAL is
//! tolerated). Schema v3: `sessions(id, workspace_id, title, recap_text,
//! cwd_at_start, cwd_last, created_at, updated_at, …)`, `workspaces(id,
//! canonical_path, …)`, `messages(session_id, seq, role, message_json,
//! created_at)`.

use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;

use crate::models::time::to_ms;
use crate::models::{ParsedMessage, Role};
use crate::sources::json::to_date;

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
pub fn db_fingerprint(path: &Path) -> (u64, i64) {
    let (s1, m1) = stat(path);
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    let (s2, m2) = stat(Path::new(&wal));
    (s1 + s2, m1.max(m2))
}

/// A grok-cli session row.
pub struct CliSession {
    pub id: String,
    pub title: Option<String>,
    pub workdir: Option<String>,
    pub created: Option<i64>,
    pub updated: Option<i64>,
    /// `updated` or 0: the change marker stored as the session's mtime.
    pub updated_key: i64,
}

/// Indexable messages of one session read from a source `seq` on.
pub struct CliMessages {
    pub messages: Vec<ParsedMessage>,
    /// Greatest source `seq` seen (including rows without text).
    pub max_seq: Option<i64>,
}

fn text(v: SqlValue) -> Option<String> {
    match v {
        SqlValue::Text(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}

/// grok-cli database file name under the grok root.
pub const GROK_DB: &str = "grok.db";

const MESSAGES_SQL: &str = "SELECT seq, role, message_json, created_at FROM messages
          WHERE session_id = ?1 AND seq >= ?2 ORDER BY seq";

/// A read-only connection to grok-cli's database.
pub struct CliDb {
    conn: Connection,
}

impl CliDb {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("open {} read-only", path.display()))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(CliDb { conn })
    }

    pub fn sessions(&self) -> Result<Vec<CliSession>> {
        let mut stmt = self.conn.prepare(
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

    /// Checks that the messages query can run (fails before any import when
    /// the table is missing).
    pub fn prepare_messages(&self) -> Result<()> {
        self.conn.prepare_cached(MESSAGES_SQL)?;
        Ok(())
    }

    /// Messages of `session_id` with source `seq >= from_seq`, in order.
    pub fn messages(&self, session_id: &str, from_seq: i64) -> Result<CliMessages> {
        let mut stmt = self.conn.prepare_cached(MESSAGES_SQL)?;
        let rows = stmt.query_map(params![session_id, from_seq], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, SqlValue>(3)?,
            ))
        })?;
        let mut out = CliMessages {
            messages: Vec::new(),
            max_seq: None,
        };
        for row in rows {
            let (seq, role, json, created) = row?;
            out.max_seq = Some(out.max_seq.map_or(seq, |m| m.max(seq)));
            if let Some((role, text)) = json.as_deref().and_then(|j| cli_message_text(&role, j)) {
                out.messages.push(ParsedMessage {
                    role,
                    at: cli_time(&created),
                    text,
                    prompt_index: None,
                });
            }
        }
        Ok(out)
    }
}

/// Text of a grok-cli message row (Vercel AI SDK `ModelMessage` JSON): user
/// text parts and assistant `text` parts; system/tool/reasoning are skipped.
pub fn cli_message_text(role: &str, message_json: &str) -> Option<(Role, String)> {
    let role = match role {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        _ => return None,
    };
    let v: Value = serde_json::from_str(message_json).ok()?;
    let content = v.get("content").unwrap_or(&v);
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|p| p.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => return None,
    };
    let text = text.trim();
    (!text.is_empty()).then(|| (role, text.to_owned()))
}

/// Timestamps in grok.db: ISO text, SQLite `YYYY-MM-DD HH:MM:SS`, or unix s/ms.
pub fn cli_time(v: &rusqlite::types::Value) -> Option<DateTime<Utc>> {
    use rusqlite::types::Value as V;
    match v {
        V::Integer(n) => {
            let ms = if n.abs() > 100_000_000_000 {
                *n
            } else {
                n * 1000
            };
            Utc.timestamp_millis_opt(ms).single()
        }
        V::Real(f) => Utc.timestamp_millis_opt((f * 1000.0) as i64).single(),
        V::Text(s) => to_date(Some(&Value::String(s.clone()))).or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
                .ok()
                .map(|n| n.and_utc())
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_rows() {
        assert_eq!(
            cli_message_text("user", r#"{"role":"user","content":"привет"}"#),
            Some((Role::User, "привет".into()))
        );
        assert_eq!(
            cli_message_text(
                "assistant",
                r#"{"role":"assistant","content":[{"type":"reasoning","text":"x"},{"type":"text","text":"ответ"},{"type":"tool-call","toolName":"bash"}]}"#
            ),
            Some((Role::Assistant, "ответ".into()))
        );
        assert_eq!(cli_message_text("tool", r#"{"content":"x"}"#), None);
        assert_eq!(cli_message_text("system", r#"{"content":"x"}"#), None);
        use rusqlite::types::Value as V;
        assert_eq!(
            cli_time(&V::Text("2026-11-01 10:00:00".into())),
            Utc.with_ymd_and_hms(2026, 11, 1, 10, 0, 0).single()
        );
        assert_eq!(
            cli_time(&V::Integer(1_700_000_000)),
            Utc.timestamp_opt(1_700_000_000, 0).single()
        );
    }
}
