//! Command-line arguments (clap).

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use session_search_core::{MacTerminal, Provider};

pub const DEFAULT_PORT: u16 = 4477;

#[derive(Parser)]
#[command(
    name = "session-search",
    version,
    about = "Search your Claude Code, Codex and Grok conversations"
)]
pub struct Cli {
    #[command(flatten)]
    pub common: Common,
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Args, Clone)]
pub struct Common {
    /// Where the index is stored [default: <OS data dir>/ozma-session-search]
    #[arg(long, global = true, env = "SESSION_SEARCH_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
    /// Claude projects folder [default: $CLAUDE_CONFIG_DIR/projects or ~/.claude/projects]
    #[arg(long, global = true, env = "SESSION_SEARCH_CLAUDE_ROOT")]
    pub claude_root: Option<PathBuf>,
    /// Codex sessions folder [default: $CODEX_HOME/sessions or ~/.codex/sessions]
    #[arg(long, global = true, env = "SESSION_SEARCH_CODEX_ROOT")]
    pub codex_root: Option<PathBuf>,
    /// Grok sessions folder
    #[arg(long, global = true, env = "SESSION_SEARCH_GROK_ROOT")]
    pub grok_root: Option<PathBuf>,
    /// Index only these providers (comma separated)
    #[arg(long, global = true, value_delimiter = ',', value_parser = parse_provider)]
    pub providers: Vec<Provider>,
}

fn parse_provider(s: &str) -> Result<Provider, String> {
    s.parse()
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Run the web UI and API (default)
    Serve(ServeArgs),
    /// Search from the command line
    Search(SearchArgs),
    /// Update the index now
    Sync {
        #[arg(long)]
        json: bool,
    },
    /// Show index status
    Status {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args, Clone)]
pub struct ServeArgs {
    /// Port (the next free one is used if busy)
    #[arg(long, default_value_t = DEFAULT_PORT, env = "SESSION_SEARCH_PORT")]
    pub port: u16,
    /// Address to bind. Anything but loopback exposes your transcripts to the network and requires --token.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: IpAddr,
    /// Do not open the browser
    #[arg(long)]
    pub no_open: bool,
    /// Require this token for /api (Authorization: Bearer or ?token=)
    #[arg(long, env = "SESSION_SEARCH_TOKEN", hide_env_values = true)]
    pub token: Option<String>,
    /// macOS terminal for "resume": terminal | iterm
    #[arg(long, env = "SESSION_SEARCH_TERMINAL", default_value = "terminal")]
    pub terminal: MacTerminal,
    /// Ports never to take while looking for a free one (comma separated)
    #[arg(long, env = "SESSION_SEARCH_SKIP_PORTS", value_delimiter = ',')]
    pub skip_ports: Vec<u16>,
}

impl Default for ServeArgs {
    fn default() -> Self {
        ServeArgs {
            port: std::env::var("SESSION_SEARCH_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(DEFAULT_PORT),
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            no_open: false,
            token: std::env::var("SESSION_SEARCH_TOKEN")
                .ok()
                .filter(|t| !t.is_empty()),
            terminal: std::env::var("SESSION_SEARCH_TERMINAL")
                .ok()
                .and_then(|t| t.parse().ok())
                .unwrap_or_default(),
            skip_ports: std::env::var("SESSION_SEARCH_SKIP_PORTS")
                .map(|v| parse_ports(&v))
                .unwrap_or_default(),
        }
    }
}

/// `"38000, 48000"` → ports; malformed entries are ignored.
fn parse_ports(v: &str) -> Vec<u16> {
    v.split(',').filter_map(|p| p.trim().parse().ok()).collect()
}

#[derive(Args)]
pub struct SearchArgs {
    /// Words and "quoted phrases"; all must occur in one message
    #[arg(required = true, num_args = 1..)]
    pub query: Vec<String>,
    #[arg(long, value_parser = parse_provider)]
    pub provider: Option<Provider>,
    /// Include headless runs (claude -p, codex exec)
    #[arg(long)]
    pub include_headless: bool,
    /// Max sessions (up to 100)
    #[arg(long)]
    pub limit: Option<usize>,
    /// Print the API JSON
    #[arg(long)]
    pub json: bool,
    /// Do not update the index before searching
    #[arg(long)]
    pub no_sync: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_global_and_subcommand_flags() {
        let cli = Cli::try_parse_from([
            "session-search",
            "search",
            "nginx",
            "deploy",
            "--providers",
            "codex,claude",
            "--provider",
            "grok",
            "--json",
        ])
        .unwrap();
        assert_eq!(cli.common.providers, [Provider::Codex, Provider::Claude]);
        match cli.cmd {
            Some(Cmd::Search(a)) => {
                assert_eq!(a.query, ["nginx", "deploy"]);
                assert_eq!(a.provider, Some(Provider::Grok));
                assert!(a.json && !a.no_sync);
            }
            _ => panic!("expected search"),
        }
        assert!(Cli::try_parse_from(["session-search", "--providers", "x", "status"]).is_err());
    }

    #[test]
    fn skip_ports_flag_and_env_format() {
        let cli = Cli::try_parse_from(["session-search", "serve", "--skip-ports", "38000,48000"])
            .unwrap();
        match cli.cmd {
            Some(Cmd::Serve(a)) => assert_eq!(a.skip_ports, [38000, 48000]),
            _ => panic!("expected serve"),
        }
        assert_eq!(parse_ports(" 1, x,2 "), [1, 2]);
    }
}
