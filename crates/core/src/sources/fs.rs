//! File system walking shared by the transcript sources.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::SourceFile;
use crate::models::Provider;

/// Size and mtime (ms) of a regular file.
pub fn stat(path: &Path) -> Option<(u64, i64)> {
    let m = fs::metadata(path).ok()?;
    if !m.is_file() {
        return None;
    }
    let mtime = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64);
    Some((m.len(), mtime))
}

/// Whether `path` is an existing regular file.
pub fn is_file(path: &Path) -> bool {
    path.is_file()
}

/// Directory entries sorted by name; an unreadable directory is empty.
pub fn read_dir_sorted(dir: &Path) -> Vec<fs::DirEntry> {
    let mut v: Vec<_> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(Result::ok).collect(),
        Err(_) => return vec![],
    };
    v.sort_by_key(|e| e.file_name());
    v
}

/// A transcript file found on disk, with its id from the file name.
pub fn source_file(provider: Provider, path: PathBuf, id: String) -> Option<SourceFile> {
    let (size, mtime_ms) = stat(&path)?;
    Some(SourceFile {
        provider,
        path,
        size,
        mtime_ms,
        id,
        meta: Vec::new(),
    })
}

/// `*.jsonl` files under `dir` up to `max_depth` levels deep, sorted by path.
pub fn walk_jsonl(
    provider: Provider,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    id_of: fn(&str) -> Option<String>,
    out: &mut Vec<SourceFile>,
) {
    for e in read_dir_sorted(dir) {
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();
        if ft.is_dir() {
            if depth < max_depth {
                walk_jsonl(provider, &path, depth + 1, max_depth, id_of, out);
            }
        } else if ft.is_file() {
            let name = e.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.ends_with(".jsonl") {
                continue;
            }
            if let Some(id) = id_of(name) {
                out.extend(source_file(provider, path, id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_respects_depth_and_extension() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(dir.path().join("x.jsonl"), "1\n").unwrap();
        std::fs::write(dir.path().join("a/y.jsonl"), "1\n").unwrap();
        std::fs::write(deep.join("z.jsonl"), "1\n").unwrap();
        std::fs::write(dir.path().join("n.txt"), "1\n").unwrap();
        let id = |n: &str| Some(n.to_owned());
        let mut out = Vec::new();
        walk_jsonl(Provider::Codex, dir.path(), 0, 1, id, &mut out);
        let names: Vec<_> = out.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(names, ["y.jsonl", "x.jsonl"]);
        assert_eq!(out[0].size, 2);
        assert!(stat(dir.path()).is_none());
    }
}
