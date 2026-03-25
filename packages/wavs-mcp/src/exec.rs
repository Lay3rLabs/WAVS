//! Execution tool foundations: types, error codes, schema merging, service cache,
//! ExecContext, PendingConfirmations, and tool name sanitization.
//!
//! This module provides the public API that Plans 02 and 03 depend on for
//! wiring execution tools into the MCP server.

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::client::WavsClient;

// ── Type alias ────────────────────────────────────────────────────────────

/// Re-use the MCP error type from rmcp.
pub type McpError = rmcp::model::ErrorData;

// ── Trust tiers (D-05, D-06, D-07, EXEC-05) ──────────────────────────────

/// Trust tier for execution tool calls.
///
/// - `ResultOnly` — raw component output, no cryptographic wrapper.
/// - `SignedResult` — component output wrapped with operator signature.
/// - `OnChain` — component output submitted on-chain; returns tx hash.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    ResultOnly,
    SignedResult,
    OnChain,
}

// ── Error code constants (D-13) ──────────────────────────────────────────

pub const ERR_EXECUTION_TIMEOUT: &str = "EXECUTION_TIMEOUT";
pub const ERR_TIER_NOT_ENABLED: &str = "TIER_NOT_ENABLED";
pub const ERR_SERVICE_NOT_FOUND: &str = "SERVICE_NOT_FOUND";
pub const ERR_COMPONENT_FAILED: &str = "COMPONENT_FAILED";
pub const ERR_SIGNING_FAILED: &str = "SIGNING_FAILED";
pub const ERR_SUBMISSION_FAILED: &str = "SUBMISSION_FAILED";

// ── Timeout constants (EXEC-08, D-14) ────────────────────────────────────

/// Maximum per-call timeout in milliseconds.
pub const MAX_TIMEOUT_MS: u64 = 25_000;

/// Default per-call timeout in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 25_000;

// ── Structured error helper (D-13, D-15) ─────────────────────────────────

/// Return a structured MCP error result with an error code, message, and
/// optional partial result (hex-encoded payload from a successful component
/// execution that failed at a later stage such as signing or submission).
pub fn exec_error(
    code: &str,
    message: &str,
    partial_result: Option<&[u8]>,
) -> Result<CallToolResult, McpError> {
    let mut error = serde_json::json!({
        "error_code": code,
        "message": message,
    });

    // D-15: include raw result if component execution succeeded
    if let Some(payload) = partial_result {
        error["partial_result"] = serde_json::json!({
            "payload": const_hex::encode(payload),
        });
    }

    Ok(CallToolResult {
        content: vec![Content::text(
            serde_json::to_string_pretty(&error).unwrap_or_else(|_| error.to_string()),
        )],
        is_error: Some(true),
    })
}

// ── Tool name sanitization (Pitfall 3) ───────────────────────────────────

/// Sanitize a free-form string into a valid MCP tool name fragment.
///
/// Rules: lowercase, replace non-alphanumeric with `_`, collapse consecutive
/// underscores, trim leading/trailing `_`, truncate to 64 chars.
pub fn sanitize_tool_name(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut last_was_underscore = true; // prevents leading underscore

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
            last_was_underscore = false;
        } else if !last_was_underscore {
            result.push('_');
            last_was_underscore = true;
        }
    }

    // Trim trailing underscore
    while result.ends_with('_') {
        result.pop();
    }

    // Truncate to 64 chars (on a char boundary, though we only have ASCII)
    result.truncate(64);

    // Trim trailing underscore again if truncation exposed one
    while result.ends_with('_') {
        result.pop();
    }

    result
}

// ── Schema merging (EXEC-05, D-14, Pitfall 1) ───────────────────────────

/// Merge a WIT-derived `inputSchema` with execution meta-parameters
/// (`trust_tier`, `timeout_ms`, `confirm`) to produce the final MCP tool
/// `inputSchema`.
///
/// The WIT params are nested under an `"input"` property to avoid name
/// collisions between component parameters and meta-parameters.
pub fn merge_exec_schema(wit_input_schema: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "input": wit_input_schema,
            "trust_tier": {
                "type": "string",
                "enum": ["result_only", "signed_result", "on_chain"],
                "description": "Trust level for this execution. result_only: raw component output. signed_result: output + operator signature. on_chain: submit result as blockchain transaction.",
                "default": "result_only"
            },
            "timeout_ms": {
                "type": "integer",
                "description": "Per-call timeout in milliseconds (max 25000).",
                "default": DEFAULT_TIMEOUT_MS,
                "maximum": MAX_TIMEOUT_MS
            },
            "confirm": {
                "type": "string",
                "description": "For on_chain tier: pass the nonce from the gas estimate response to confirm and submit the transaction."
            }
        },
        "required": ["trust_tier"]
    })
}

