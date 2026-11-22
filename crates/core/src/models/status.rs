//! Index status and sync results.

use serde::Serialize;

use super::provider::Provider;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SyncResult {
    pub files: usize,
    pub updated: usize,
    pub messages: usize,
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
    /// Minimum characters of a query term, so the UI need not hard-code it.
    pub min_query_length: usize,
}
