//! Incremental indexing. Transcript files are append-only, so only the new tail
//! (from the stored byte offset) is read. A file that got shorter is reindexed.
//! Sessions whose files were deleted by the agent stay in the index.

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::{self, to_ms, FileState};
use crate::parse::{ParsedLine, ParsedMessage};
use crate::provider::{discover, Provider, Roots, SourceFile};
use crate::query::fold;

/// Longer lines are multi-megabyte tool output without message text.
pub const MAX_LINE: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SyncResult {
    pub files: usize,
    pub updated: usize,
    pub messages: usize,
}

/// Reads complete lines starting at `offset`; `on_line` gets each line and its
/// start offset. Returns the offset right after the last complete line — an
/// unfinished tail is read next time.
pub fn read_lines(
    path: &Path,
    offset: u64,
    mut on_line: impl FnMut(&[u8], u64),
) -> std::io::Result<u64> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut r = BufReader::with_capacity(256 * 1024, f);
    let mut committed = offset;
    let mut consumed = offset;
    let mut line: Vec<u8> = Vec::new();
    let mut skipping = false;
    loop {
        let buf = r.fill_buf()?;
        if buf.is_empty() {
            break;
        }
        match memchr::memchr(b'\n', buf) {
            Some(i) => {
                if !skipping && line.len() + i <= MAX_LINE {
                    line.extend_from_slice(&buf[..i]);
                    if !line.is_empty() {
                        on_line(&line, committed);
                    }
                }
                line.clear();
                skipping = false;
                r.consume(i + 1);
                consumed += i as u64 + 1;
                committed = consumed;
            }
            None => {
                let n = buf.len();
                if !skipping {
                    if line.len() + n > MAX_LINE {
                        skipping = true;
                        line = Vec::new();
                    } else {
                        line.extend_from_slice(buf);
                    }
                }
                r.consume(n);
                consumed += n as u64;
            }
        }
    }
    Ok(committed)
}

#[derive(Default)]
struct Meta {
    id: Option<String>,
    cwd: Option<String>,
    kind: Option<String>,
    git_branch: Option<String>,
    title: Option<String>,
    started_at: Option<DateTime<Utc>>,
    last_at: Option<DateTime<Utc>>,
}

fn note_at(meta: &mut Meta, at: Option<DateTime<Utc>>) {
    if let Some(at) = at {
        meta.started_at.get_or_insert(at);
        meta.last_at = Some(at);
    }
}

fn collect(
    items: Vec<ParsedLine>,
    meta: &mut Meta,
    msgs: &mut Vec<ParsedMessage>,
    rewinds: &mut Vec<i64>,
) {
    for item in items {
        match item {
            ParsedLine::Meta(m) => {
                if meta.id.is_none() {
                    meta.id = m.id;
                }
                if meta.cwd.is_none() {
                    meta.cwd = m.cwd;
                }
                if meta.kind.is_none() {
                    meta.kind = m.kind;
                }
                if m.git_branch.is_some() {
                    meta.git_branch = m.git_branch;
                }
                if m.title.is_some() {
                    meta.title = m.title;
                }
                note_at(meta, m.at);
            }
            ParsedLine::Message(m) => {
                note_at(meta, m.at);
                msgs.push(m);
            }
            ParsedLine::Rewind(target) => {
                msgs.retain(|m| !matches!(m.prompt_index, Some(p) if p > target));
                rewinds.push(target);
            }
        }
    }
}

