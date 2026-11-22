//! Command line: argument parsing, logging setup and dispatch to commands.

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

mod args;
mod commands;

use args::{Cli, Cmd, ServeArgs};

/// Entry point of the binary: the process exit code, or an error (exit 1).
pub fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let serving = matches!(cli.cmd, None | Some(Cmd::Serve(_)));
    let default_level = if serving { "info" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level)),
        )
        .with_writer(std::io::stderr)
        .init();

    let terminal = match &cli.cmd {
        Some(Cmd::Serve(a)) => a.terminal,
        _ => ServeArgs::default().terminal,
    };
    let engine = commands::open_engine(&cli.common, terminal)?;
    match cli.cmd {
        None => commands::serve::run(engine, ServeArgs::default()),
        Some(Cmd::Serve(args)) => commands::serve::run(engine, args),
        Some(Cmd::Search(args)) => commands::search::run(&engine, args),
        Some(Cmd::Sync { json }) => commands::sync::run(&engine, json),
        Some(Cmd::Status { json }) => commands::status::run(&engine, json),
    }
}
