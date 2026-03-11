//! In-memory ring buffer for component log entries.
//!
//! Each call to `host.log()` inside a WASM component currently emits a tracing
//! event tagged with `service_id`, `workflow_id`, and `component_digest`. That
//! output ends up in the WAVS node's stdout/stderr mix with no way for a developer
//! to query "what did _this_ service log on trigger X?".
//!
//! This module provides `ComponentLogBuffer`: a bounded, per-service ring buffer
//! that captures every `host.log()` call alongside the regular tracing pipeline.
//! It is `Clone` (wraps an `Arc`) so a single instance can be shared cheaply
//! between the engine and the HTTP server state.
//!
//! `WavsDb` holds a `ComponentLogBuffer`; both `WasmEngine` and `HttpState`
//! receive the same `WavsDb` clone, so they see the same log data.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use wavs_types::{ComponentDigest, ServiceId, WorkflowId};

/// Default maximum number of log entries retained per service.
/// Once the buffer is full, the oldest entry is dropped.
pub const DEFAULT_MAX_LOG_ENTRIES_PER_SERVICE: usize = 500;

/// Log severity level, mirroring the WIT `log-level` enum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ComponentLogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl ComponentLogLevel {
    /// Numeric rank: higher = more severe. Used for minimum-level filtering.
    pub fn rank(&self) -> u8 {
        match self {
            ComponentLogLevel::Trace => 0,
            ComponentLogLevel::Debug => 1,
            ComponentLogLevel::Info => 2,
            ComponentLogLevel::Warn => 3,
            ComponentLogLevel::Error => 4,
        }
    }

    pub fn from_str_lossy(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "error" => Some(ComponentLogLevel::Error),
            "warn" | "warning" => Some(ComponentLogLevel::Warn),
            "info" => Some(ComponentLogLevel::Info),
            "debug" => Some(ComponentLogLevel::Debug),
            "trace" => Some(ComponentLogLevel::Trace),
            _ => None,
        }
    }
}

/// Whether the log entry came from an operator or aggregator component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ComponentKind {
    Operator,
    Aggregator,
}

/// A single captured log entry from a WASM component.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LogEntry {
    /// Unix milliseconds when the entry was captured.
    pub timestamp_ms: u64,
    pub level: ComponentLogLevel,
    /// String representation of the `ServiceId`.
    pub service_id: String,
    /// String representation of the `WorkflowId`.
    pub workflow_id: String,
    /// Hex-encoded component digest.
    pub digest: String,
    /// Hex-encoded `EventId` for this trigger execution.
    /// Present for aggregator components; `None` for operator components.
    pub event_id: Option<String>,
    pub component_kind: ComponentKind,
    pub message: String,
}

/// Bounded in-memory ring buffer of component log entries, keyed by `ServiceId`.
///
/// All fields are `Arc`-backed so cloning is `O(1)` and all clones share
/// the same underlying data.
#[derive(Clone)]
pub struct ComponentLogBuffer {
    entries: Arc<DashMap<ServiceId, VecDeque<LogEntry>>>,
    max_per_service: usize,
}

impl ComponentLogBuffer {
    pub fn new(max_per_service: usize) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            max_per_service,
        }
    }

    /// Append a log entry, dropping the oldest if the buffer is full.
    pub fn push(&self, service_id: ServiceId, entry: LogEntry) {
        let mut queue = self.entries.entry(service_id).or_default();
        if queue.len() >= self.max_per_service {
            queue.pop_front();
        }
        queue.push_back(entry);
    }

    /// Query log entries for a service with optional filters.
    ///
    /// Results are returned newest-first and capped at `limit`.
    pub fn query(
        &self,
        service_id: &ServiceId,
        since_ms: Option<u64>,
        min_level: Option<&ComponentLogLevel>,
        workflow_id: Option<&str>,
        limit: usize,
    ) -> Vec<LogEntry> {
        let min_rank = min_level.map(|l| l.rank()).unwrap_or(0);
        match self.entries.get(service_id) {
            None => vec![],
            Some(queue) => queue
                .iter()
                .filter(|e| {
                    let level_ok = e.level.rank() >= min_rank;
                    let since_ok = since_ms.map_or(true, |ms| e.timestamp_ms >= ms);
                    let wf_ok = workflow_id.map_or(true, |wf| e.workflow_id == wf);
                    level_ok && since_ok && wf_ok
                })
                .rev()
                .take(limit)
                .cloned()
                .collect(),
        }
    }

    /// Return all log entries associated with a specific trigger execution.
    pub fn query_by_event(&self, service_id: &ServiceId, event_id: &str) -> Vec<LogEntry> {
        match self.entries.get(service_id) {
            None => vec![],
            Some(queue) => queue
                .iter()
                .filter(|e| e.event_id.as_deref() == Some(event_id))
                .cloned()
                .collect(),
        }
    }

    /// Remove all log entries for a service (e.g., on service deletion).
    pub fn clear_service(&self, service_id: &ServiceId) {
        self.entries.remove(service_id);
    }
}

impl Default for ComponentLogBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_LOG_ENTRIES_PER_SERVICE)
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Build a `LogEntry` for an operator component invocation.
pub fn make_operator_log_entry(
    service_id: &ServiceId,
    workflow_id: &WorkflowId,
    digest: &ComponentDigest,
    level: ComponentLogLevel,
    message: String,
) -> LogEntry {
    LogEntry {
        timestamp_ms: current_timestamp_ms(),
        level,
        service_id: service_id.to_string(),
        workflow_id: workflow_id.to_string(),
        digest: digest.to_string(),
        event_id: None,
        component_kind: ComponentKind::Operator,
        message,
    }
}

/// Build a `LogEntry` for an aggregator component invocation.
pub fn make_aggregator_log_entry(
    service_id: &ServiceId,
    workflow_id: &WorkflowId,
    digest: &ComponentDigest,
    event_id: &wavs_types::EventId,
    level: ComponentLogLevel,
    message: String,
) -> LogEntry {
    LogEntry {
        timestamp_ms: current_timestamp_ms(),
        level,
        service_id: service_id.to_string(),
        workflow_id: workflow_id.to_string(),
        digest: digest.to_string(),
        event_id: Some(event_id.to_string()),
        component_kind: ComponentKind::Aggregator,
        message,
    }
}
