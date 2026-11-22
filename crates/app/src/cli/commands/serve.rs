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

pub fn run(engine: Engine, args: ServeArgs) -> Result<ExitCode> {
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
    let listener = bind(args.host, args.port, &args.skip_ports)?;
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
    let state = AppState {
        engine: engine.clone(),
        token: args.token.clone(),
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
    fn binds_next_free_port() {
        let host: IpAddr = "127.0.0.1".parse().unwrap();
        let first = bind(host, 0, &[]).unwrap();
        assert!(first.local_addr().unwrap().port() > 0);
    }
}
