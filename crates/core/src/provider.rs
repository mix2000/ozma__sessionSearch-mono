//! Transcript sources: Claude Code, Codex CLI, Grok CLI.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::parse::{self, LineParser, ParsedLine};

/// Stateless line parsers (Claude, Codex): byte pre-filter + JSON parse.
struct Simple {
    wanted: fn(&[u8]) -> bool,
    parse: fn(&str) -> Vec<ParsedLine>,
}

impl LineParser for Simple {
    fn feed(&mut self, line: &[u8], _start: u64) -> Vec<ParsedLine> {
        if !(self.wanted)(line) {
            return vec![];
        }
        (self.parse)(&String::from_utf8_lossy(line))
    }
}

/// grok-cli database file name under the grok root.
pub const GROK_DB: &str = "grok.db";

/// Whether a session's source is grok-cli's database.
pub fn is_grok_db(file_path: &str) -> bool {
    Path::new(file_path).file_name().and_then(|n| n.to_str()) == Some(GROK_DB)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
    Grok,
}

impl Provider {
    pub const ALL: [Provider; 3] = [Provider::Claude, Provider::Codex, Provider::Grok];

    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
            Provider::Grok => "grok",
        }
    }

    /// A fresh parser for one file of this provider.
    pub fn parser(self) -> Box<dyn LineParser> {
        match self {
            Provider::Claude => Box::new(Simple {
                wanted: parse::claude::line_wanted,
                parse: parse::claude::parse_line,
            }),
            Provider::Codex => Box::new(Simple {
                wanted: parse::codex::line_wanted,
                parse: parse::codex::parse_line,
            }),
            Provider::Grok => Box::new(parse::grok::UpdatesParser::default()),
        }
    }

    /// Program and arguments that resume conversation `id` (run in its
    /// workdir). Grok sessions imported from grok-cli's `grok.db` use `-s`.
    pub fn resume_argv(self, id: &str, file_path: &str) -> Vec<String> {
        match self {
            Provider::Claude => vec!["claude".into(), "--resume".into(), id.into()],
            Provider::Codex => vec!["codex".into(), "resume".into(), id.into()],
            Provider::Grok if is_grok_db(file_path) => {
                vec!["grok".into(), "-s".into(), id.into()]
            }
            Provider::Grok => vec!["grok".into(), "--resume".into(), id.into()],
        }
    }

    /// Words that identify this agent's process on a command line.
    pub fn process_markers(self) -> &'static [&'static str] {
        match self {
            Provider::Claude => &["claude"],
            Provider::Codex => &["codex"],
            Provider::Grok => &["grok"],
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "claude" => Ok(Provider::Claude),
            "codex" => Ok(Provider::Codex),
            "grok" => Ok(Provider::Grok),
            other => Err(format!("unknown provider: {other}")),
        }
    }
}

/// Where each provider keeps its transcripts.
#[derive(Debug, Clone)]
pub struct Roots {
    pub claude: PathBuf,
    pub codex: PathBuf,
    pub grok: PathBuf,
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

impl Roots {
    /// Defaults: `$CLAUDE_CONFIG_DIR/projects` or `~/.claude/projects`;
    /// `$CODEX_HOME/sessions` or `~/.codex/sessions`; grok — `$GROK_HOME` or
    /// `~/.grok` (Grok Build sessions in `sessions/`, grok-cli in `grok.db`). Each can be
    /// overridden with `SESSION_SEARCH_{CLAUDE,CODEX,GROK}_ROOT`.
    pub fn from_env() -> Self {
        let claude = env_path("SESSION_SEARCH_CLAUDE_ROOT").unwrap_or_else(|| {
            env_path("CLAUDE_CONFIG_DIR")
                .unwrap_or_else(|| home().join(".claude"))
                .join("projects")
        });
        let codex = env_path("SESSION_SEARCH_CODEX_ROOT").unwrap_or_else(|| {
            env_path("CODEX_HOME")
                .unwrap_or_else(|| home().join(".codex"))
                .join("sessions")
        });
        let grok = env_path("SESSION_SEARCH_GROK_ROOT")
            .unwrap_or_else(|| env_path("GROK_HOME").unwrap_or_else(|| home().join(".grok")));
        Roots {
            claude,
            codex,
            grok,
        }
    }

    pub fn get(&self, p: Provider) -> &Path {
        match p {
            Provider::Claude => &self.claude,
            Provider::Codex => &self.codex,
            Provider::Grok => &self.grok,
        }
    }
}

/// A transcript file found on disk.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub provider: Provider,
    pub path: PathBuf,
    pub size: u64,
    pub mtime_ms: i64,
    /// Id from the file name (Codex refines it from `session_meta`).
    pub id: String,
    /// Metadata known before reading the file (Grok Build `summary.json`).
    pub meta: Vec<ParsedLine>,
}

