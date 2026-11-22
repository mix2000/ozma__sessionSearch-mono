//! `session-search status`: index size, last sync and transcript roots.

use std::process::ExitCode;

use anyhow::Result;
use session_search_core::{Engine, Status};

pub fn run(engine: &Engine, json: bool) -> Result<ExitCode> {
    let st = engine.status()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&st)?);
    } else {
        print!("{}", render(&st));
    }
    Ok(ExitCode::SUCCESS)
}

fn render(st: &Status) -> String {
    let mut out = format!("sessions: {}, messages: {}\n", st.sessions, st.messages);
    if let Some(at) = &st.index.last_run_at {
        out += &format!("last sync: {at}\n");
    }
    for p in &st.providers {
        out += &format!(
            "  {:<7} {:>5} sessions  {}{}\n",
            p.provider.as_str(),
            p.sessions,
            p.root,
            if p.exists { "" } else { "  (not found)" }
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_search_core::{IndexStatus, Provider, ProviderStatus};

    #[test]
    fn text_output() {
        let st = Status {
            sessions: 2,
            messages: 5,
            index: IndexStatus {
                last_run_at: Some("2026-11-01T00:00:00.000Z".into()),
                ..Default::default()
            },
            providers: vec![ProviderStatus {
                provider: Provider::Codex,
                root: "/x".into(),
                exists: false,
                sessions: 0,
            }],
            min_query_length: 2,
        };
        assert_eq!(
            render(&st),
            "sessions: 2, messages: 5\nlast sync: 2026-11-01T00:00:00.000Z\n  codex       0 sessions  /x  (not found)\n"
        );
    }
}
