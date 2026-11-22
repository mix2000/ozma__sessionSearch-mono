//! `session-search sync`: update the index now.

use std::process::ExitCode;

use anyhow::Result;
use session_search_core::Engine;

pub fn run(engine: &Engine, json: bool) -> Result<ExitCode> {
    let started = std::time::Instant::now();
    let r = engine.sync();
    if json {
        println!("{}", serde_json::to_string_pretty(&r)?);
    } else {
        println!(
            "files: {}, updated: {}, new messages: {} ({} ms)",
            r.files,
            r.updated,
            r.messages,
            started.elapsed().as_millis()
        );
    }
    Ok(ExitCode::SUCCESS)
}
