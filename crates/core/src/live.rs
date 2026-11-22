//! Which conversations are currently open in some process (best effort).
//!
//! Two sources: command lines of running agent processes that contain a session
//! id (`claude --resume <id>`, `codex resume <id>`), and the Claude Code registry
//! `<claude config>/sessions/<pid>.json` (`{pid, sessionId}`) for sessions that
//! were started without an id on the command line.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::provider::Provider;

const TTL: Duration = Duration::from_secs(5);

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("valid regex")
});

/// session id -> pid
pub type LiveMap = HashMap<String, u32>;

pub struct LiveCache {
    claude_sessions_dir: PathBuf,
    cache: Mutex<Option<(Instant, LiveMap)>>,
}

impl LiveCache {
    pub fn new(claude_sessions_dir: PathBuf) -> Self {
        LiveCache {
            claude_sessions_dir,
            cache: Mutex::new(None),
        }
    }

    /// Current map, rescanned at most every few seconds.
    pub fn get(&self) -> LiveMap {
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, map)) = guard.as_ref() {
            if at.elapsed() < TTL {
                return map.clone();
            }
        }
        let map = scan(&self.claude_sessions_dir);
        *guard = Some((Instant::now(), map.clone()));
        map
    }
}

/// Session ids on the command line of an agent process. The program (or the
/// script run by node/bun/python) must be the agent itself, and the id must be a
/// whole argument (`--resume <id>`, `--resume=<id>`, `resume <id>`), so a shell
/// whose command text merely mentions an id does not count.
pub fn ids_from_cmdline(args: &[String]) -> Vec<String> {
    let is_agent = args.iter().take(2).any(|a| {
        let base = a.rsplit(['/', '\\']).next().unwrap_or(a).to_lowercase();
        Provider::ALL
            .iter()
            .flat_map(|p| p.process_markers())
            .any(|m| base.starts_with(m))
    });
    if !is_agent {
        return vec![];
    }
    args.iter()
        .skip(1)
        .filter_map(|a| {
            let v = a.rsplit_once('=').map_or(a.as_str(), |(_, v)| v);
            UUID_RE.is_match(v).then(|| v.to_lowercase())
        })
        .collect()
}

fn scan(claude_sessions_dir: &Path) -> LiveMap {
    let mut out = LiveMap::new();
    let Ok(mut sys) = System::new() else {
        return out;
    };
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    let me = std::process::id();
    for (pid, proc_) in sys.processes() {
        let pid = pid.as_u32();
        if pid == me {
            continue;
        }
        let mut args: Vec<String> = proc_
            .cmd()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        if args.is_empty() {
            args.push(proc_.name().to_string_lossy().into_owned());
        }
        for id in ids_from_cmdline(&args) {
            out.entry(id).or_insert(pid);
        }
    }
    if let Ok(entries) = std::fs::read_dir(claude_sessions_dir) {
        for e in entries.flatten() {
            let name = e.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(stem) = name.strip_suffix(".json") else {
                continue;
            };
            if stem.is_empty() || !stem.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(e.path()) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let (Some(pid), Some(sid)) = (
                v.get("pid").and_then(|p| p.as_u64()),
                v.get("sessionId").and_then(|s| s.as_str()),
            ) else {
                continue;
            };
            let Ok(pid) = u32::try_from(pid) else {
                continue;
            };
            if sys.process(Pid::from_u32(pid)).is_some() {
                out.entry(sid.to_lowercase()).or_insert(pid);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn ids_only_from_agent_processes() {
        let id = "0a1b2c3d-1111-2222-3333-444455556666";
        assert_eq!(
            ids_from_cmdline(&v(&["/usr/local/bin/claude", "--resume", id])),
            [id]
        );
        assert_eq!(
            ids_from_cmdline(&v(&["node", "/opt/codex/bin/codex.js", "resume", id])),
            [id]
        );
        assert_eq!(
            ids_from_cmdline(&v(&["claude.exe", &format!("--resume={id}")])),
            [id]
        );
        assert!(ids_from_cmdline(&v(&["vim", id])).is_empty());
        let shell_text = format!("echo claude --resume {id}");
        assert!(ids_from_cmdline(&v(&["/bin/zsh", "-c", &shell_text])).is_empty());
        assert!(ids_from_cmdline(&v(&["claude", &format!("x{id}")])).is_empty());
    }

    #[test]
    fn scan_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("1.json"), r#"{"pid":1,"sessionId":"x"}"#).unwrap();
        std::fs::write(dir.path().join("bad.json"), "{").unwrap();
        let cache = LiveCache::new(dir.path().to_path_buf());
        let _ = cache.get();
        let _ = cache.get();
    }
}
