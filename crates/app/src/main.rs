//! `session-search` — local search over Claude Code / Codex / Grok conversations.

mod assets;
mod server;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use session_search_core::{
    default_data_dir, engine::dedup_providers, Engine, EngineConfig, MacTerminal, Provider, Roots,
    SearchParams,
};

const DEFAULT_PORT: u16 = 4477;
const SYNC_EVERY: Duration = Duration::from_secs(5 * 60);
const FIRST_SYNC_AFTER: Duration = Duration::from_secs(1);
/// Ports of other local tools that must never be taken.
const RESERVED_PORTS: [u16; 2] = [38000, 48000];

#[derive(Parser)]
#[command(
    name = "session-search",
    version,
    about = "Search your Claude Code, Codex and Grok conversations"
)]
struct Cli {
    #[command(flatten)]
    common: Common,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Args, Clone)]
struct Common {
    /// Where the index is stored [default: <OS data dir>/ozma-session-search]
    #[arg(long, global = true, env = "SESSION_SEARCH_DATA_DIR")]
    data_dir: Option<PathBuf>,
    /// Claude projects folder [default: $CLAUDE_CONFIG_DIR/projects or ~/.claude/projects]
    #[arg(long, global = true, env = "SESSION_SEARCH_CLAUDE_ROOT")]
    claude_root: Option<PathBuf>,
    /// Codex sessions folder [default: $CODEX_HOME/sessions or ~/.codex/sessions]
    #[arg(long, global = true, env = "SESSION_SEARCH_CODEX_ROOT")]
    codex_root: Option<PathBuf>,
    /// Grok sessions folder
    #[arg(long, global = true, env = "SESSION_SEARCH_GROK_ROOT")]
    grok_root: Option<PathBuf>,
    /// Index only these providers (comma separated)
    #[arg(long, global = true, value_delimiter = ',', value_parser = parse_provider)]
    providers: Vec<Provider>,
}

fn parse_provider(s: &str) -> Result<Provider, String> {
    s.parse()
}

#[derive(Subcommand)]
enum Cmd {
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
struct ServeArgs {
    /// Port (the next free one is used if busy)
    #[arg(long, default_value_t = DEFAULT_PORT, env = "SESSION_SEARCH_PORT")]
    port: u16,
    /// Address to bind. Anything but loopback exposes your transcripts to the network.
    #[arg(long, default_value = "127.0.0.1")]
    host: IpAddr,
    /// Do not open the browser
    #[arg(long)]
    no_open: bool,
    /// Require this token for /api (Authorization: Bearer or ?token=)
    #[arg(long, env = "SESSION_SEARCH_TOKEN", hide_env_values = true)]
    token: Option<String>,
    /// macOS terminal for "resume": terminal | iterm
    #[arg(long, env = "SESSION_SEARCH_TERMINAL", default_value = "terminal")]
    terminal: MacTerminal,
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
        }
    }
}

#[derive(Args)]
struct SearchArgs {
    /// Words and "quoted phrases"; all must occur in one message
    #[arg(required = true, num_args = 1..)]
    query: Vec<String>,
    #[arg(long, value_parser = parse_provider)]
    provider: Option<Provider>,
    /// Include headless runs (claude -p, codex exec)
    #[arg(long)]
    include_headless: bool,
    /// Max sessions (up to 100)
    #[arg(long)]
    limit: Option<usize>,
    /// Print the API JSON
    #[arg(long)]
    json: bool,
    /// Do not update the index before searching
    #[arg(long)]
    no_sync: bool,
}

fn main() -> Result<()> {
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
    let engine = open_engine(&cli.common, terminal)?;
    match cli.cmd {
        None => serve(engine, ServeArgs::default()),
        Some(Cmd::Serve(args)) => serve(engine, args),
        Some(Cmd::Search(args)) => search_cmd(&engine, args),
        Some(Cmd::Sync { json }) => {
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
            Ok(())
        }
        Some(Cmd::Status { json }) => {
            let st = engine.status().map_err(anyhow::Error::msg)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&st)?);
            } else {
                println!("sessions: {}, messages: {}", st.sessions, st.messages);
                if let Some(at) = &st.index.last_run_at {
                    println!("last sync: {at}");
                }
                for p in &st.providers {
                    println!(
                        "  {:<7} {:>5} sessions  {}{}",
                        p.provider.as_str(),
                        p.sessions,
                        p.root,
                        if p.exists { "" } else { "  (not found)" }
                    );
                }
            }
            Ok(())
        }
    }
}

