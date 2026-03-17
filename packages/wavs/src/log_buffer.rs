use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{Event, Subscriber};
use tracing_subscriber::{layer::Context, Layer};
use utoipa::ToSchema;

pub const DEFAULT_BROADCAST_CAPACITY: usize = 256;
pub const DEFAULT_LOG_BUFFER_CAPACITY: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp_ms: u64,
    pub level: String,
    pub target: String,
    pub fields: String,
}

pub struct LogBufferInner {
    entries: RwLock<VecDeque<LogEntry>>,
    next_id: AtomicU64,
    capacity: usize,
    tx: broadcast::Sender<LogEntry>,
}

pub type LogBuffer = Arc<LogBufferInner>;

impl LogBufferInner {
    pub fn new() -> LogBuffer {
        Self::with_capacity(DEFAULT_LOG_BUFFER_CAPACITY, DEFAULT_BROADCAST_CAPACITY)
    }

    pub fn with_capacity(capacity: usize, broadcast_capacity: usize) -> LogBuffer {
        let capacity = capacity.max(1);
        let broadcast_capacity = broadcast_capacity.max(1);
        let (tx, _) = broadcast::channel(broadcast_capacity);
        Arc::new(Self {
            entries: RwLock::new(VecDeque::with_capacity(capacity.min(1024))),
            next_id: AtomicU64::new(0),
            capacity,
            tx,
        })
    }

    /// Push an entry into the buffer. The `id` field of the entry is
    /// overwritten with a monotonically increasing value assigned under the
    /// write lock, guaranteeing that entries are always stored in id order.
    /// Returns the id that was assigned to the entry.
    pub fn push(&self, mut entry: LogEntry) -> u64 {
        let mut entries = self.entries.write().unwrap();
        entry.id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let id = entry.id;
        let _ = self.tx.send(entry.clone()); // broadcast with correct id
        if entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(entry);
        id
    }

    /// Returns the id that will be assigned to the next pushed entry.
    /// Useful as a cursor baseline when no entries match a filter.
    pub fn watermark(&self) -> u64 {
        self.next_id.load(Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.tx.subscribe()
    }

    /// Returns entries with id >= since_id, up to limit, filtered by min level and target prefix.
    pub fn since(
        &self,
        since_id: u64,
        limit: usize,
        min_level: Option<&str>,
        target: Option<&str>,
    ) -> Vec<LogEntry> {
        let min_level_val = min_level.map(level_value);
        let entries = self.entries.read().unwrap();
        entries
            .iter()
            .filter(|e| e.id >= since_id)
            .filter(|e| {
                min_level_val
                    .map(|min| level_value(&e.level) >= min)
                    .unwrap_or(true)
            })
            .filter(|e| target.map(|t| e.target.starts_with(t)).unwrap_or(true))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Returns the last `n` entries for SSE replay on connect.
    pub fn last_n(&self, n: usize) -> Vec<LogEntry> {
        let entries = self.entries.read().unwrap();
        let skip = entries.len().saturating_sub(n);
        entries.iter().skip(skip).cloned().collect()
    }
}

/// Returns a numeric severity value: higher = more severe.
pub fn level_value(level: &str) -> u8 {
    if level.eq_ignore_ascii_case("error") {
        4
    } else if level.eq_ignore_ascii_case("warn") {
        3
    } else if level.eq_ignore_ascii_case("info") {
        2
    } else if level.eq_ignore_ascii_case("debug") {
        1
    } else if level.eq_ignore_ascii_case("trace") {
        0
    } else {
        2
    }
}

pub struct InMemoryLogLayer {
    buffer: LogBuffer,
}

impl InMemoryLogLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

// Field visitor to format event fields (same pattern as app/src-tauri/src/logger.rs TauriLogLayer)
struct FieldFmt(String);

impl tracing::field::Visit for FieldFmt {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if !self.0.is_empty() {
            self.0.push_str(", ");
        }
        self.0.push_str(field.name());
        self.0.push('=');
        self.0.push_str(&format!("{value:?}"));
    }
}

impl<S: Subscriber> Layer<S> for InMemoryLogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut v = FieldFmt(String::new());
        event.record(&mut v);

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let entry = LogEntry {
            id: 0, // assigned by push() under the write lock
            timestamp_ms,
            level: meta.level().to_string(),
            target: meta.target().to_string(),
            fields: v.0,
        };

        self.buffer.push(entry);
    }
}
