use std::time::SystemTime;

use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{filter::LevelFilter, Layer};
use tracing_web::{performance_layer, MakeWebConsoleWriter};
use wasm_bindgen::prelude::*;

use crate::state::AppState;

pub fn init_logger(state: AppState) {
    static LOGGER_INITIALIZED: std::sync::Once = std::sync::Once::new();

    LOGGER_INITIALIZED.call_once(move || {
        set_stack_trace_limit(30);

        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_ansi(false) // Only partially supported across JavaScript runtimes
            .without_time()
            .with_level(true)
            .with_target(false)
            .with_writer(MakeWebConsoleWriter::new().with_pretty_level()); // write events to the console

        let perf_layer = performance_layer().with_details_from_fields(Pretty::default());

        let log_layer = LogLayer::new(state);

        let level_filter = LevelFilter::DEBUG;

        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(perf_layer)
            .with(log_layer)
            .with(level_filter)
            .init();

        tracing::info!("(info) Logger initialized");
        tracing::debug!("(debug) Logger initialized");

        std::panic::set_hook(Box::new(tracing_panic::panic_hook));
    });
}

#[wasm_bindgen(
    inline_js = "export function set_stack_trace_limit(limit) { Error.stackTraceLimit = limit; }"
)]
extern "C" {
    fn set_stack_trace_limit(limit: u32);
}

struct LogLayer {
    state: AppState,
}

impl LogLayer {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

impl<S> Layer<S> for LogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut v = FieldFmt(String::new());
        event.record(&mut v);

        // Use js_sys::Date for WASM-compatible time
        let ts = {
            let millis = js_sys::Date::now();
            let secs = (millis / 1000.0) as u64;
            let nanos = ((millis % 1000.0) * 1_000_000.0) as u32;
            std::time::UNIX_EPOCH + std::time::Duration::new(secs, nanos)
        };

        self.state.log_list.lock_mut().push_cloned(LogItem {
            ts,
            level: *meta.level(),
            target: meta.target().to_string(),
            fields: v.0,
        });
    }
}

#[derive(Clone, Debug)]
pub struct LogItem {
    pub ts: SystemTime,
    pub level: Level,
    pub target: String,
    pub fields: String, // or a map if you want structured fields
}

// Visitor to format event fields
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
