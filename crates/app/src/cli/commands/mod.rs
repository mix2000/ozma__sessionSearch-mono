//! One module per subcommand, plus opening the index from the common flags.

use anyhow::{Context, Result};
use session_search_core::{
    dedup_providers, default_data_dir, Engine, EngineConfig, MacTerminal, Provider, Roots,
};

use super::args::Common;

pub mod search;
pub mod serve;
pub mod status;
pub mod sync;

/// Transcript roots: environment defaults overridden by flags.
fn roots(c: &Common) -> Roots {
    let mut roots = Roots::from_env();
    if let Some(p) = &c.claude_root {
        roots.claude = p.clone();
    }
    if let Some(p) = &c.codex_root {
        roots.codex = p.clone();
    }
    if let Some(p) = &c.grok_root {
        roots.grok = p.clone();
    }
    roots
}

/// All providers unless `--providers` narrows them.
fn providers(c: &Common) -> Vec<Provider> {
    if c.providers.is_empty() {
        Provider::ALL.to_vec()
    } else {
        dedup_providers(&c.providers)
    }
}

pub fn open_engine(c: &Common, terminal: MacTerminal) -> Result<Engine> {
    let data_dir = c.data_dir.clone().unwrap_or_else(default_data_dir);
    Engine::open(EngineConfig {
        db_path: data_dir.join("index.sqlite"),
        roots: roots(c),
        providers: providers(c),
        mac_terminal: terminal,
    })
    .context("open index")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn common() -> Common {
        Common {
            data_dir: None,
            claude_root: Some("/c".into()),
            codex_root: None,
            grok_root: Some("/g".into()),
            providers: vec![],
        }
    }

    #[test]
    fn flags_override_roots_and_providers() {
        let c = common();
        let r = roots(&c);
        assert_eq!(
            (r.claude.to_str(), r.grok.to_str()),
            (Some("/c"), Some("/g"))
        );
        assert_eq!(providers(&c), Provider::ALL.to_vec());
        let c = Common {
            providers: vec![Provider::Grok, Provider::Grok, Provider::Claude],
            ..common()
        };
        assert_eq!(providers(&c), [Provider::Grok, Provider::Claude]);
    }
}
