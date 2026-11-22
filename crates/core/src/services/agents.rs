//! Per-agent rules: how to resume a conversation and how to recognise the
//! agent's process.

use std::collections::HashSet;
use std::path::Path;

use crate::models::Provider;
use crate::sources::grok::cli::GROK_DB;

/// Whether a session's source is grok-cli's database.
pub fn is_grok_db(file_path: &str) -> bool {
    Path::new(file_path).file_name().and_then(|n| n.to_str()) == Some(GROK_DB)
}

/// Program and arguments that resume conversation `id` (run in its
/// workdir). Grok sessions imported from grok-cli's `grok.db` use `-s`.
pub fn resume_argv(provider: Provider, id: &str, file_path: &str) -> Vec<String> {
    match provider {
        Provider::Claude => vec!["claude".into(), "--resume".into(), id.into()],
        Provider::Codex => vec!["codex".into(), "resume".into(), id.into()],
        Provider::Grok if is_grok_db(file_path) => vec!["grok".into(), "-s".into(), id.into()],
        Provider::Grok => vec!["grok".into(), "--resume".into(), id.into()],
    }
}

/// Words that identify this agent's process on a command line.
pub fn process_markers(provider: Provider) -> &'static [&'static str] {
    match provider {
        Provider::Claude => &["claude"],
        Provider::Codex => &["codex"],
        Provider::Grok => &["grok"],
    }
}

/// Unique providers in a stable order.
pub fn dedup_providers(list: &[Provider]) -> Vec<Provider> {
    let mut seen = HashSet::new();
    list.iter().copied().filter(|p| seen.insert(*p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_commands() {
        let a = |p: Provider, f: &str| resume_argv(p, "abc", f).join(" ");
        assert_eq!(a(Provider::Claude, "/x.jsonl"), "claude --resume abc");
        assert_eq!(a(Provider::Codex, "/x.jsonl"), "codex resume abc");
        assert_eq!(
            a(Provider::Grok, "/h/.grok/sessions/g/abc/updates.jsonl"),
            "grok --resume abc"
        );
        assert_eq!(a(Provider::Grok, "/h/.grok/grok.db"), "grok -s abc");
    }

    #[test]
    fn markers_and_dedup() {
        use Provider::*;
        assert_eq!(process_markers(Codex), ["codex"]);
        assert_eq!(
            dedup_providers(&[Grok, Claude, Grok, Codex, Claude]),
            [Grok, Claude, Codex]
        );
        assert!(!is_grok_db("/g/grok.db.bak"));
    }
}
