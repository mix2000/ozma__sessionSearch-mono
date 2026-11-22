//! Claude Code session registry: `<claude config>/sessions/<pid>.json` with
//! `{pid, sessionId}` for sessions started without an id on the command line.

use std::path::{Path, PathBuf};

use crate::models::Roots;

/// Sessions that some process registered as open (replaceable in tests).
pub trait SessionRegistry: Send + Sync {
    /// `(pid, session id)` pairs; the pid may no longer run.
    fn entries(&self) -> Vec<(u32, String)>;
}

/// The Claude Code registry next to the projects folder.
pub struct ClaudeRegistry {
    dir: PathBuf,
}

impl ClaudeRegistry {
    pub fn new(dir: PathBuf) -> Self {
        ClaudeRegistry { dir }
    }

    /// `<claude projects>/../sessions`.
    pub fn for_roots(roots: &Roots) -> Self {
        let dir = roots
            .claude
            .parent()
            .map(|p| p.join("sessions"))
            .unwrap_or_else(|| PathBuf::from("sessions"));
        ClaudeRegistry::new(dir)
    }
}

impl SessionRegistry for ClaudeRegistry {
    fn entries(&self) -> Vec<(u32, String)> {
        read_claude_registry(&self.dir)
    }
}

/// `(pid, session id)` of every well-formed registry entry. Missing folder,
/// unreadable or malformed files are skipped.
pub fn read_claude_registry(dir: &Path) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
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
        if let Some(entry) = parse_entry(&text) {
            out.push(entry);
        }
    }
    out
}

/// `{pid, sessionId}` of one registry file.
pub fn parse_entry(text: &str) -> Option<(u32, String)> {
    let v = serde_json::from_str::<serde_json::Value>(text).ok()?;
    let pid = v.get("pid").and_then(|p| p.as_u64())?;
    let sid = v.get("sessionId").and_then(|s| s.as_str())?;
    let pid = u32::try_from(pid).ok()?;
    Some((pid, sid.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_pid_named_well_formed_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("12.json"),
            r#"{"pid":12,"sessionId":"AbC"}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("bad.json"), r#"{"pid":1,"sessionId":"x"}"#).unwrap();
        std::fs::write(dir.path().join("3.json"), "{").unwrap();
        std::fs::write(dir.path().join("4.json"), r#"{"pid":-1,"sessionId":"x"}"#).unwrap();
        assert_eq!(read_claude_registry(dir.path()), [(12, "AbC".to_owned())]);
        assert!(read_claude_registry(&dir.path().join("missing")).is_empty());
        assert_eq!(parse_entry(r#"{"pid":5000000000,"sessionId":"x"}"#), None);
        assert_eq!(ClaudeRegistry::new(dir.path().into()).entries().len(), 1);
    }

    #[test]
    fn registry_dir_is_next_to_projects() {
        let roots = Roots {
            claude: "/h/.claude/projects".into(),
            codex: "/c".into(),
            grok: "/g".into(),
        };
        assert_eq!(
            ClaudeRegistry::for_roots(&roots).dir,
            Path::new("/h/.claude/sessions")
        );
    }
}
