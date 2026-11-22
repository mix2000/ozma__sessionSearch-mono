//! OS locations: home and data folders, transcript roots from the environment.

use std::path::PathBuf;

use crate::models::Roots;

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Default data dir: `<OS data dir>/ozma-session-search`.
pub fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| ".".into()))
        .join("ozma-session-search")
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_absolute_or_relative_to_home() {
        assert!(default_data_dir().ends_with("ozma-session-search"));
        let r = Roots::from_env();
        assert!(!r.claude.as_os_str().is_empty());
        assert!(!r.codex.as_os_str().is_empty());
        assert!(!r.grok.as_os_str().is_empty());
    }
}
