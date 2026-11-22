//! `messages` table: indexed user prompts and assistant answers.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::models::search::fold;
use crate::models::time::to_ms;
use crate::models::ParsedMessage;

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: i64,
    pub session_id: String,
    pub seq: i64,
    pub role: String,
    pub at: Option<i64>,
    pub text: String,
}

fn message_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MessageRow> {
    Ok(MessageRow {
        id: r.get(0)?,
        session_id: r.get(1)?,
        seq: r.get(2)?,
        role: r.get(3)?,
        at: r.get(4)?,
        text: r.get(5)?,
    })
}

/// Messages containing all `terms_lc` (already lowercased), freshest first.
pub fn find(
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
    let rows = stmt.query_map(args.as_slice(), message_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Messages of a session with `seq` in `from..=to`.
pub fn range(conn: &Connection, session_id: &str, from: i64, to: i64) -> Result<Vec<MessageRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, session_id, seq, role, at, text FROM messages
          WHERE session_id = ?1 AND seq BETWEEN ?2 AND ?3 ORDER BY seq ASC",
    )?;
    let rows = stmt.query_map(params![session_id, from, to], message_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn count(conn: &Connection, session_id: &str) -> Result<i64> {
    Ok(conn
        .prepare_cached("SELECT COUNT(*) FROM messages WHERE session_id = ?1")?
        .query_row([session_id], |r| r.get(0))?)
}

pub fn insert(conn: &Connection, session_id: &str, seq: i64, m: &ParsedMessage) -> Result<()> {
    conn.prepare_cached(
        "INSERT INTO messages (session_id, seq, role, at, text, text_lc, prompt_index)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(params![
        session_id,
        seq,
        m.role.as_str(),
        to_ms(m.at),
        m.text,
        fold(&m.text),
        m.prompt_index
    ])?;
    Ok(())
}

/// Removes messages with `seq >= from` (a provisional tail written by the previous pass).
pub fn delete_from_seq(conn: &Connection, session_id: &str, from: i64) -> Result<()> {
    conn.prepare_cached("DELETE FROM messages WHERE session_id = ?1 AND seq >= ?2")?
        .execute(params![session_id, from])?;
    Ok(())
}

/// Grok rewind: drops messages of prompts after `target`.
pub fn delete_after_prompt(conn: &Connection, session_id: &str, target: i64) -> Result<()> {
    conn.prepare_cached("DELETE FROM messages WHERE session_id = ?1 AND prompt_index > ?2")?
        .execute(params![session_id, target])?;
    Ok(())
}

/// Prompt index of the last stored message that has one.
pub fn last_prompt_index(conn: &Connection, session_id: &str) -> Result<Option<i64>> {
    Ok(conn
        .prepare_cached(
            "SELECT prompt_index FROM messages WHERE session_id = ?1
               AND prompt_index IS NOT NULL ORDER BY seq DESC LIMIT 1",
        )?
        .query_row([session_id], |r| r.get(0))
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Provider, Role};

    fn msg(text: &str, prompt_index: Option<i64>) -> ParsedMessage {
        ParsedMessage {
            role: Role::User,
            at: None,
            text: text.into(),
            prompt_index,
        }
    }

    #[test]
    fn insert_find_and_trim() {
        let dir = tempfile::tempdir().unwrap();
        let c = crate::storage::open(&dir.path().join("i.sqlite")).unwrap();
        crate::storage::sessions::insert(&c, "s", Provider::Grok, "/u.jsonl").unwrap();
        insert(&c, "s", 0, &msg("Почини ДЕПЛОЙ", Some(0))).unwrap();
        insert(&c, "s", 1, &msg("второй", Some(1))).unwrap();
        insert(&c, "s", 2, &msg("третий", Some(2))).unwrap();
        let found = find(&c, &["деплой".into()], Some("grok"), true, 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "Почини ДЕПЛОЙ");
        assert_eq!(last_prompt_index(&c, "s").unwrap(), Some(2));
        delete_after_prompt(&c, "s", 1).unwrap();
        assert_eq!(count(&c, "s").unwrap(), 2);
        delete_from_seq(&c, "s", 1).unwrap();
        assert_eq!(range(&c, "s", 0, 5).unwrap().len(), 1);
    }
}
