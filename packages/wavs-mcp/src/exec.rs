//! Execution tool pipeline: dynamic tool generation from deployed services,
//! Tier 1 (result_only) execution dispatch, types, error codes, schema merging,
//! service cache, ExecContext, PendingConfirmations, and tool name sanitization.
//!
//! This module provides the public API for wiring execution tools into the MCP
//! server: `build_exec_tools()` generates Tool definitions from the service list,
//! and `handle_exec_tool()` dispatches `wavs_exec_*` tool calls through the
//! WAVS node's `/dev/execute` endpoint.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use rmcp::model::{CallToolResult, Content, ErrorCode, Tool};
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

// ── Dynamic tool generation (D-01, D-02, D-03, EXEC-01) ─────────────────

/// Extract a human-readable component source description from a workflow JSON.
fn component_source_desc(workflow: &serde_json::Value) -> String {
    let source = &workflow["component"]["source"];

    if let Some(uri) = source["oci"]["uri"].as_str() {
        return uri.to_string();
    }
    if let Some(digest) = source["digest"].as_str() {
        let short = if digest.len() > 12 {
            &digest[..12]
        } else {
            digest
        };
        return format!("component:{short}");
    }
    if let Some(uri) = source["download"]["uri"].as_str() {
        return uri.to_string();
    }

    "local".to_string()
}

/// Build MCP Tool definitions for all deployed service workflows.
///
/// Each service workflow gets one tool named `wavs_exec_{sanitized_service_name}_{workflow_id}`.
/// The `services_json` is the response from `GET /services` on the WAVS node --
/// a JSON object where each key is a service identifier.
pub fn build_exec_tools(services_json: &serde_json::Value) -> Vec<Tool> {
    let mut tools = Vec::new();

    let services = match services_json.as_object() {
        Some(obj) => obj,
        None => return tools,
    };

    for (_service_id, service) in services {
        let service_name = service["name"].as_str().unwrap_or("unknown");
        let workflows = match service["workflows"].as_object() {
            Some(w) => w,
            None => continue,
        };

        for (workflow_id, workflow) in workflows {
            let sanitized_name = sanitize_tool_name(service_name);
            let tool_name = format!("wavs_exec_{sanitized_name}_{workflow_id}");

            let source_desc = component_source_desc(workflow);
            let description = format!(
                "Execute {service_name} workflow '{workflow_id}'. Source: {source_desc}. \
                 Supports trust tiers: result_only, signed_result, on_chain."
            );

            // Build a permissive input schema (generic object) since the MCP server
            // does not have access to the component bytes for full WIT parsing.
            let wit_schema = serde_json::json!({
                "type": "object",
                "description": "Input data to pass to the component. Structure depends on the component's WIT interface.",
                "additionalProperties": true
            });
            let input_schema = merge_exec_schema(wit_schema);

            // Convert the merged schema Value to the Arc<Map> format rmcp expects.
            let schema_map: Arc<serde_json::Map<String, serde_json::Value>> =
                Arc::new(input_schema.as_object().cloned().unwrap_or_default());

            tools.push(Tool {
                name: tool_name.into(),
                description: description.into(),
                input_schema: schema_map,
            });
        }
    }

    tools
}

// ── Service resolution ───────────────────────────────────────────────────

/// Resolve a `wavs_exec_*` tool name back to the service and workflow it targets.
///
/// Returns `(service_id_hex, workflow_id, service_name, component_source_desc)`.
fn resolve_tool_service(
    tool_name: &str,
    services_json: &serde_json::Value,
) -> Option<(String, String, String, String)> {
    let suffix = tool_name.strip_prefix("wavs_exec_")?;

    let services = services_json.as_object()?;

    for (service_id, service) in services {
        let service_name = service["name"].as_str().unwrap_or("unknown");
        let sanitized_name = sanitize_tool_name(service_name);
        let workflows = service["workflows"].as_object()?;

        for (workflow_id, workflow) in workflows {
            let expected = format!("{sanitized_name}_{workflow_id}");
            if suffix == expected {
                return Some((
                    service_id.clone(),
                    workflow_id.clone(),
                    service_name.to_string(),
                    component_source_desc(workflow),
                ));
            }
        }
    }

    None
}