pub(crate) fn title_from(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsert_file_row(
    conn: &Connection,
    path: &str,
    provider: Provider,
    session_id: Option<&str>,
    size: u64,
    offset: u64,
    mtime: i64,
    tail_seq: Option<i64>,
) -> Result<()> {
    conn.prepare_cached(
        r#"INSERT INTO files (path, provider, session_id, size, "offset", mtime, tail_seq)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
           ON CONFLICT(path) DO UPDATE SET provider = ?2, session_id = ?3, size = ?4,
               "offset" = ?5, mtime = ?6, tail_seq = ?7"#,
    )?
    .execute(params![
        path,
        provider.as_str(),
        session_id,
        size as i64,
        offset as i64,
        mtime,
        tail_seq
    ])?;
    Ok(())
}

fn upsert_file(
    conn: &Connection,
    src: &SourceFile,
    session_id: &str,
    offset: u64,
    tail_seq: Option<i64>,
) -> Result<()> {
    upsert_file_row(
        conn,
        &src.path.to_string_lossy(),
        src.provider,
        Some(session_id),
        src.size,
        offset,
        src.mtime_ms,
        tail_seq,
    )
}

fn reset_session(conn: &Connection, id: &str) -> Result<()> {
    conn.prepare_cached("DELETE FROM messages WHERE session_id = ?1")?
        .execute([id])?;
    conn.prepare_cached(
        "UPDATE sessions SET message_count = 0, next_seq = 0, title = NULL WHERE id = ?1",
    )?
    .execute([id])?;
    Ok(())
}

/// Removes the provisional tail message(s) written by the previous pass.
fn drop_from(conn: &Connection, id: &str, seq: i64) -> Result<()> {
    conn.prepare_cached("DELETE FROM messages WHERE session_id = ?1 AND seq >= ?2")?
        .execute(params![id, seq])?;
    conn.prepare_cached("UPDATE sessions SET next_seq = ?2 WHERE id = ?1")?
        .execute(params![id, seq])?;
    Ok(())
}

pub(crate) fn count_messages(conn: &Connection, id: &str) -> Result<i64> {
    Ok(conn
        .prepare_cached("SELECT COUNT(*) FROM messages WHERE session_id = ?1")?
        .query_row([id], |r| r.get(0))?)
}

pub(crate) fn insert_message(
    conn: &Connection,
    session_id: &str,
    seq: i64,
    m: &ParsedMessage,
) -> Result<()> {
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

/// A session already linked to another file: when that file still exists this
/// one is a copy/fork and is ignored (`None`); otherwise the file was moved and
/// the session is reindexed from the new path.
fn claim(conn: &Connection, session: db::SessionRow, path: &str) -> Result<Option<db::SessionRow>> {
    if session.file_path == path {
        return Ok(Some(session));
    }
    if Path::new(&session.file_path).is_file() {
        return Ok(None);
    }
    reset_session(conn, &session.id)?;
    conn.prepare_cached("UPDATE sessions SET file_path = ?2 WHERE id = ?1")?
        .execute(params![session.id, path])?;
    db::get_session(conn, &session.id)
}

/// Indexes the new tail of one file inside the caller's transaction.
/// Returns the number of added messages.
pub fn index_file(conn: &Connection, src: &SourceFile, state: Option<&FileState>) -> Result<usize> {
    let path_str = src.path.to_string_lossy().into_owned();
    let mut session = match state.and_then(|s| s.session_id.as_deref()) {
        Some(id) => db::get_session(conn, id)?,
        None => None,
    };
    // Counted before dropping a provisional tail, so a re-read tail is not "new".
    let mut count_before = match &session {
        Some(s) => count_messages(conn, &s.id)?,
        None => 0,
    };
    let mut offset = if session.is_some() {
        state.map_or(0, |s| s.offset)
    } else {
        0
    };
    if let Some(s) = session.take() {
        let id = s.id.clone();
        let before = s.next_seq;
        match claim(conn, s, &path_str)? {
            None => {
                upsert_file(conn, src, &id, src.size, None)?;
                return Ok(0);
            }
            Some(s) => {
                if s.next_seq != before || src.size < offset {
                    if src.size < offset {
                        // The file was rewritten — start over.
                        reset_session(conn, &s.id)?;
                    }
                    offset = 0;
                    count_before = 0;
                } else if let Some(t) = state.and_then(|st| st.tail_seq) {
                    drop_from(conn, &s.id, t)?;
                }
                session = db::get_session(conn, &s.id)?;
            }
        }
    }

    let mut meta = Meta::default();
    let mut msgs: Vec<ParsedMessage> = Vec::new();
    let mut rewinds: Vec<i64> = Vec::new();
    collect(src.meta.clone(), &mut meta, &mut msgs, &mut rewinds);
    let mut parser = src.provider.parser();
    if let Some(s) = &session {
        let last_prompt: Option<i64> = conn
            .prepare_cached(
                "SELECT prompt_index FROM messages WHERE session_id = ?1
                   AND prompt_index IS NOT NULL ORDER BY seq DESC LIMIT 1",
            )?
            .query_row([&s.id], |r| r.get(0))
            .optional()?;
        if let Some(p) = last_prompt {
            parser.resume_prompt(p);
        }
    }
    let processed = read_lines(&src.path, offset, |line, start| {
        collect(parser.feed(line, start), &mut meta, &mut msgs, &mut rewinds);
    })?;
    let tail = parser.finish();

    let session = match session {
        Some(s) => s,
        None => {
            let id = meta.id.clone().unwrap_or_else(|| src.id.clone());
            match db::get_session(conn, &id)? {
                Some(existing) => match claim(conn, existing, &path_str)? {
                    // The whole file was read from the start: replace old messages.
                    Some(s) if s.next_seq > 0 => {
                        reset_session(conn, &s.id)?;
                        db::get_session(conn, &s.id)?.expect("session exists")
                    }
                    Some(s) => s,
                    None => {
                        upsert_file(conn, src, &id, src.size, None)?;
                        return Ok(0);
                    }
                },
                None => {
                    conn.prepare_cached(
                        "INSERT INTO sessions (id, provider, file_path) VALUES (?1, ?2, ?3)",
                    )?
                    .execute(params![id, src.provider.as_str(), path_str])?;
                    db::get_session(conn, &id)?.expect("session inserted")
                }
            }
        }
    };

    // Rewinds seen in this pass also cut messages indexed earlier.
    for target in &rewinds {
        conn.prepare_cached("DELETE FROM messages WHERE session_id = ?1 AND prompt_index > ?2")?
            .execute(params![session.id, target])?;
    }
    let mut seq = session.next_seq;
    for m in &msgs {
        insert_message(conn, &session.id, seq, m)?;
        seq += 1;
    }
    let (stored_offset, tail_seq) = match &tail {
        Some((m, start)) => {
            note_at(&mut meta, m.at);
            insert_message(conn, &session.id, seq, m)?;
            seq += 1;
            (*start, Some(seq - 1))
        }
        None => (processed, None),
    };
    let first_user = msgs
        .iter()
        .chain(tail.as_ref().map(|(m, _)| m))
        .find(|m| m.role.as_str() == "user")
        .map(|m| m.text.clone());
    let title = meta
        .title
        .clone()
        .or(session.title.clone())
        .or_else(|| first_user.as_deref().map(title_from));
    let count = count_messages(conn, &session.id)?;
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
        session.id,
        meta.cwd,
        meta.kind,
        meta.git_branch,
        title,
        to_ms(meta.started_at),
        to_ms(meta.last_at),
        count,
        seq
    ])?;
    upsert_file(conn, src, &session.id, stored_offset, tail_seq)?;
    Ok((count - count_before).max(0) as usize)
}

