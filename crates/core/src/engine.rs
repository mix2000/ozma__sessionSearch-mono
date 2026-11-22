//! Facade of the core: owns the index connections, the background sync status
//! and the live-session cache, and exposes search, context and resume to the
//! app. All methods are blocking; call them off the async runtime.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use rusqlite::Connection;

use crate::error::{Error, Result};
use crate::models::messages as msg;
use crate::models::time::{iso, iso_opt};
use crate::models::{
    ContextMessage, EngineConfig, IndexStatus, MacTerminal, Provider, ProviderStatus,
    ResumeResponse, Roots, SearchParams, SearchResponse, Status, SyncResult,
};
use crate::platform::{self, FsProbe, OsFs, OsProcesses, OsTerminal};
use crate::services::command::valid_id;
use crate::services::indexer;
use crate::services::launch_plan;
use crate::services::live::LiveCache;
use crate::services::query::{fold, parse_query, query_is_valid, MIN_QUERY_CHARS};
use crate::services::resume::{self, ResumeChecks};
use crate::services::search::{self as search_svc, CardContext, MAX_ROWS};
use crate::sources::registry::ClaudeRegistry;
use crate::storage::{self, messages, sessions};

/// How resume opens a terminal: `(workdir, argv)` (replaceable in tests).
pub type Launcher = dyn Fn(&Path, &[String]) -> anyhow::Result<()> + Send + Sync;

/// The system terminal for the chosen macOS terminal app.
fn system_launcher(mac_terminal: MacTerminal) -> Box<Launcher> {
    Box::new(move |dir, argv| {
        launch_plan::launch(&OsTerminal, platform::current_os(), mac_terminal, dir, argv)
    })
}

/// Messages shown on each side of a hit in the context view.
pub const CONTEXT_RADIUS: i64 = 3;

