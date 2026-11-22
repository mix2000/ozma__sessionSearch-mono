//! `session-search` — local search over Claude Code / Codex / Grok conversations.

use std::process::ExitCode;

fn main() -> ExitCode {
    match session_search_app::cli::run() {
        Ok(code) => code,
        Err(e) => {
            // Same output as returning the error from `main`.
            eprintln!("Error: {e:?}");
            ExitCode::FAILURE
        }
    }
}
