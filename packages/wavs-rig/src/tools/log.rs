//! LogTool — log a message via wasi:logging to the WAVS host.
//!
//! Since wavs-rig is an rlib (not a cdylib), it cannot call component-specific
//! `host::log()` directly. Instead, LogTool writes to stderr via eprintln!, which
//! the WAVS runtime captures and routes through its logging subsystem.

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::Deserialize;

// ─── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum LogToolError {
    #[error("Log error: {0}")]
    LogError(String),
}

// ─── LogTool ──────────────────────────────────────────────────────────────────

/// Arguments for LogTool: level string and message text.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogArgs {
    /// Log level: "trace", "debug", "info", "warn", or "error".
    /// Defaults to "info" if unrecognized.
    pub level: String,
    /// The message to log.
    pub message: String,
}

/// Log a message to the WAVS host logging system.
///
/// Writes to stderr which the WAVS runtime captures and forwards to the
/// configured tracing subscriber. The level string controls severity formatting.
pub struct LogTool;

impl Tool for LogTool {
    const NAME: &'static str = "log";

    type Error = LogToolError;
    type Args = LogArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Log a message to WAVS host logging. Level can be: trace, debug, info, warn, error.".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(LogArgs))
                .unwrap_or(serde_json::Value::Null),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let level = args.level.to_lowercase();
        let level_str = match level.as_str() {
            "trace" => "TRACE",
            "debug" => "DEBUG",
            "warn" | "warning" => "WARN",
            "error" => "ERROR",
            _ => "INFO", // default to INFO for unrecognized levels
        };

        // Write to stderr — the WAVS runtime captures this and routes to wasi:logging.
        // This is the standard logging path for rlib components that cannot call
        // host::log() directly (which is only available in cdylib component worlds).
        eprintln!("[wavs-rig] {}: {}", level_str, args.message);

        Ok(args.message)
    }
}
