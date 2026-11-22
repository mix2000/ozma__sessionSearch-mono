//! Engine configuration: where transcripts live and how resume opens a terminal.

use std::path::{Path, PathBuf};

use super::provider::Provider;

/// Where each provider keeps its transcripts. Defaults from the environment:
/// [`Roots::from_env`] (in `platform::paths`).
#[derive(Debug, Clone)]
pub struct Roots {
    pub claude: PathBuf,
    pub codex: PathBuf,
    pub grok: PathBuf,
}

impl Roots {
    pub fn get(&self, p: Provider) -> &Path {
        match p {
            Provider::Claude => &self.claude,
            Provider::Codex => &self.codex,
            Provider::Grok => &self.grok,
        }
    }
}

/// Which macOS terminal app to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MacTerminal {
    #[default]
    Terminal,
    ITerm,
}

impl std::str::FromStr for MacTerminal {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "" | "terminal" | "terminal.app" => Ok(MacTerminal::Terminal),
            "iterm" | "iterm2" | "iterm.app" => Ok(MacTerminal::ITerm),
            other => Err(format!(
                "unknown terminal: {other} (expected terminal|iterm)"
            )),
        }
    }
}

/// The OS a command or launch plan is built for (the current one comes from
/// `platform::current_os`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Mac,
    Windows,
    Linux,
}

pub struct EngineConfig {
    pub db_path: PathBuf,
    pub roots: Roots,
    pub providers: Vec<Provider>,
    pub mac_terminal: MacTerminal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_terminal_names() {
        assert_eq!("".parse::<MacTerminal>(), Ok(MacTerminal::Terminal));
        assert_eq!("iTerm2".parse::<MacTerminal>(), Ok(MacTerminal::ITerm));
        assert!("xterm".parse::<MacTerminal>().is_err());
    }

    #[test]
    fn roots_by_provider() {
        let r = Roots {
            claude: "/c".into(),
            codex: "/x".into(),
            grok: "/g".into(),
        };
        assert_eq!(r.get(Provider::Codex), Path::new("/x"));
        assert_eq!(r.get(Provider::Grok), Path::new("/g"));
    }
}
