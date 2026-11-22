//! Core of ozma Session Search: finds Claude Code / Codex / Grok transcripts on
//! disk, keeps an incremental SQLite index of the conversation text and
//! searches it.

pub mod db;
pub mod engine;
pub mod grok_db;
pub mod indexer;
pub mod live;
pub mod parse;
pub mod provider;
pub mod query;
pub mod terminal;

pub use engine::{
    ApiError, ApiResult, ContextMessage, Engine, EngineConfig, IndexStatus, LiveState,
    ResumeResponse, SearchParams, SearchResponse, Status, TranscriptHit, TranscriptResult,
};
pub use indexer::SyncResult;
pub use provider::{Provider, Roots};
pub use terminal::MacTerminal;

/// Default data dir: `<OS data dir>/ozma-session-search`.
pub fn default_data_dir() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| ".".into()))
        .join("ozma-session-search")
}
