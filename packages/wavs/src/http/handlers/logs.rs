//! HTTP handlers for `GET /dev/logs/{service_id}` and
//! `GET /dev/logs/{service_id}/events/{event_id}`.
//!
//! These endpoints expose the in-memory component log ring buffer so developers
//! can query what a WASM component emitted via `host.log()` without tailing
//! the WAVS node's raw stdout.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::str::FromStr;
use utils::storage::log_buffer::ComponentLogLevel;
use wavs_types::ServiceId;

use crate::http::state::HttpState;

// ---------------------------------------------------------------------------
// Query parameter types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// Return only entries at or after this Unix millisecond timestamp.
    pub since: Option<u64>,
    /// Minimum log level to include (error | warn | info | debug | trace).
    pub level: Option<String>,
    /// Restrict to a specific workflow within the service.
    pub workflow_id: Option<String>,
    /// Maximum number of entries to return (default 50, max 500).
    pub limit: Option<usize>,
}

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// List log entries for a service.
///
/// `GET /dev/logs/{service_id}?since=<ms>&level=<level>&workflow_id=<id>&limit=<n>`
///
/// Results are returned newest-first, capped at `limit` (default 50, max 500).
#[utoipa::path(
    get,
    path = "/dev/logs/{service_id}",
    params(
        ("service_id" = String, Path, description = "Service ID"),
        ("since"       = Option<u64>, Query, description = "Unix millisecond lower bound"),
        ("level"       = Option<String>, Query, description = "Minimum level: error|warn|info|debug|trace"),
        ("workflow_id" = Option<String>, Query, description = "Filter by workflow ID"),
        ("limit"       = Option<usize>, Query, description = "Max entries (default 50, max 500)"),
    ),
    responses(
        (status = 200, description = "Log entries", body = Vec<utils::storage::log_buffer::LogEntry>),
    ),
    description = "Query buffered component log entries for a service"
)]
#[axum::debug_handler]
pub async fn handle_get_logs(
    State(state): State<HttpState>,
    Path(service_id_str): Path<String>,
    Query(params): Query<LogsQuery>,
) -> impl IntoResponse {
    let service_id =
        match ServiceId::from_str(&service_id_str) {
            Ok(id) => id,
            Err(_) => return (
                StatusCode::BAD_REQUEST,
                Json(
                    serde_json::json!({"error": "invalid service_id: expected 32-byte hex string"}),
                ),
            )
                .into_response(),
        };

    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    let min_level = params
        .level
        .as_deref()
        .and_then(ComponentLogLevel::from_str_lossy);

    let entries = state.db_storage.component_logs.query(
        &service_id,
        params.since,
        min_level.as_ref(),
        params.workflow_id.as_deref(),
        limit,
    );

    (StatusCode::OK, Json(entries)).into_response()
}

/// List log entries for a specific trigger execution.
///
/// `GET /dev/logs/{service_id}/events/{event_id}`
///
/// Returns all buffered log entries (operator + aggregator) whose `event_id`
/// matches. Operator logs don't carry an `event_id`, so only aggregator
/// entries will appear here.
#[utoipa::path(
    get,
    path = "/dev/logs/{service_id}/events/{event_id}",
    params(
        ("service_id" = String, Path, description = "Service ID"),
        ("event_id"   = String, Path, description = "Hex-encoded EventId (20 bytes)"),
    ),
    responses(
        (status = 200, description = "Log entries for the trigger execution",
         body = Vec<utils::storage::log_buffer::LogEntry>),
    ),
    description = "Query buffered component log entries for a specific trigger execution"
)]
#[axum::debug_handler]
pub async fn handle_get_logs_by_event(
    State(state): State<HttpState>,
    Path((service_id_str, event_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let service_id =
        match ServiceId::from_str(&service_id_str) {
            Ok(id) => id,
            Err(_) => return (
                StatusCode::BAD_REQUEST,
                Json(
                    serde_json::json!({"error": "invalid service_id: expected 32-byte hex string"}),
                ),
            )
                .into_response(),
        };

    let entries = state
        .db_storage
        .component_logs
        .query_by_event(&service_id, &event_id);

    (StatusCode::OK, Json(entries)).into_response()
}