// ── Service cache (D-04, Pattern 3) ──────────────────────────────────────

/// Thread-safe service list cache with a configurable TTL.
///
/// The cached value is the raw JSON from `GET /services` on the WAVS node.
/// Both `list_tools()` (for dynamic tool generation) and `call_tool()` (for
/// service lookup) share the same cache instance.
pub struct ServiceCache {
    inner: RwLock<Option<CachedServices>>,
    ttl: Duration,
}

struct CachedServices {
    services: serde_json::Value,
    fetched_at: Instant,
}

impl ServiceCache {
    /// Create a new cache with the given time-to-live.
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: RwLock::new(None),
            ttl,
        }
    }

    /// Return the cached service list if it exists and is not stale.
    pub async fn get(&self) -> Option<serde_json::Value> {
        let guard = self.inner.read().await;
        guard.as_ref().and_then(|cached| {
            if cached.fetched_at.elapsed() < self.ttl {
                Some(cached.services.clone())
            } else {
                None
            }
        })
    }

    /// Store a fresh service list in the cache.
    pub async fn set(&self, services: serde_json::Value) {
        let mut guard = self.inner.write().await;
        *guard = Some(CachedServices {
            services,
            fetched_at: Instant::now(),
        });
    }

    /// Immediately invalidate the cache (e.g. after deploy/delete).
    pub async fn invalidate(&self) {
        let mut guard = self.inner.write().await;
        *guard = None;
    }
}

// ── ExecContext ───────────────────────────────────────────────────────────

/// Extensible context passed to `handle_exec_tool()` so that the function
/// signature does not need to change when Plan 03 adds fields (e.g.
/// signing credentials, pending confirmations).
pub struct ExecContext<'a> {
    /// HTTP client for the WAVS node.
    pub client: &'a WavsClient,
    /// Cached service list JSON from `GET /services`.
    pub services_json: &'a serde_json::Value,
    /// Available after Plan 03 adds signing support.
    pub signing_mnemonic: Option<&'a wavs_types::Credential>,
    /// Available after Plan 03 adds on-chain submission.
    pub mcp_chain_credential: Option<&'a wavs_types::Credential>,
    /// Shared pending confirmations cache for Tier 3 two-step flow.
    pub pending_confirmations: Option<&'a PendingConfirmations>,
}

// ── PendingConfirmations (D-09) ──────────────────────────────────────────

/// A pending execution awaiting user confirmation for on-chain submission.
pub struct PendingExecution {
    pub service_id: String,
    pub workflow_id: String,
    pub payload: Vec<u8>,
    pub gas_estimate: String,
    pub chain_id: String,
    pub created_at: Instant,
}

/// Thread-safe store for pending Tier 3 executions awaiting confirmation.
///
/// Each entry is keyed by a hex nonce and auto-expires after 60 seconds.
pub struct PendingConfirmations {
    inner: RwLock<HashMap<String, PendingExecution>>,
}

impl PendingConfirmations {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Store a pending execution and return the nonce the agent must send
    /// back to confirm submission.
    pub async fn store(&self, execution: PendingExecution) -> String {
        let nonce = format!(
            "{:016x}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        );
        self.inner.write().await.insert(nonce.clone(), execution);
        nonce
    }

    /// Take (remove) a pending execution by nonce, garbage-collecting any
    /// entries older than 60 seconds.
    pub async fn take(&self, nonce: &str) -> Option<PendingExecution> {
        let mut map = self.inner.write().await;
        // Garbage-collect expired entries
        map.retain(|_, v| v.created_at.elapsed() < Duration::from_secs(60));
        map.remove(nonce)
    }
}

impl Default for PendingConfirmations {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_basic() {
        assert_eq!(sanitize_tool_name("My Service!"), "my_service");
        assert_eq!(sanitize_tool_name("hello-world"), "hello_world");
        assert_eq!(sanitize_tool_name("___leading"), "leading");
        assert_eq!(sanitize_tool_name("trailing___"), "trailing");
        assert_eq!(sanitize_tool_name("a--b..c"), "a_b_c");
    }

    #[test]
    fn sanitize_truncation() {
        let long = "a".repeat(100);
        let sanitized = sanitize_tool_name(&long);
        assert!(sanitized.len() <= 64);
    }

    #[test]
    fn merge_schema_has_required_fields() {
        let wit = serde_json::json!({"type": "object", "properties": {"msg": {"type": "string"}}});
        let merged = merge_exec_schema(wit);
        let obj = merged.as_object().unwrap();
        assert!(obj.contains_key("properties"));
        let props = obj["properties"].as_object().unwrap();
        assert!(props.contains_key("input"));
        assert!(props.contains_key("trust_tier"));
        assert!(props.contains_key("timeout_ms"));
        assert!(props.contains_key("confirm"));
        let required = obj["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("trust_tier")));
    }
}
