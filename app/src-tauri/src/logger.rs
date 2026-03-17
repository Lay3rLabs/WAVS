use tauri::{AppHandle, Emitter};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;
use wavs_gui_shared::event::LogEvent;

pub struct TauriLogLayer {
    app: AppHandle,
}

impl TauriLogLayer {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl<S> Layer<S> for TauriLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut v = FieldFmt(String::new());
        event.record(&mut v);

        let log_event = LogEvent {
            level: meta.level().to_string(),
            target: meta.target().to_string(),
            fields: v.0,
        };

        // Emit to frontend - ignore errors if frontend isn't listening yet
        let _ = self.app.emit("log", log_event);
    }
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
