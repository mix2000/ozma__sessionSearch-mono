//! Incremental indexing of all transcript sources into the SQLite index.
//! Sessions whose files were deleted by the agent stay in the index.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use rusqlite::Connection;

use crate::models::{Provider, Roots, SyncResult};
use crate::services::command::valid_id;
use crate::sources::fs::is_file;
use crate::sources::grok::cli::GROK_DB;
use crate::sources::{discover, SourceFile};
use crate::storage::files::file_states;

pub mod file;
pub mod grok_cli;
pub mod meta;

pub use file::index_file;

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
    // Ids end up in resume commands: unsafe ones are not indexed.
    files.retain(|f| valid_id(&f.id));
    let known = file_states(conn)?;
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
        let db_path = roots.grok.join(GROK_DB);
        if is_file(&db_path) {
            res.files += 1;
            match grok_cli::sync(conn, &db_path, &known) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(root: &std::path::Path) -> Roots {
        Roots {
            claude: root.join("claude"),
            codex: root.join("codex"),
            grok: root.join("grok"),
        }
    }

    fn write_claude(root: &std::path::Path, id: &str) {
        let dir = root.join("claude/-w");
        std::fs::create_dir_all(&dir).unwrap();
        let line = serde_json::json!({"type": "user", "message": {"content": "привет"}});
        std::fs::write(dir.join(format!("{id}.jsonl")), format!("{line}\n")).unwrap();
    }

    #[test]
    fn unchanged_files_are_skipped_and_stop_is_honoured() {
        let dir = tempfile::tempdir().unwrap();
        write_claude(dir.path(), "11111111-2222-3333-4444-555555555555");
        write_claude(dir.path(), "21111111-2222-3333-4444-555555555555");
        let mut conn = crate::storage::open(&dir.path().join("i.sqlite")).unwrap();
        let r = roots(dir.path());
        let stop = AtomicBool::new(true);
        let res = sync(&mut conn, &r, &Provider::ALL, &stop).unwrap();
        assert_eq!((res.files, res.updated), (2, 0));
        stop.store(false, Ordering::SeqCst);
        let res = sync(&mut conn, &r, &Provider::ALL, &stop).unwrap();
        assert_eq!(
            res,
            SyncResult {
                files: 2,
                updated: 2,
                messages: 2
            }
        );
        let res = sync(&mut conn, &r, &[Provider::Claude], &stop).unwrap();
        assert_eq!((res.files, res.updated, res.messages), (2, 0, 0));
    }
}