fn stat(path: &Path) -> Option<(u64, i64)> {
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

fn read_dir_sorted(dir: &Path) -> Vec<fs::DirEntry> {
    let mut v: Vec<_> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(Result::ok).collect(),
        Err(_) => return vec![],
    };
    v.sort_by_key(|e| e.file_name());
    v
}

/// Claude: `<root>/<project folder>/<uuid>.jsonl`. Subagent transcripts live in
/// deeper folders and are not picked up.
fn claude_files(root: &Path) -> Vec<SourceFile> {
    let mut out = Vec::new();
    for dir in read_dir_sorted(root) {
        if !dir.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        for entry in read_dir_sorted(&dir.path()) {
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(parse::claude::id_from_file) else {
                continue;
            };
            let path = entry.path();
            if let Some((size, mtime_ms)) = stat(&path) {
                out.push(SourceFile {
                    provider: Provider::Claude,
                    path,
                    size,
                    mtime_ms,
                    id,
                    meta: Vec::new(),
                });
            }
        }
    }
    out
}

fn walk_jsonl(
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
            if let (Some(id), Some((size, mtime_ms))) = (id_of(name), stat(&path)) {
                out.push(SourceFile {
                    provider,
                    path,
                    size,
                    mtime_ms,
                    id,
                    meta: Vec::new(),
                });
            }
        }
    }
}

/// All transcript files of a provider under `root`.
pub fn discover(provider: Provider, root: &Path) -> Vec<SourceFile> {
    match provider {
        Provider::Claude => claude_files(root),
        Provider::Codex => {
            let mut out = Vec::new();
            walk_jsonl(provider, root, 0, 4, parse::codex::id_from_file, &mut out);
            out
        }
        Provider::Grok => grok_build_files(root),
    }
}

/// Grok Build: `<root>/sessions/<encoded cwd>/<session id>/updates.jsonl` with
/// `summary.json` next to it. Hidden and subagent sessions are skipped. The
/// file's mtime also covers `summary.json`, so title changes are picked up.
fn grok_build_files(root: &Path) -> Vec<SourceFile> {
    let mut out = Vec::new();
    for group in read_dir_sorted(&root.join("sessions")) {
        if !group.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let group_path = group.path();
        let group_cwd = group
            .file_name()
            .to_str()
            .and_then(parse::grok::decode_cwd)
            .or_else(|| {
                fs::read_to_string(group_path.join(".cwd"))
                    .ok()
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
            });
        for session in read_dir_sorted(&group_path) {
            if !session.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let dir = session.path();
            let path = dir.join("updates.jsonl");
            let Some((size, mut mtime_ms)) = stat(&path) else {
                continue;
            };
            let summary_path = dir.join("summary.json");
            let summary = fs::read_to_string(&summary_path)
                .ok()
                .and_then(|s| parse::grok::parse_summary(&s))
                .unwrap_or_default();
            if summary.skipped() {
                continue;
            }
            if let Some((_, m)) = stat(&summary_path) {
                mtime_ms = mtime_ms.max(m);
            }
            let id = summary
                .id
                .clone()
                .or_else(|| session.file_name().to_str().map(str::to_owned))
                .filter(|id| crate::terminal::valid_id(id));
            let Some(id) = id else { continue };
            let meta = summary.meta(&id, group_cwd.clone());
            out.push(SourceFile {
                provider: Provider::Grok,
                path,
                size,
                mtime_ms,
                id,
                meta,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_round_trip() {
        for p in Provider::ALL {
            assert_eq!(p.as_str().parse::<Provider>().unwrap(), p);
            assert_eq!(
                serde_json::to_value(p).unwrap(),
                serde_json::json!(p.as_str())
            );
        }
        assert!("x".parse::<Provider>().is_err());
    }

    #[test]
    fn resume_commands() {
        let a = |p: Provider, f: &str| p.resume_argv("abc", f).join(" ");
        assert_eq!(a(Provider::Claude, "/x.jsonl"), "claude --resume abc");
        assert_eq!(a(Provider::Codex, "/x.jsonl"), "codex resume abc");
        assert_eq!(
            a(Provider::Grok, "/h/.grok/sessions/g/abc/updates.jsonl"),
            "grok --resume abc"
        );
        assert_eq!(a(Provider::Grok, "/h/.grok/grok.db"), "grok -s abc");
    }
}
