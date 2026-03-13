use std::{collections::HashSet, convert::Infallible};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use utoipa::ToSchema;

use crate::{
    http::state::HttpState,
    log_buffer::{level_value, LogEntry},
};

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 1000;
const SSE_REPLAY_COUNT: usize = 50;
const VALID_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];

fn validate_level(level: Option<&str>) -> Result<(), (StatusCode, String)> {
    if let Some(l) = level {
        if !VALID_LEVELS.contains(&l.to_lowercase().as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "invalid level {:?}; must be one of: {}",
                    l,
                    VALID_LEVELS.join(", ")
                ),
            ));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct LogsQuery {
    #[serde(default)]
    pub since_id: u64,
    pub limit: Option<usize>,
    pub level: Option<String>,
    pub target: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct LogsResponse {
    pub entries: Vec<LogEntry>,
    /// Pass this as `since_id` on the next poll to receive only new entries.
    pub next_id: u64,
}

#[utoipa::path(
    get,
    path = "/dev/logs",
    params(
        ("since_id" = Option<u64>, Query, description = "Return entries with id >= since_id (default: 0)"),
        ("limit" = Option<usize>, Query, description = "Maximum entries to return (default: 100, max: 1000)"),
        ("level" = Option<String>, Query, description = "Minimum log level filter: trace/debug/info/warn/error"),
        ("target" = Option<String>, Query, description = "Filter by target prefix (e.g. 'wavs')"),
    ),
    responses(
        (status = 200, description = "Structured log entries from the in-memory ring buffer", body = LogsResponse),
        (status = 400, description = "Invalid query parameter"),
    ),
    description = "Poll structured log entries. Pass the returned `next_id` as `since_id` on subsequent calls to receive only new entries."
)]
#[axum::debug_handler]
pub async fn handle_logs(
    State(state): State<HttpState>,
    Query(params): Query<LogsQuery>,
) -> impl IntoResponse {
    if let Err((status, msg)) = validate_level(params.level.as_deref()) {
        return (status, msg).into_response();
    }
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let entries = state.log_buffer.since(
        params.since_id,
        limit,
        params.level.as_deref(),
        params.target.as_deref(),
    );
    // If no entries match the filter, advance to the buffer watermark so
    // polling clients don't get stuck re-scanning the same window.
    let next_id = entries
        .iter()
        .map(|e| e.id)
        .max()
        .map(|id| id + 1)
        .unwrap_or_else(|| state.log_buffer.watermark());
    Json(LogsResponse { entries, next_id }).into_response()
}

#[derive(Deserialize)]
pub struct LogsStreamQuery {
    pub level: Option<String>,
    pub target: Option<String>,
}

#[utoipa::path(
    get,
    path = "/dev/logs/stream",
    params(
        ("level" = Option<String>, Query, description = "Minimum log level filter: trace/debug/info/warn/error"),
        ("target" = Option<String>, Query, description = "Filter by target prefix (e.g. 'wavs')"),
    ),
    responses(
        (status = 200, description = "Server-Sent Events stream of log entries (JSON per event)"),
        (status = 400, description = "Invalid query parameter"),
    ),
    description = "Subscribe to a real-time SSE stream of structured log entries. Replays the last 50 buffered entries on connect, then streams live."
)]
#[axum::debug_handler]
pub async fn handle_logs_stream(
    State(state): State<HttpState>,
    Query(params): Query<LogsStreamQuery>,
) -> axum::response::Response {
    if let Err((status, msg)) = validate_level(params.level.as_deref()) {
        return (status, msg).into_response();
    }

    let level = params.level.clone();
    let target = params.target.clone();

    // Subscribe before reading buffered entries so no live events are missed.
    // Track the exact set of replayed IDs so that only those specific entries
    // are deduplicated from the live stream; a max-id cutoff would incorrectly
    // drop live entries that arrived between subscribe() and last_n() but were
    // not captured in the replay snapshot.
    let mut rx = state.log_buffer.subscribe();
    let replay = state.log_buffer.last_n(SSE_REPLAY_COUNT);
    let replayed_ids: HashSet<u64> = replay.iter().map(|e| e.id).collect();

    let stream = async_stream::stream! {
        // Replay recent buffered entries
        for entry in replay {
            if matches_filters(&entry, level.as_deref(), target.as_deref()) {
                if let Ok(data) = serde_json::to_string(&entry) {
                    yield Ok::<Event, Infallible>(Event::default().data(data));
                }
            }
        }

        // Stream live entries, skipping any already covered by the replay
        loop {
            match rx.recv().await {
                Ok(entry) => {
                    if replayed_ids.contains(&entry.id) {
                        continue;
                    }
                    if matches_filters(&entry, level.as_deref(), target.as_deref()) {
                        if let Ok(data) = serde_json::to_string(&entry) {
                            yield Ok(Event::default().data(data));
                        }
                    }
                }
                Err(RecvError::Lagged(_)) => {
                    // Receiver fell behind; continue without emitting a gap notification
                }
                Err(RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn matches_filters(entry: &LogEntry, min_level: Option<&str>, target: Option<&str>) -> bool {
    if let Some(l) = min_level {
        if level_value(&entry.level) < level_value(l) {
            return false;
        }
    }
    if let Some(t) = target {
        if !entry.target.starts_with(t) {
            return false;
        }
    }
    true
}
