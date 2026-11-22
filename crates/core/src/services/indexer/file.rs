//! Indexing the new tail of one transcript file. Transcript files are
//! append-only, so only the part after the stored byte offset is read. A file
//! that got shorter is reindexed.

use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use super::meta::{collect, note_at, title_from, Meta};
use crate::models::time::to_ms;
use crate::models::ParsedMessage;
use crate::sources::fs::is_file;
use crate::sources::lines::read_lines;
use crate::sources::{parser_for, SourceFile};
use crate::storage::files::{self, FileRow, FileState};
use crate::storage::messages;
use crate::storage::sessions::{self, FileSessionUpdate, SessionRow};

fn upsert_file(
    conn: &Connection,
    src: &SourceFile,
    session_id: &str,
    offset: u64,
    tail_seq: Option<i64>,
) -> Result<()> {
    files::upsert(
        conn,
        &FileRow {
            path: &src.path.to_string_lossy(),
            provider: src.provider,
            session_id: Some(session_id),
            size: src.size,
            offset,
            mtime: src.mtime_ms,
            tail_seq,
        },
    )
}

/// Removes the provisional tail message(s) written by the previous pass.
fn drop_from(conn: &Connection, id: &str, seq: i64) -> Result<()> {
    messages::delete_from_seq(conn, id, seq)?;
    sessions::set_next_seq(conn, id, seq)
}

/// A session already linked to another file: when that file still exists this
/// one is a copy/fork and is ignored (`None`); otherwise the file was moved and
/// the session is reindexed from the new path.
fn claim(conn: &Connection, session: SessionRow, path: &str) -> Result<Option<SessionRow>> {
    if session.file_path == path {
        return Ok(Some(session));
    }
    if is_file(Path::new(&session.file_path)) {
        return Ok(None);
    }
    sessions::reset(conn, &session.id)?;
    sessions::set_file_path(conn, &session.id, path)?;
    sessions::get_session(conn, &session.id)
}

/// Where reading resumes for a file already linked to a session.
struct Resume {
    session: Option<SessionRow>,
    offset: u64,
    /// Messages counted before this pass (a re-read provisional tail is not "new").
    count_before: i64,
}

/// `None`: the file is a copy of another live transcript and was only recorded.
fn prepare(
    conn: &Connection,
    src: &SourceFile,
    state: Option<&FileState>,
) -> Result<Option<Resume>> {
    let path_str = src.path.to_string_lossy();
    let Some(s) = (match state.and_then(|s| s.session_id.as_deref()) {
        Some(id) => sessions::get_session(conn, id)?,
        None => None,
    }) else {
        return Ok(Some(Resume {
            session: None,
            offset: 0,
            count_before: 0,
        }));
    };
    let mut count_before = messages::count(conn, &s.id)?;
    let mut offset = state.map_or(0, |s| s.offset);
    let (id, before) = (s.id.clone(), s.next_seq);
    let Some(s) = claim(conn, s, &path_str)? else {
        upsert_file(conn, src, &id, src.size, None)?;
        return Ok(None);
    };
    if s.next_seq != before || src.size < offset {
        if src.size < offset {
            // The file was rewritten — start over.
            sessions::reset(conn, &s.id)?;
        }
        offset = 0;
        count_before = 0;
    } else if let Some(t) = state.and_then(|st| st.tail_seq) {
        drop_from(conn, &s.id, t)?;
    }
    let session = sessions::get_session(conn, &s.id)?;
    Ok(Some(Resume {
        session,
        offset,
        count_before,
    }))
}

/// Records parsed from the new part of a file.
struct Parsed {
    meta: Meta,
    msgs: Vec<ParsedMessage>,
    rewinds: Vec<i64>,
    /// A message still being written, with the offset where it starts.
    tail: Option<(ParsedMessage, u64)>,
    /// Offset after the last complete line.
    processed: u64,
}

fn parse(
    conn: &Connection,
    src: &SourceFile,
    session: Option<&SessionRow>,
    offset: u64,
) -> Result<Parsed> {
    let mut meta = Meta::default();
    let mut msgs: Vec<ParsedMessage> = Vec::new();
    let mut rewinds: Vec<i64> = Vec::new();
    collect(src.meta.clone(), &mut meta, &mut msgs, &mut rewinds);
    let mut parser = parser_for(src.provider);
    if let Some(s) = session {
        if let Some(p) = messages::last_prompt_index(conn, &s.id)? {
            parser.resume_prompt(p);
        }
    }
    let processed = read_lines(&src.path, offset, |line, start| {
        collect(parser.feed(line, start), &mut meta, &mut msgs, &mut rewinds);
    })?;
    let tail = parser.finish();
    Ok(Parsed {
        meta,
        msgs,
        rewinds,
        tail,
        processed,
    })
}

/// The session for a file read from the start; `None` when it belongs to another live file.
fn resolve_session(conn: &Connection, src: &SourceFile, meta: &Meta) -> Result<Option<SessionRow>> {
    let path_str = src.path.to_string_lossy();
    let id = meta.id.clone().unwrap_or_else(|| src.id.clone());
    let Some(existing) = sessions::get_session(conn, &id)? else {
        sessions::insert(conn, &id, src.provider, &path_str)?;
        return Ok(Some(
            sessions::get_session(conn, &id)?.expect("session inserted"),
        ));
    };
    match claim(conn, existing, &path_str)? {
        // The whole file was read from the start: replace old messages.
        Some(s) if s.next_seq > 0 => {
            sessions::reset(conn, &s.id)?;
            Ok(Some(
                sessions::get_session(conn, &s.id)?.expect("session exists"),
            ))
        }
        Some(s) => Ok(Some(s)),
        None => {
            upsert_file(conn, src, &id, src.size, None)?;
            Ok(None)
        }
    }
}