pub struct Engine {
    writer: Mutex<Connection>,
    reader: Mutex<Connection>,
    roots: Roots,
    providers: Vec<Provider>,
    running: AtomicBool,
    stop: AtomicBool,
    status: Mutex<IndexStatus>,
    live: LiveCache,
    fs: Box<dyn FsProbe>,
    launcher: Box<Launcher>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Engine {
    pub fn open(cfg: EngineConfig) -> anyhow::Result<Self> {
        let writer = storage::open(&cfg.db_path)?;
        let reader = storage::open(&cfg.db_path)?;
        let registry = ClaudeRegistry::for_roots(&cfg.roots);
        Ok(Engine {
            writer: Mutex::new(writer),
            reader: Mutex::new(reader),
            roots: cfg.roots,
            providers: cfg.providers,
            running: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            status: Mutex::new(IndexStatus::default()),
            live: LiveCache::new(Box::new(OsProcesses), Box::new(registry)),
            fs: Box::new(OsFs),
            launcher: system_launcher(cfg.mac_terminal),
        })
    }

    /// Replaces the terminal launcher (tests).
    pub fn with_launcher(mut self, launcher: Box<Launcher>) -> Self {
        self.launcher = launcher;
        self
    }

    pub fn roots(&self) -> &Roots {
        &self.roots
    }

    /// Makes a running and all further syncs stop after the current file.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Synchronizes all transcripts. A concurrent call is a no-op.
    pub fn sync(&self) -> SyncResult {
        if self.running.swap(true, Ordering::SeqCst) {
            return SyncResult::default();
        }
        let started = Instant::now();
        let result = {
            let mut conn = lock(&self.writer);
            indexer::sync(&mut conn, &self.roots, &self.providers, &self.stop)
        };
        let elapsed = started.elapsed();
        let mut st = lock(&self.status);
        let out = match result {
            Ok(r) => {
                st.last_error = None;
                if r.messages > 0 {
                    tracing::info!(
                        "index: {} sessions updated, +{} messages in {} ms",
                        r.updated,
                        r.messages,
                        elapsed.as_millis()
                    );
                }
                r
            }
            Err(e) => {
                tracing::error!("index: {e:#}");
                st.last_error = Some(format!("{e:#}"));
                SyncResult::default()
            }
        };
        st.last_run_at = Some(iso(chrono::Utc::now().timestamp_millis()));
        st.last_duration_ms = Some(elapsed.as_millis() as u64);
        self.running.store(false, Ordering::SeqCst);
        out
    }

    pub fn status(&self) -> Result<Status> {
        let conn = lock(&self.reader);
        let (sessions, messages) = sessions::stats(&conn)?;
        let counts: HashMap<String, i64> =
            sessions::sessions_by_provider(&conn)?.into_iter().collect();
        drop(conn);
        let mut index = lock(&self.status).clone();
        index.running = self.running.load(Ordering::SeqCst);
        let providers = self
            .providers
            .iter()
            .map(|&p| {
                let root = self.roots.get(p);
                ProviderStatus {
                    provider: p,
                    root: root.to_string_lossy().into_owned(),
                    exists: self.fs.is_dir(root),
                    sessions: counts.get(p.as_str()).copied().unwrap_or(0),
                }
            })
            .collect();
        Ok(Status {
            sessions,
            messages,
            index,
            providers,
            min_query_length: MIN_QUERY_CHARS,
        })
    }

    /// All words/phrases must occur in one message (case-insensitive).
    /// Sessions freshest first, up to 3 hits each.
    pub fn search(&self, params: &SearchParams) -> Result<SearchResponse> {
        let terms = parse_query(&params.q);
        if !query_is_valid(&terms) {
            return Err(Error::bad_request(msg::query_too_short()));
        }
        let terms_lc: Vec<String> = terms.iter().map(|t| fold(t)).collect();
        let provider = params
            .provider
            .as_deref()
            .filter(|p| p.parse::<Provider>().is_ok());
        let conn = lock(&self.reader);
        let rows = messages::find(
            &conn,
            &terms_lc,
            provider,
            params.include_headless,
            MAX_ROWS,
        )?;
        let grouped = search_svc::group_by_session(&rows);
        let limit = search_svc::effective_limit(params.limit);
        let ids: Vec<String> = grouped.order.iter().take(limit).cloned().collect();
        let found = sessions::get_sessions(&conn, &ids)?;
        drop(conn);
        let live = self.live.get();
        let ctx = CardContext {
            live: &live,
            fs: self.fs.as_ref(),
            os: platform::current_os(),
        };
        Ok(search_svc::assemble(
            terms,
            rows.len(),
            &grouped,
            found,
            limit,
            &ctx,
        ))
    }

    /// Messages around a hit (full text).
    pub fn context(&self, session_id: &str, seq: i64) -> Result<Vec<ContextMessage>> {
        let conn = lock(&self.reader);
        let rows = messages::range(
            &conn,
            session_id,
            (seq - CONTEXT_RADIUS).max(0),
            seq + CONTEXT_RADIUS,
        )?;
        Ok(rows
            .into_iter()
            .map(|r| ContextMessage {
                id: r.id,
                seq: r.seq,
                role: r.role,
                at: iso_opt(r.at),
                text: r.text,
            })
            .collect())
    }

    /// Opens the conversation in a new terminal window in its workdir.
    pub fn resume(&self, id: &str) -> Result<ResumeResponse> {
        if !valid_id(id) {
            return Err(Error::bad_request(msg::BAD_SESSION_ID));
        }
        let session = {
            let conn = lock(&self.reader);
            sessions::get_session(&conn, id)?
        }
        .ok_or_else(|| Error::not_found(msg::SESSION_NOT_FOUND))?;
        let target = resume::target(&session, platform::current_os())?;
        resume::check(
            &target,
            ResumeChecks {
                live_pid: self.live.get().get(&session.id.to_lowercase()).copied(),
                workdir_exists: self.fs.is_dir(Path::new(&target.workdir)),
                file_exists: self.fs.is_file(Path::new(&session.file_path)),
            },
        )?;
        (self.launcher)(Path::new(&target.workdir), &target.argv)
            .map_err(|e| resume::launch_failed(&target, &e))?;
        Ok(ResumeResponse {
            ok: true,
            command: target.command,
            workdir: target.workdir,
        })
    }
}
