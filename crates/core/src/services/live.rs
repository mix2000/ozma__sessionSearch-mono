//! Which conversations are currently open in some process (best effort).
//!
//! Two sources: command lines of running agent processes that contain a session
//! id (`claude --resume <id>`, `codex resume <id>`), and the Claude Code registry
//! `<claude config>/sessions/<pid>.json` (`{pid, sessionId}`) for sessions that
//! were started without an id on the command line.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;

use super::agents::process_markers;
use crate::models::Provider;
use crate::platform::{ProcessSnapshot, ProcessSource};
use crate::sources::registry::SessionRegistry;

const TTL: Duration = Duration::from_secs(5);

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("valid regex")
});

/// session id -> pid
pub type LiveMap = HashMap<String, u32>;

pub struct LiveCache {
    source: Box<dyn ProcessSource>,
    registry: Box<dyn SessionRegistry>,
    cache: Mutex<Option<(Instant, LiveMap)>>,
}

impl LiveCache {
    pub fn new(source: Box<dyn ProcessSource>, registry: Box<dyn SessionRegistry>) -> Self {
        LiveCache {
            source,
            registry,
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
        let map = match self.source.snapshot() {
            Some(snap) => live_map(&snap, &self.registry.entries()),
            None => LiveMap::new(),
        };
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
            .flat_map(|&p| process_markers(p))
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

/// Live sessions from a process snapshot and Claude registry entries
/// (`(pid, session id)`); registry entries count only while the pid runs.
/// The first process that mentions a session wins.
pub fn live_map(snap: &ProcessSnapshot, registry: &[(u32, String)]) -> LiveMap {
    let mut out = LiveMap::new();
    for (pid, args) in &snap.processes {
        for id in ids_from_cmdline(args) {
            out.entry(id).or_insert(*pid);
        }
    }
    for (pid, sid) in registry {
        if snap.alive.contains(pid) {
            out.entry(sid.to_lowercase()).or_insert(*pid);
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
    fn map_from_processes_and_registry() {
        let id = "0A1B2C3D-1111-2222-3333-444455556666";
        let snap = ProcessSnapshot {
            processes: vec![(10, v(&["claude", "--resume", id]))],
            alive: [10, 20].into_iter().collect(),
        };
        let registry = vec![
            (20, "Other-Session".to_owned()),
            (30, "dead".to_owned()),
            (20, id.to_owned()),
        ];
        let map = live_map(&snap, &registry);
        assert_eq!(map.len(), 2);
        assert_eq!(map[&id.to_lowercase()], 10);
        assert_eq!(map["other-session"], 20);
    }

    struct Fixed(Option<ProcessSnapshot>);
    impl ProcessSource for Fixed {
        fn snapshot(&self) -> Option<ProcessSnapshot> {
            self.0.clone()
        }
    }

    struct Registry(Vec<(u32, String)>);
    impl SessionRegistry for Registry {
        fn entries(&self) -> Vec<(u32, String)> {
            self.0.clone()
        }
    }

    #[test]
    fn cache_uses_source_and_registry() {
        let snap = ProcessSnapshot {
            processes: vec![],
            alive: [1].into_iter().collect(),
        };
        let registry = || Box::new(Registry(vec![(1, "x".into())]));
        let cache = LiveCache::new(Box::new(Fixed(Some(snap))), registry());
        assert_eq!(cache.get()["x"], 1);
        assert_eq!(cache.get().len(), 1);
        let none = LiveCache::new(Box::new(Fixed(None)), registry());
        assert!(none.get().is_empty());
    }

    #[test]
    fn scan_does_not_panic() {
        let cache = LiveCache::new(
            Box::new(crate::platform::OsProcesses),
            Box::new(Registry(vec![])),
        );
        let _ = cache.get();
        let _ = cache.get();
    }
}