/// Writes messages and session metadata; returns the stored message count.
fn persist(conn: &Connection, src: &SourceFile, session: &SessionRow, p: Parsed) -> Result<i64> {
    let Parsed {
        mut meta,
        msgs,
        rewinds,
        tail,
        processed,
    } = p;
    // Rewinds seen in this pass also cut messages indexed earlier.
    for target in &rewinds {
        messages::delete_after_prompt(conn, &session.id, *target)?;
    }
    let mut seq = session.next_seq;
    for m in &msgs {
        messages::insert(conn, &session.id, seq, m)?;
        seq += 1;
    }
    let (stored_offset, tail_seq) = match &tail {
        Some((m, start)) => {
            note_at(&mut meta, m.at);
            messages::insert(conn, &session.id, seq, m)?;
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
    let count = messages::count(conn, &session.id)?;
    let update = FileSessionUpdate {
        workdir: meta.cwd,
        kind: meta.kind,
        git_branch: meta.git_branch,
        title,
        started_at: to_ms(meta.started_at),
        last_at: to_ms(meta.last_at),
        message_count: count,
        next_seq: seq,
    };
    sessions::update_from_file(conn, &session.id, &update)?;
    upsert_file(conn, src, &session.id, stored_offset, tail_seq)?;
    Ok(count)
}

/// Indexes the new tail of one file inside the caller's transaction.
/// Returns the number of added messages.
pub fn index_file(conn: &Connection, src: &SourceFile, state: Option<&FileState>) -> Result<usize> {
    let Some(resume) = prepare(conn, src, state)? else {
        return Ok(0);
    };
    let parsed = parse(conn, src, resume.session.as_ref(), resume.offset)?;
    let session = match resume.session {
        Some(s) => s,
        None => match resolve_session(conn, src, &parsed.meta)? {
            Some(s) => s,
            None => return Ok(0),
        },
    };
    let count = persist(conn, src, &session, parsed)?;
    Ok((count - resume.count_before).max(0) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Provider;
    use crate::sources::fs::source_file;
    use std::io::Write;

    const ID: &str = "11111111-2222-3333-4444-555555555555";

    fn line(role: &str, text: &str) -> String {
        format!(
            "{}\n",
            serde_json::json!({"type": role, "entrypoint": "cli", "cwd": "/w",
                "timestamp": "2026-11-10T08:55:37.856Z", "message": {"content": text}})
        )
    }

    fn pass(conn: &Connection, path: &Path) -> usize {
        let src = source_file(Provider::Claude, path.to_path_buf(), ID.into()).unwrap();
        let states = files::file_states(conn).unwrap();
        index_file(conn, &src, states.get(&*path.to_string_lossy())).unwrap()
    }

    #[test]
    fn incremental_tail_and_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::storage::open(&dir.path().join("i.sqlite")).unwrap();
        let p = dir.path().join(format!("{ID}.jsonl"));
        std::fs::write(&p, line("user", "первый вопрос")).unwrap();
        assert_eq!(pass(&conn, &p), 1);
        let s = sessions::get_session(&conn, ID).unwrap().unwrap();
        assert_eq!(
            (s.title.as_deref(), s.workdir.as_deref()),
            (Some("первый вопрос"), Some("/w"))
        );

        std::fs::OpenOptions::new()
            .append(true)
            .open(&p)
            .unwrap()
            .write_all(line("assistant", "ответ").as_bytes())
            .unwrap();
        assert_eq!(pass(&conn, &p), 1);
        assert_eq!(messages::count(&conn, ID).unwrap(), 2);

        // Shorter file: reindexed from scratch.
        std::fs::write(&p, line("user", "заново")).unwrap();
        pass(&conn, &p);
        let rows = messages::range(&conn, ID, 0, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "заново");
    }

    #[test]
    fn copy_of_a_live_file_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::storage::open(&dir.path().join("i.sqlite")).unwrap();
        let a = dir.path().join("a").join(format!("{ID}.jsonl"));
        let b = dir.path().join("b").join(format!("{ID}.jsonl"));
        for p in [&a, &b] {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, line("user", "одно и то же")).unwrap();
        }
        assert_eq!(pass(&conn, &a), 1);
        assert_eq!(pass(&conn, &b), 0);
        let s = sessions::get_session(&conn, ID).unwrap().unwrap();
        assert_eq!(s.file_path, a.to_string_lossy());
        // The original is gone: the session moves to the copy.
        std::fs::remove_file(&a).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&b)
            .unwrap()
            .write_all(line("assistant", "ещё").as_bytes())
            .unwrap();
        pass(&conn, &b);
        let s = sessions::get_session(&conn, ID).unwrap().unwrap();
        assert_eq!(
            (s.file_path, s.message_count),
            (b.to_string_lossy().into_owned(), 2)
        );
    }
}
