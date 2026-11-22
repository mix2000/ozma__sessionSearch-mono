//! `/api/agents/transcripts/*`: search, status, sync, context, resume.

use axum::extract::{Path, Query, State};
use axum::Json;
use session_search_core::{ContextMessage, ResumeResponse, SearchResponse, Status, SyncResult};

use super::{blocking, ApiResult};
use crate::http::dto::{context_seq, search_params, QueryMap};
use crate::http::AppState;

pub async fn search(
    State(st): State<AppState>,
    Query(q): Query<QueryMap>,
) -> ApiResult<SearchResponse> {
    let params = search_params(&q);
    let engine = st.engine.clone();
    blocking(move || engine.search(&params)).await.map(Json)
}

pub async fn status(State(st): State<AppState>) -> ApiResult<Status> {
    let engine = st.engine.clone();
    blocking(move || engine.status()).await.map(Json)
}

pub async fn sync(State(st): State<AppState>) -> ApiResult<SyncResult> {
    let engine = st.engine.clone();
    blocking(move || Ok(engine.sync())).await.map(Json)
}

pub async fn context(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<QueryMap>,
) -> ApiResult<Vec<ContextMessage>> {
    let seq = context_seq(&q)?;
    let engine = st.engine.clone();
    blocking(move || engine.context(&id, seq)).await.map(Json)
}

pub async fn resume(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<ResumeResponse> {
    let engine = st.engine.clone();
    blocking(move || engine.resume(&id)).await.map(Json)
}
