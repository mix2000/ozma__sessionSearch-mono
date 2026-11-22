//! `session-search` app: `cli` parses arguments and runs commands; `http` is
//! the API and the embedded UI. The binary (`main.rs`) only calls [`cli::run`].

pub mod cli;
pub mod http;
mod platform;