fn open_engine(c: &Common, terminal: MacTerminal) -> Result<Engine> {
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
    let data_dir = c.data_dir.clone().unwrap_or_else(default_data_dir);
    let providers = if c.providers.is_empty() {
        Provider::ALL.to_vec()
    } else {
        dedup_providers(&c.providers)
    };
    Engine::open(EngineConfig {
        db_path: data_dir.join("index.sqlite"),
        roots,
        providers,
        mac_terminal: terminal,
    })
    .context("open index")
}

fn one_line(s: &str, max: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > max {
        flat.chars().take(max).collect::<String>() + "…"
    } else {
        flat
    }
}

fn search_cmd(engine: &Engine, args: SearchArgs) -> Result<()> {
    if !args.no_sync {
        engine.sync();
    }
    let params = SearchParams {
        q: args.query.join(" "),
        provider: args.provider.map(|p| p.as_str().to_owned()),
        include_headless: args.include_headless,
        limit: args.limit,
    };
    let res = match engine.search(&params) {
        Ok(r) => r,
        Err(e) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::json!({ "statusCode": e.status, "message": e.message })
                );
            } else {
                eprintln!("{}", e.message);
            }
            std::process::exit(2);
        }
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(());
    }
    if res.results.is_empty() {
        println!("Nothing found.");
        return Ok(());
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
                "  {w}{}",
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
        if let Ok(cmd) = engine.resume_command(&r.id) {
            println!("  resume: {cmd}");
        }
    }
    if res.truncated {
        println!("\n(results truncated — refine the query)");
    }
    Ok(())
}

fn bind(host: IpAddr, start: u16) -> Result<std::net::TcpListener> {
    let mut last_err = None;
    for port in start..=start.saturating_add(100) {
        if RESERVED_PORTS.contains(&port) {
            continue;
        }
        match std::net::TcpListener::bind(SocketAddr::new(host, port)) {
            Ok(l) => return Ok(l),
            Err(e) => last_err = Some(e),
        }
    }
    Err(anyhow::anyhow!(
        "no free port from {start}: {}",
        last_err.map(|e| e.to_string()).unwrap_or_default()
    ))
}

fn serve(engine: Engine, args: ServeArgs) -> Result<()> {
    let loopback = args.host.is_loopback();
    if !loopback {
        tracing::warn!(
            "binding to {} — transcripts become reachable from the network{}",
            args.host,
            if args.token.is_some() {
                ""
            } else {
                "; consider --token"
            }
        );
    }
    let listener = bind(args.host, args.port)?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let shown_host = if loopback {
        "127.0.0.1".to_owned()
    } else {
        addr.ip().to_string()
    };
    let url = format!("http://{shown_host}:{}/", addr.port());
    let open_url = match &args.token {
        Some(t) => format!("{url}?token={t}"),
        None => url.clone(),
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let engine = Arc::new(engine);
    let state = server::AppState {
        engine: engine.clone(),
        token: args.token.clone(),
        loopback_only: loopback,
    };
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::from_std(listener)?;
        println!("Session Search: {url}");
        if !assets::has_ui() {
            tracing::warn!("web UI is not embedded (web/dist was empty at build time)");
        }
        if !args.no_open {
            if let Err(e) = open::that_detached(&open_url) {
                tracing::warn!("could not open the browser: {e}");
            }
        }
        let indexer = tokio::spawn(background_sync(engine.clone()));
        axum::serve(listener, server::router(state))
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        tracing::info!("shutting down");
        engine.request_stop();
        indexer.abort();
        Ok::<_, anyhow::Error>(())
    })?;
    // Wait for a running sync to stop after its current file.
    rt.shutdown_timeout(Duration::from_secs(10));
    Ok(())
}

async fn background_sync(engine: Arc<Engine>) {
    tokio::time::sleep(FIRST_SYNC_AFTER).await;
    let mut tick = tokio::time::interval(SYNC_EVERY);
    loop {
        tick.tick().await;
        let e = engine.clone();
        if let Err(err) = tokio::task::spawn_blocking(move || e.sync()).await {
            tracing::error!("sync task failed: {err}");
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
}
