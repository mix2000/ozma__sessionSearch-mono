//! The index as a whole: storage, background sync status, search, context, resume.
//! All methods are blocking; call them off the async runtime.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

use crate::db::{self, iso, iso_opt};
use crate::indexer::{self, SyncResult};
use crate::live::{LiveCache, LiveMap};
use crate::provider::{Provider, Roots};
use crate::query::{build_snippet, fold, parse_query, query_is_valid, Snippet};
use crate::terminal::{self, MacTerminal, TargetOs};

pub const MAX_ROWS: usize = 600;
pub const HITS_PER_SESSION: usize = 3;
pub const DEFAULT_LIMIT: usize = 40;
pub const CONTEXT_RADIUS: i64 = 3;

/// An error with an HTTP status, rendered as `{statusCode, message}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
    /// For resume errors: the command to run by hand.
    pub command: Option<String>,
}

impl ApiError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        ApiError {
            status,
            message: message.into(),
            command: None,
        }
    }
    pub fn bad_request(m: impl Into<String>) -> Self {
        Self::new(400, m)
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(404, m)
    }
    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self::new(500, e.to_string())
    }
    fn with_command(mut self, c: &str) -> Self {
        self.command = Some(c.to_owned());
        self
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.status, self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::internal(format!("{e:#}"))
    }
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LiveState {
    External { pid: u32 },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptHit {
    pub message_id: i64,
    pub seq: i64,
    pub role: String,
    pub at: Option<String>,
    pub snippet: Snippet,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptResult {
    pub id: String,
    pub provider: String,
    pub title: Option<String>,
    pub workdir: Option<String>,
    pub kind: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<String>,
    pub last_at: Option<String>,
    pub message_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<LiveState>,
    pub workdir_exists: bool,
    /// Command to continue the conversation in one's own terminal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_command: Option<String>,
    pub matches: usize,
    pub hits: Vec<TranscriptHit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub terms: Vec<String>,
    pub results: Vec<TranscriptResult>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SearchParams {
    pub q: String,
    pub provider: Option<String>,
    pub include_headless: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextMessage {
    pub id: i64,
    pub seq: i64,
    pub role: String,
    pub at: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub running: bool,
    pub last_run_at: Option<String>,
    pub last_duration_ms: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider: Provider,
    pub root: String,
    pub exists: bool,
    pub sessions: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub sessions: i64,
    pub messages: i64,
    pub index: IndexStatus,
    pub providers: Vec<ProviderStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeResponse {
    pub ok: bool,
    pub command: String,
    pub workdir: String,
}

/// How resume opens a terminal (replaceable in tests).
pub type Launcher = dyn Fn(&Path, &[String]) -> Result<()> + Send + Sync;

pub struct EngineConfig {
    pub db_path: PathBuf,
    pub roots: Roots,
    pub providers: Vec<Provider>,
    pub mac_terminal: MacTerminal,
}

pub struct Engine {
    writer: Mutex<Connection>,
    reader: Mutex<Connection>,
    roots: Roots,
    providers: Vec<Provider>,
    running: AtomicBool,
    stop: AtomicBool,
    status: Mutex<IndexStatus>,
    live: LiveCache,
    launcher: Box<Launcher>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Engine {
    pub fn open(cfg: EngineConfig) -> Result<Self> {
        let writer = db::open(&cfg.db_path)?;
        let reader = db::open(&cfg.db_path)?;
        let sessions_dir = cfg
            .roots
            .claude
            .parent()
            .map(|p| p.join("sessions"))
            .unwrap_or_else(|| PathBuf::from("sessions"));
        let mac_terminal = cfg.mac_terminal;
        Ok(Engine {
            writer: Mutex::new(writer),
            reader: Mutex::new(reader),
            roots: cfg.roots,
            providers: cfg.providers,
            running: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            status: Mutex::new(IndexStatus::default()),
            live: LiveCache::new(sessions_dir),
            launcher: Box::new(move |dir, argv| terminal::launch(mac_terminal, dir, argv)),
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

    pub fn status(&self) -> ApiResult<Status> {
        let conn = lock(&self.reader);
        let (sessions, messages) = db::stats(&conn)?;
        let counts: HashMap<String, i64> = db::sessions_by_provider(&conn)?.into_iter().collect();
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
                    exists: root.is_dir(),
                    sessions: counts.get(p.as_str()).copied().unwrap_or(0),
                }
            })
            .collect();
        Ok(Status {
            sessions,
            messages,
            index,
            providers,
        })
    }

    /// All words/phrases must occur in one message (case-insensitive).
    /// Sessions freshest first, up to 3 hits each.
    pub fn search(&self, params: &SearchParams) -> ApiResult<SearchResponse> {
        let terms = parse_query(&params.q);
        if !query_is_valid(&terms) {
            return Err(ApiError::bad_request("Введите хотя бы 2 символа"));
        }
        let terms_lc: Vec<String> = terms.iter().map(|t| fold(t)).collect();
        let provider = params
            .provider
            .as_deref()
            .filter(|p| p.parse::<Provider>().is_ok());
        let conn = lock(&self.reader);
        let rows = db::find_messages(
            &conn,
            &terms_lc,
            provider,
            params.include_headless,
            MAX_ROWS,
        )?;

        let mut order: Vec<String> = Vec::new();
        let mut by_session: HashMap<String, Vec<db::MessageRow>> = HashMap::new();
        for r in &rows {
            let list = by_session.entry(r.session_id.clone()).or_insert_with(|| {
                order.push(r.session_id.clone());
                Vec::new()
            });
            list.push(r.clone());
        }
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(100);
        let ids: Vec<String> = order.iter().take(limit).cloned().collect();
        let sessions = db::get_sessions(&conn, &ids)?;
        drop(conn);
        let live = self.live.get();
        let mut results = Vec::with_capacity(sessions.len());
        for s in sessions {
            let hits = &by_session[&s.id];
            let mut result = to_result(s, &live);
            result.matches = hits.len();
            result.hits = hits
                .iter()
                .take(HITS_PER_SESSION)
                .map(|h| TranscriptHit {
                    message_id: h.id,
                    seq: h.seq,
                    role: h.role.clone(),
                    at: iso_opt(h.at),
                    snippet: build_snippet(&h.text, &terms),
                })
                .collect();
            results.push(result);
        }
        Ok(SearchResponse {
            terms,
            results,
            truncated: rows.len() >= MAX_ROWS || by_session.len() > limit,
        })
    }

    /// Messages around a hit (full text).
    pub fn context(&self, session_id: &str, seq: i64) -> ApiResult<Vec<ContextMessage>> {
        let conn = lock(&self.reader);
        let rows = db::context(
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
    pub fn resume(&self, id: &str) -> ApiResult<ResumeResponse> {
        if !terminal::valid_id(id) {
            return Err(ApiError::bad_request("Некорректный id разговора"));
        }
        let session = {
            let conn = lock(&self.reader);
            db::get_session(&conn, id)?
        }
        .ok_or_else(|| ApiError::not_found("Разговор не найден"))?;
        let provider: Provider = session
            .provider
            .parse()
            .map_err(|e: String| ApiError::internal(e))?;
        let workdir = session
            .workdir
            .clone()
            .ok_or_else(|| ApiError::bad_request("Папка разговора неизвестна"))?;
        let argv = provider.resume_argv(&session.id, &session.file_path);
        let command = terminal::copy_command(TargetOs::current(), &workdir, &argv);
        if let Some(pid) = self.live.get().get(&session.id.to_lowercase()) {
            return Err(
                ApiError::new(409, format!("Разговор уже открыт в терминале (pid {pid})"))
                    .with_command(&command),
            );
        }
        if !Path::new(&workdir).is_dir() {
            return Err(
                ApiError::new(409, format!("Папка разговора не найдена: {workdir}"))
                    .with_command(&command),
            );
        }
        if !Path::new(&session.file_path).is_file() {
            return Err(ApiError::new(
                409,
                "Агент уже удалил файл этого разговора — продолжить нельзя, текст остался только в поиске",
            )
            .with_command(&command));
        }
        (self.launcher)(Path::new(&workdir), &argv).map_err(|e| {
            ApiError::new(500, format!("Не удалось открыть терминал: {e:#}")).with_command(&command)
        })?;
        Ok(ResumeResponse {
            ok: true,
            command,
            workdir,
        })
    }

    /// Copy command for a session without launching (CLI).
    pub fn resume_command(&self, id: &str) -> ApiResult<String> {
        let conn = lock(&self.reader);
        let s =
            db::get_session(&conn, id)?.ok_or_else(|| ApiError::not_found("Разговор не найден"))?;
        session_command(&s).ok_or_else(|| ApiError::internal("unknown provider"))
    }
}

/// `cd <workdir> && <agent resume command>` for the current OS (just the
/// agent command when the workdir is unknown).
fn session_command(s: &db::SessionRow) -> Option<String> {
    let provider: Provider = s.provider.parse().ok()?;
    let argv = provider.resume_argv(&s.id, &s.file_path);
    Some(match s.workdir.as_deref() {
        Some(w) => terminal::copy_command(TargetOs::current(), w, &argv),
        None => argv.join(" "),
    })
}

fn to_result(t: db::SessionRow, live: &LiveMap) -> TranscriptResult {
    let workdir_exists = t.workdir.as_deref().is_some_and(|w| Path::new(w).is_dir());
    TranscriptResult {
        live: live
            .get(&t.id.to_lowercase())
            .map(|&pid| LiveState::External { pid }),
        resume_command: session_command(&t),
        id: t.id,
        provider: t.provider,
        title: t.title,
        workdir: t.workdir,
        kind: t.kind,
        git_branch: t.git_branch,
        started_at: iso_opt(t.started_at),
        last_at: iso_opt(t.last_at),
        message_count: t.message_count,
        workdir_exists,
        matches: 0,
        hits: Vec::new(),
    }
}

/// Unique providers in a stable order.
pub fn dedup_providers(list: &[Provider]) -> Vec<Provider> {
    let mut seen = HashSet::new();
    list.iter().copied().filter(|p| seen.insert(*p)).collect()
}
