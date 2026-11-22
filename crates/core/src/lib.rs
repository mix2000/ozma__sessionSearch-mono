//! Core of ozma Session Search: finds Claude Code / Codex / Grok transcripts on
//! disk, keeps an incremental SQLite index of the conversation text and
//! searches it.
//!
//! Layers: [`models`] (types and API DTOs) → `sources` (transcript formats on
//! disk) → `storage` (SQLite index) → `services` (indexing, search, resume
//! rules) → [`Engine`] (facade). OS-specific code lives in `platform`.

mod engine;
mod error;
pub mod models;
mod platform;
mod services;
mod sources;
mod storage;

pub use engine::{Engine, Launcher};
pub use error::{Error, Result};
pub use models::{
    ContextMessage, EngineConfig, IndexStatus, LiveState, MacTerminal, Provider, ProviderStatus,
    ResumeResponse, Roots, SearchParams, SearchResponse, Snippet, Status, SyncResult,
    TranscriptHit, TranscriptResult,
};
pub use platform::paths::default_data_dir;
pub use services::agents::dedup_providers;
