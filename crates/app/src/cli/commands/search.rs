//! `session-search search`: search from the command line.

use std::process::ExitCode;

use anyhow::Result;
use session_search_core::{unprintable, Engine, SearchParams, SearchResponse};

use crate::cli::args::SearchArgs;

/// Text from a transcript, safe for a terminal: control (escape sequences
/// included) and bidirectional characters become `�`.
fn printable(s: &str) -> String {
    s.replace(unprintable, "\u{fffd}")
}

/// Whitespace collapsed to single spaces, clipped to `max` chars with `…`.
fn one_line(s: &str, max: usize) -> String {
    let flat = printable(&s.split_whitespace().collect::<Vec<_>>().join(" "));
    if flat.chars().count() > max {
        flat.chars().take(max).collect::<String>() + "…"
    } else {
        flat
    }
}

fn params(args: &SearchArgs) -> SearchParams {
    SearchParams {
        q: args.query.join(" "),
        provider: args.provider.map(|p| p.as_str().to_owned()),
        include_headless: args.include_headless,
        limit: args.limit,
    }
}

/// Exit code when the query is rejected (too short).
const EXIT_BAD_QUERY: u8 = 2;

pub fn run(engine: &Engine, args: SearchArgs) -> Result<ExitCode> {
    if !args.no_sync {
        engine.sync();
    }
    let res = match engine.search(&params(&args)) {
        Ok(r) => r,
        Err(e) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::json!({ "statusCode": e.status(), "message": e.message() })
                );
            } else {
                eprintln!("{}", e.message());
            }
            return Ok(ExitCode::from(EXIT_BAD_QUERY));
        }
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(ExitCode::SUCCESS);
    }
    print_results(&res);
    Ok(ExitCode::SUCCESS)
}

fn print_results(res: &SearchResponse) {
    if res.results.is_empty() {
        println!("Nothing found.");
        return;
    }
    for r in &res.results {
        println!(
            "\n[{}] {}  ({} messages, last {})",
            r.provider,
            one_line(r.title.as_deref().unwrap_or("(untitled)"), 100),
            r.message_count,
            r.last_at.as_deref().unwrap_or("?")
        );
        if let Some(w) = &r.workdir {
            println!(
                "  {}{}",
                printable(w),
                if r.workdir_exists {
                    ""
                } else {
                    "  (folder missing)"
                }
            );
        }
        for h in &r.hits {
            println!(
                "  #{} {} {}: {}",
                h.seq,
                h.role,
                h.at.as_deref().unwrap_or(""),
                one_line(&h.snippet.matched, 200)
            );
        }
        if r.matches > r.hits.len() {
            println!("  … {} more", r.matches - r.hits.len());
        }
        if let Some(cmd) = &r.resume_command {
            let hint = if r.resume_needs_folder {
                "  (run it in the folder above)"
            } else {
                ""
            };
            println!("  resume: {cmd}{hint}");
        }
    }
    if res.truncated {
        println!("\n(results truncated — refine the query)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_search_core::Provider;

    #[test]
    fn one_line_collapses_and_clips() {
        assert_eq!(one_line(" a\n\tb  c ", 10), "a b c");
        assert_eq!(one_line("абвгд", 3), "абв…");
        assert_eq!(one_line("a\u{1b}[31m\tb", 10), "a\u{fffd}[31m b");
        assert_eq!(printable("a\u{202e}b\u{2066}"), "a\u{fffd}b\u{fffd}");
        assert_eq!(printable("/a\n\u{9b}b c"), "/a\u{fffd}\u{fffd}b c");
    }

    #[test]
    fn params_from_args() {
        let p = params(&SearchArgs {
            query: vec!["a".into(), "\"b c\"".into()],
            provider: Some(Provider::Codex),
            include_headless: true,
            limit: Some(3),
            json: false,
            no_sync: true,
        });
        assert_eq!(p.q, "a \"b c\"");
        assert_eq!(p.provider.as_deref(), Some("codex"));
        assert!(p.include_headless);
        assert_eq!(p.limit, Some(3));
    }
}