/// One full pass over all providers. Errors in single files are logged and skipped.
/// `stop` is checked between files (graceful shutdown).
pub fn sync(
    conn: &mut Connection,
    roots: &Roots,
    providers: &[Provider],
    stop: &AtomicBool,
) -> Result<SyncResult> {
    let mut files: Vec<SourceFile> = Vec::new();
    for &p in providers {
        files.extend(discover(p, roots.get(p)));
    }
    let known = db::file_states(conn)?;
    let mut res = SyncResult {
        files: files.len(),
        ..Default::default()
    };
    for src in &files {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let state = known.get(&*src.path.to_string_lossy());
        if state.is_some_and(|s| s.size == src.size && s.mtime == src.mtime_ms) {
            continue;
        }
        let tx = conn.transaction()?;
        match index_file(&tx, src, state) {
            Ok(added) => {
                tx.commit()?;
                res.updated += 1;
                res.messages += added;
            }
            Err(err) => {
                drop(tx);
                tracing::warn!("transcript {}: {err:#}", src.path.display());
            }
        }
    }
    if providers.contains(&Provider::Grok) && !stop.load(Ordering::Relaxed) {
        let db_path = roots.grok.join(crate::provider::GROK_DB);
        if db_path.is_file() {
            res.files += 1;
            match crate::grok_db::sync(conn, &db_path, &known) {
                Ok(r) => {
                    res.updated += r.updated;
                    res.messages += r.messages;
                }
                Err(err) => tracing::warn!("grok-cli {}: {err:#}", db_path.display()),
            }
        }
    }
    Ok(res)
}

/// Whether a session row exists (helper for tests and resume).
pub fn session_exists(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM sessions WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn read_lines_stops_at_last_complete_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jsonl");
        std::fs::write(&p, b"one\n\ntwo\nthr").unwrap();
        let mut got = Vec::new();
        let end = read_lines(&p, 0, |l, _| {
            got.push(String::from_utf8(l.to_vec()).unwrap())
        })
        .unwrap();
        assert_eq!(got, ["one", "two"]);
        assert_eq!(end, 9);
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"ee\n").unwrap();
        got.clear();
        let end = read_lines(&p, end, |l, _| {
            got.push(String::from_utf8(l.to_vec()).unwrap())
        })
        .unwrap();
        assert_eq!(got, ["three"]);
        assert_eq!(end, 15);
    }

    #[test]
    fn title_collapses_whitespace_and_clips() {
        assert_eq!(title_from("  a\n\n b\tc "), "a b c");
        assert_eq!(title_from(&"я".repeat(500)).chars().count(), 200);
    }
}