// ── Tier 1 execution dispatch (EXEC-02, EXEC-08, D-14) ──────────────────

/// Handle a `wavs_exec_*` tool call. Extracts trust_tier, timeout, and input
/// from args, then executes the component via the WAVS node's `/dev/execute`
/// endpoint.
///
/// This function handles Tier 1 (`result_only`) directly. Tier 2 and 3 return
/// placeholder errors until Plan 03 adds support.
pub async fn handle_exec_tool(
    ctx: &ExecContext<'_>,
    tool_name: &str,
    args: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<CallToolResult, McpError> {
    let args_map = args.unwrap_or_default();

    // 1. Parse trust_tier (required)
    let trust_tier: TrustTier = match args_map.get("trust_tier") {
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| McpError {
            code: ErrorCode::INVALID_PARAMS,
            message: format!(
                "Invalid trust_tier: {e}. Must be one of: result_only, signed_result, on_chain"
            )
            .into(),
            data: None,
        })?,
        None => {
            return Err(McpError {
                code: ErrorCode::INVALID_PARAMS,
                message: "Missing required parameter: trust_tier".into(),
                data: None,
            });
        }
    };

    // 2. Parse timeout_ms (optional, default DEFAULT_TIMEOUT_MS, clamp to MAX_TIMEOUT_MS)
    let timeout_ms: u64 = match args_map.get("timeout_ms") {
        Some(v) => {
            let raw = v.as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);
            raw.min(MAX_TIMEOUT_MS)
        }
        None => DEFAULT_TIMEOUT_MS,
    };

    // 3. Parse input (optional, defaults to empty object)
    let input = args_map
        .get("input")
        .cloned()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    // 4. Resolve service and workflow from tool name
    let (service_id, workflow_id, service_name, _source_desc) =
        resolve_tool_service(tool_name, ctx.services_json).ok_or_else(|| {
            // Return as a tool result error, not an MCP protocol error
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                message: format!(
                    "No service found for tool '{tool_name}'. \
                     The service may have been removed. Call tools/list to refresh."
                )
                .into(),
                data: None,
            }
        })?;

    // 5. Dispatch by trust tier
    match trust_tier {
        TrustTier::ResultOnly => {
            // Build trigger and data JSON for the /dev/execute endpoint
            let trigger = serde_json::json!({"manual": null});

            // Serialize input to bytes for the Raw data variant
            let input_bytes = serde_json::to_vec(&input).unwrap_or_default();
            let data = serde_json::json!({"Raw": input_bytes});

            // Execute with timeout
            let execute_fut =
                ctx.client
                    .execute_component(&service_id, &workflow_id, &trigger, &data);

            let result = match tokio::time::timeout(
                Duration::from_millis(timeout_ms),
                execute_fut,
            )
            .await
            {
                Err(_elapsed) => {
                    return exec_error(
                        ERR_EXECUTION_TIMEOUT,
                        &format!(
                            "Component execution timed out after {timeout_ms}ms"
                        ),
                        None,
                    );
                }
                Ok(Err(e)) => {
                    return exec_error(
                        ERR_COMPONENT_FAILED,
                        &format!(
                            "Component execution failed for {service_name}/{workflow_id}: {e:#}"
                        ),
                        None,
                    );
                }
                Ok(Ok(responses)) => responses,
            };

            // Extract the first WasmResponse payload
            if result.is_empty() {
                return exec_error(
                    ERR_COMPONENT_FAILED,
                    "Component returned no responses",
                    None,
                );
            }

            // The response is a Vec<Value> where each item has a "payload" field (hex bytes)
            let first = &result[0];
            let payload_display = if let Some(payload) = first.get("payload") {
                // payload is typically a hex string or array of bytes
                if let Some(hex_str) = payload.as_str() {
                    // Try to decode hex to UTF-8 for display
                    match const_hex::decode(hex_str) {
                        Ok(bytes) => match String::from_utf8(bytes.clone()) {
                            Ok(text) => text,
                            Err(_) => format!("0x{hex_str}"),
                        },
                        Err(_) => hex_str.to_string(),
                    }
                } else if let Some(arr) = payload.as_array() {
                    // Array of byte values
                    let bytes: Vec<u8> = arr
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                        .collect();
                    match String::from_utf8(bytes.clone()) {
                        Ok(text) => text,
                        Err(_) => format!("0x{}", const_hex::encode(&bytes)),
                    }
                } else {
                    serde_json::to_string_pretty(payload)
                        .unwrap_or_else(|_| payload.to_string())
                }
            } else {
                // No "payload" field -- return the full response object
                serde_json::to_string_pretty(first)
                    .unwrap_or_else(|_| first.to_string())
            };

            Ok(CallToolResult {
                content: vec![Content::text(payload_display)],
                is_error: Some(false),
            })
        }

        TrustTier::SignedResult => exec_error(
            ERR_TIER_NOT_ENABLED,
            "Tier signed_result is not yet implemented -- coming in next update",
            None,
        ),

        TrustTier::OnChain => exec_error(
            ERR_TIER_NOT_ENABLED,
            "Tier on_chain is not yet implemented -- coming in next update",
            None,
        ),
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

    #[test]
    fn build_exec_tools_generates_tools_from_services() {
        let services = serde_json::json!({
            "abc123": {
                "name": "My Echo Service",
                "workflows": {
                    "default": {
                        "component": {
                            "source": {"digest": "f0b42a5171c9dcd75eac41c8ce2c4e7882d304c885266d8ac7b70af996b9a420"}
                        }
                    }
                }
            }
        });
        let tools = build_exec_tools(&services);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "wavs_exec_my_echo_service_default");
        let desc: &str = tools[0].description.as_ref();
        assert!(desc.contains("My Echo Service"));
        assert!(desc.contains("component:f0b42a5171c9"));
    }

    #[test]
    fn build_exec_tools_empty_services() {
        let tools = build_exec_tools(&serde_json::json!({}));
        assert!(tools.is_empty());
    }

    #[test]
    fn build_exec_tools_multiple_workflows() {
        let services = serde_json::json!({
            "svc1": {
                "name": "Multi-Workflow",
                "workflows": {
                    "default": {
                        "component": {"source": {"digest": "aabb"}}
                    },
                    "secondary": {
                        "component": {"source": {"oci": {"uri": "ghcr.io/foo/bar:latest"}}}
                    }
                }
            }
        });
        let tools = build_exec_tools(&services);
        assert_eq!(tools.len(), 2);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"wavs_exec_multi_workflow_default"));
        assert!(names.contains(&"wavs_exec_multi_workflow_secondary"));
    }

    #[test]
    fn resolve_tool_service_finds_match() {
        let services = serde_json::json!({
            "abc123": {
                "name": "Echo Service",
                "workflows": {
                    "default": {
                        "component": {"source": {"digest": "deadbeef"}}
                    }
                }
            }
        });
        let result =
            resolve_tool_service("wavs_exec_echo_service_default", &services);
        assert!(result.is_some());
        let (sid, wid, name, _source) = result.unwrap();
        assert_eq!(sid, "abc123");
        assert_eq!(wid, "default");
        assert_eq!(name, "Echo Service");
    }

    #[test]
    fn resolve_tool_service_returns_none_for_unknown() {
        let services = serde_json::json!({
            "abc123": {
                "name": "Echo Service",
                "workflows": {
                    "default": {
                        "component": {"source": {"digest": "deadbeef"}}
                    }
                }
            }
        });
        assert!(resolve_tool_service("wavs_exec_nonexistent_default", &services).is_none());
    }

    #[test]
    fn component_source_desc_variants() {
        assert_eq!(
            component_source_desc(&serde_json::json!({"component": {"source": {"oci": {"uri": "ghcr.io/test:v1"}}}})),
            "ghcr.io/test:v1"
        );
        assert_eq!(
            component_source_desc(&serde_json::json!({"component": {"source": {"digest": "abcdef123456789012"}}})),
            "component:abcdef123456"
        );
        assert_eq!(
            component_source_desc(&serde_json::json!({"component": {"source": {"download": {"uri": "https://example.com/comp.wasm"}}}})),
            "https://example.com/comp.wasm"
        );
        assert_eq!(
            component_source_desc(&serde_json::json!({"component": {"source": {}}})),
            "local"
        );
    }
}
