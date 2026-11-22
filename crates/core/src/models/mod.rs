//! Domain types and API DTOs (serde). No I/O and no business logic here.

pub mod config;
pub mod launch;
pub mod messages;
pub mod provider;
pub mod search;
pub mod status;
pub mod time;
pub mod transcript;

pub use config::{EngineConfig, MacTerminal, Roots, TargetOs};
pub use launch::LaunchPlan;
pub use provider::Provider;
pub use search::{
    ContextMessage, LiveState, ResumeResponse, SearchParams, SearchResponse, Snippet,
    TranscriptHit, TranscriptResult,
};
pub use status::{IndexStatus, ProviderStatus, Status, SyncResult};
pub use transcript::{ParsedLine, ParsedMessage, ParsedMeta, Role};
