//! `session-search serve` (default): web UI + API with background indexing.

use std::net::{IpAddr, SocketAddr, TcpListener};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use session_search_core::Engine;

use crate::cli::args::ServeArgs;
use crate::http::{self, AppState};
use crate::platform::shutdown_signal;

const SYNC_EVERY: Duration = Duration::from_secs(5 * 60);
const FIRST_SYNC_AFTER: Duration = Duration::from_secs(1);

/// Ports tried from `start`: up to 100 more, never one of `skip`.
fn candidate_ports(start: u16, skip: &[u16]) -> impl Iterator<Item = u16> + '_ {
    (start..=start.saturating_add(100)).filter(move |p| !skip.contains(p))
}

fn bind(host: IpAddr, start: u16, skip: &[u16]) -> Result<TcpListener> {
    let mut last_err = None;
    for port in candidate_ports(start, skip) {
        match TcpListener::bind(SocketAddr::new(host, port)) {
            Ok(l) => return Ok(l),
            Err(e) => last_err = Some(e),
        }
    }
    Err(anyhow::anyhow!(
        "no free port from {start}: {}",
        last_err.map(|e| e.to_string()).unwrap_or_default()
    ))
}

/// The token to enforce: a blank one counts as none. Outside loopback there is
/// no Host/Origin check, so the API (including resume) is never served without it.
fn effective_token(host: IpAddr, token: Option<&str>) -> Result<Option<String>> {
    let token = token.map(str::trim).filter(|t| !t.is_empty());
    if token.is_none() && !host.is_loopback() {
        anyhow::bail!(
            "--host {host} exposes your transcripts to the network: set --token (or SESSION_SEARCH_TOKEN)"
        );
    }
    Ok(token.map(str::to_owned))
}

/// The page to open; the UI takes the token from `?token=` and drops it from the address.
fn browser_url(url: &str, token: Option<&str>) -> String {
    match token {
        Some(t) => {
            let query: String = form_urlencoded::Serializer::new(String::new())
                .append_pair("token", t)
                .finish();
            format!("{url}?{query}")
        }
        None => url.to_owned(),
    }
}

pub fn run(engine: Engine, args: ServeArgs) -> Result<ExitCode> {
    let loopback = args.host.is_loopback();
    let token = effective_token(args.host, args.token.as_deref())?;
    if !loopback {
        tracing::warn!(
            "binding to {} — transcripts become reachable from the network by anyone with the token",
            args.host
        );
    }
    let listener = bind(args.host, args.port, &args.skip_ports)?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let shown_host = if loopback {
        "127.0.0.1".to_owned()
    } else {
        addr.ip().to_string()
    };
    let url = format!("http://{shown_host}:{}/", addr.port());
    let open_url = browser_url(&url, token.as_deref());

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let engine = Arc::new(engine);
    let state = AppState {
        engine: engine.clone(),
        token,
        loopback_only: loopback,
    };
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::from_std(listener)?;
        println!("Session Search: {url}");
        if !http::assets::has_ui() {
            tracing::warn!("web UI is not embedded (web/dist was empty at build time)");
        }
        if !args.no_open {
            if let Err(e) = open::that_detached(&open_url) {
                tracing::warn!("could not open the browser: {e}");
            }
        }
        let indexer = tokio::spawn(background_sync(engine.clone()));
        axum::serve(listener, http::router(state))
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        tracing::info!("shutting down");
        engine.request_stop();
        indexer.abort();
        Ok::<_, anyhow::Error>(())
    })?;
    // Wait for a running sync to stop after its current file.
    rt.shutdown_timeout(Duration::from_secs(10));
    Ok(ExitCode::SUCCESS)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skipped_ports_are_not_tried() {
        let ports: Vec<u16> = candidate_ports(37999, &[38000]).take(3).collect();
        assert_eq!(ports, [37999, 38001, 38002]);
        assert_eq!(candidate_ports(65500, &[]).last(), Some(65535));
    }

    #[test]
    fn network_address_requires_a_token() {
        let check = |host: &str, token| effective_token(host.parse().unwrap(), token);
        for host in ["0.0.0.0", "::", "192.168.1.10", "::ffff:127.0.0.1"] {
            for token in [None, Some(""), Some("  ")] {
                let err = check(host, token).unwrap_err().to_string();
                assert!(err.contains("--token"), "{host}: {err}");
            }
            assert_eq!(check(host, Some("t")).unwrap().as_deref(), Some("t"));
        }
        for host in ["127.0.0.1", "127.0.0.2", "::1"] {
            assert_eq!(check(host, None).unwrap(), None);
            assert_eq!(check(host, Some("")).unwrap(), None);
            assert_eq!(check(host, Some(" t ")).unwrap().as_deref(), Some("t"));
        }
    }

    #[test]
    fn browser_url_encodes_the_token() {
        assert_eq!(browser_url("http://h/", None), "http://h/");
        assert_eq!(
            browser_url("http://h/", Some("a+b&c#d")),
            "http://h/?token=a%2Bb%26c%23d"
        );
    }

    #[test]
    fn binds_next_free_port() {
        let host: IpAddr = "127.0.0.1".parse().unwrap();
        let first = bind(host, 0, &[]).unwrap();
        assert!(first.local_addr().unwrap().port() > 0);
    }
}
