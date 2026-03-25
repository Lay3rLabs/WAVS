# Phase 3: MCP Execution Interface - Research

**Researched:** 2026-03-25
**Domain:** MCP tool execution, cryptographic signing, on-chain submission, dynamic tool discovery
**Confidence:** HIGH

## Summary

Phase 3 extends the existing `wavs-mcp` server (26 static management tools) with dynamic execution tools -- one per deployed service workflow. Each tool is named `wavs_exec_{service}_{workflow}`, uses Phase 2's WIT-to-schema library for auto-generated `inputSchema`, and supports three trust tiers (`result_only`, `signed_result`, `on_chain`) as an explicit parameter on each tool.

The existing codebase provides almost everything needed: the `WavsMcpServer` already has `list_tools()`/`call_tool()` dispatch, the WAVS node has `POST /dev/triggers` with `wait_for_completion` for synchronous execution, `WavsSigner` supports single-operator signing without the aggregator, and `chain_ops.rs` has on-chain transaction submission patterns. The `rmcp` crate (v0.1.5) supports `Peer::notify_tool_list_changed()` and `ToolsCapability { list_changed: Some(true) }` for dynamic tool discovery.

The primary engineering challenge is wiring these pieces together: (1) making `list_tools()` merge static management tools with dynamically-generated execution tools, (2) routing `call_tool()` for `wavs_exec_*` names through the trigger-execute-return pipeline, (3) adding the trust tier envelope/signing/submission layer, and (4) sending `notify_tool_list_changed()` when services are deployed or removed.

**Primary recommendation:** Extend `WavsMcpServer` in-place with a cached service list, dynamic tool generation using `generate_schema_cached()`, and a trust tier state machine (result_only -> signed_result -> on_chain) per `call_tool()` invocation.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Execution tools use `wavs_exec_` prefix (not `wavs_run_`). Resolves conflict between ROADMAP and REQUIREMENTS in favor of EXEC-07. Update ROADMAP to match.
- **D-02:** One tool per deployed service workflow: `wavs_exec_{service}_{workflow}`. V2 can get smarter about surfacing components that access different functions than the standalone run interface.
- **D-03:** Rich tool descriptions: include service name, workflow purpose (from WIT doc comments), supported trust tiers, and component source (OCI URI or local path). Helps agents pick the right tool.
- **D-04:** Tool list caching strategy is Claude's discretion -- optimize for performance. STATE.md flagged 5s TTL design; choose unified or separate cache based on what performs best.
- **D-05:** Tier 1 (`result_only`) returns the raw component output directly as MCP tool result content. No wrapper envelope -- keep it simple.
- **D-06:** Tier 2 (`signed_result`) wraps the result in a structured envelope with operator signature and signer public key. Cryptographic data encoding is Claude's discretion (hex is natural fit given alloy/EVM ecosystem).
- **D-07:** Tier 3 (`on_chain`) defaults to returning `{tx_hash, chain_id, block_explorer_url}`. Optional `wait_for_receipt: true` parameter returns full transaction receipt instead (status, gas used, block number). Note: waiting for receipt may consume more of the timeout budget.
- **D-08:** All three trust tiers are always accepted as input on every exec tool. If a tier is disabled for a service, return a structured error (don't silently downgrade).
- **D-09:** Tier 3 has a two-step flow: first call returns a gas cost estimate, agent must confirm with a follow-up call to actually submit the transaction. Protects agents managing funds from unexpected costs.
- **D-10:** Two-level gating is sufficient: global `--exec-enabled` CLI flag on MCP server + per-service `exec_enabled` in service.json. No additional allowlist/denylist needed.
- **D-11:** When Tier 3 is requested but not enabled for a service, return a structured error: "on_chain tier not enabled for this service". No fallback to lower tiers.
- **D-12:** No interactive confirmation for on-chain submission -- the agent explicitly chose the `on_chain` tier, which IS the confirmation. The two-level gating + cost estimate step (D-09) provide sufficient safety.
- **D-13:** All execution errors use structured error codes. Defined codes: `EXECUTION_TIMEOUT`, `TIER_NOT_ENABLED`, `SERVICE_NOT_FOUND`, `COMPONENT_FAILED`, `SIGNING_FAILED`, `SUBMISSION_FAILED`. Agents can programmatically handle each case.
- **D-14:** Timeout is configurable per-call via optional `timeout_ms` parameter, capped at 25s (EXEC-08). Default is 25s.
- **D-15:** If component executes successfully but signing (Tier 2) or submission (Tier 3) fails, the raw component result is included in the error response alongside the error code. Avoids wasting successful execution.

### Claude's Discretion
- Tool list caching implementation (unified vs separate, TTL value)
- Cryptographic data encoding format for Tier 2 signatures (hex recommended given EVM ecosystem)
- Whether to implement `wait_for_receipt` in v1 or defer to v2
- Internal execution pathway: whether Tier 1 bypasses aggregator or goes through existing pipeline
- How `notifications/tools/list_changed` is wired to service deploy/remove events
- Gas estimation implementation details for the Tier 3 two-step flow

### Deferred Ideas (OUT OF SCOPE)
- Per-function tool granularity (V2 -- smarter component function surfacing beyond one-tool-per-workflow)
- Per-service allowlist/denylist for exec tools (not needed with two-level gating)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EXEC-01 | Deployed service components appear as callable MCP tools via `tools/list` | Dynamic tool generation from service list + `generate_schema_cached()` for inputSchema |
| EXEC-02 | Agent can call a component via `tools/call` and receive execution result (Tier 1) | `POST /dev/triggers` with `wait_for_completion: true` returns after component execution |
| EXEC-03 | Agent can request signed result with operator signature (Tier 2) | `WavsSigner::sign()` with `PrivateKeySigner` from HD-derived key per service |
| EXEC-04 | Agent can request on-chain submission with tx hash (Tier 3), gated by service flag | `chain_ops.rs` patterns for EVM tx submission; two-level gating via `--exec-enabled` + service flag |
| EXEC-05 | Trust tier is an explicit `inputSchema` parameter on each tool | Single tool per workflow with `trust_tier` enum param in combined inputSchema |
| EXEC-06 | MCP `notifications/tools/list_changed` fires on service deploy/remove | `rmcp` 0.1.5 `Peer::notify_tool_list_changed()` + polling/webhook from service CRUD endpoints |
| EXEC-07 | Execution tools guarded by `--exec-enabled` flag, use `wavs_exec_` prefix | CLI arg addition to `Args` struct + conditional tool inclusion in `list_tools()` |
| EXEC-08 | Per-call timeout cap (25s) enforced at MCP layer | `tokio::time::timeout()` wrapping execution future, configurable via `timeout_ms` param |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rmcp | 0.1.5 | MCP protocol server, tool dispatch, notifications | Already in use; provides `ServerHandler`, `Peer::notify_tool_list_changed()`, `ToolsCapability { list_changed }` |
| wavs-types | workspace | Service, Workflow, Trigger, WasmResponse, signing types | All domain types already defined here |
| wit-schema | workspace | `generate_schema_cached()` for auto-generating inputSchema from component WIT | Phase 2 output; provides schema with cache by component digest |
| alloy-signer-local | workspace | `PrivateKeySigner` for single-operator signing (Tier 2) | Already used in `chain_ops.rs` for signing key derivation |
| tokio | workspace | Async runtime, `tokio::time::timeout` for per-call 25s cap | Already the project's async runtime |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| schemars | 0.8.x (via rmcp) | JSON Schema generation for static param structs | For the trust tier / timeout parameter schema overlay on WIT-derived inputSchema |
| const-hex | workspace | Hex encoding for signatures, public keys | Tier 2 signature data encoding (EVM ecosystem standard) |
| serde_json | workspace | JSON construction for dynamic tool schemas and responses | Building tool definitions, error responses |
| wasmtime | workspace | Engine for component type introspection | Required by `generate_schema_cached()` -- engine and component needed for schema generation |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Polling service list from WAVS node | Webhook/event channel from dispatcher | Webhook is cleaner but requires WAVS node changes; polling with TTL cache is simpler and sufficient for MCP use case |
| Single-operator signing via WavsSigner | Full aggregator pipeline | Aggregator is overkill for MCP-initiated ad-hoc execution; WavsSigner trait gives direct signing without consensus overhead |
| Dynamic serde_json schema construction | schemars derive on a composite struct | Derive cannot merge WIT-derived schema with trust tier params; manual JSON construction is required |

**Installation:**
No new dependencies needed. `wit-schema` is added as a workspace dependency to `wavs-mcp/Cargo.toml`:
```toml
wit-schema = { path = "../wit-schema" }
wasmtime = { workspace = true, features = ["component-model"] }
```

## Architecture Patterns

### Recommended Project Structure
```
packages/wavs-mcp/src/
  main.rs          # Add --exec-enabled CLI arg
  server.rs        # Extend WavsMcpServer with exec fields, list_tools() dynamic merge, call_tool() exec dispatch
  client.rs        # Add execute_trigger_sync() method (trigger + wait_for_completion)
  chain_ops.rs     # Existing; reuse for Tier 3 tx submission
  exec.rs          # NEW: Execution tool logic -- trust tier state machine, signing, schema merging
  scaffold.rs      # Existing
```

### Pattern 1: Dynamic + Static Tool Merge in list_tools()
**What:** `list_tools()` returns static management tools (existing 26) concatenated with dynamically generated execution tools from the cached service list.
**When to use:** Every `tools/list` call.
**Example:**
```rust
// In list_tools():
let mut tools = self.static_tools(); // existing 26 tools

if self.exec_enabled {
    let exec_tools = self.build_exec_tools().await?;
    tools.extend(exec_tools);
}

Ok(ListToolsResult { tools, next_cursor: None })
```

### Pattern 2: Trust Tier State Machine in call_tool()
**What:** When `call_tool()` receives a `wavs_exec_*` name, it extracts `trust_tier` from args, executes the component, then applies the appropriate post-processing (return raw, sign, or submit on-chain).
**When to use:** Every exec tool invocation.
**Example:**
```rust
// In call_tool() match:
name if name.starts_with("wavs_exec_") => {
    if !self.exec_enabled {
        return Err(ErrorData { code: ErrorCode::INVALID_REQUEST, .. });
    }
    self.handle_exec_tool(name, args).await
}
```

### Pattern 3: Cached Service List with TTL
**What:** Service list fetched from WAVS node, cached with TTL. Cache invalidated on deploy/delete calls through this MCP server. Separate from Phase 2's schema cache (schema cache is by component digest, service cache is the full service list).
**When to use:** Every `list_tools()` and `call_tool()` for service lookup.
**Recommendation:** 5-second TTL with immediate invalidation on local deploy/delete. Use `tokio::sync::RwLock<Option<(Instant, Vec<ServiceInfo>)>>` for thread-safe cached reads.

### Pattern 4: Peer Storage for Notifications
**What:** Override `set_peer()` and `get_peer()` on `ServerHandler` to store the `Peer<RoleServer>` in the server struct, enabling `notify_tool_list_changed()` calls from deploy/delete tool handlers.
**When to use:** Service deploy and remove operations.
**Example:**
```rust
#[derive(Clone)]
pub struct WavsMcpServer {
    // ... existing fields ...
    peer: Arc<tokio::sync::RwLock<Option<Peer<RoleServer>>>>,
}

// In ServerHandler impl:
fn set_peer(&mut self, peer: Peer<RoleServer>) {
    // Store peer for later notification use
    let peer_store = self.peer.clone();
    tokio::spawn(async move {
        *peer_store.write().await = Some(peer);
    });
}

fn get_peer(&self) -> Option<Peer<RoleServer>> {
    self.peer.try_read().ok().and_then(|g| g.clone())
}
```

### Anti-Patterns to Avoid
- **Separate tools per trust tier:** Do NOT create `wavs_exec_foo_result_only`, `wavs_exec_foo_signed`, `wavs_exec_foo_onchain`. The trust tier is a parameter on a single tool per workflow (EXEC-05).
- **Blocking the MCP server loop:** Component execution can take up to 25s. Always run execution in a spawned task with `tokio::time::timeout()`, never block the main server handler.
- **Silently downgrading tiers:** If Tier 3 is requested but not enabled, return `TIER_NOT_ENABLED` error (D-11). Never silently fall back to a lower tier.
- **Rebuilding schema on every list_tools():** Use Phase 2's `SchemaCache` (keyed by component digest) to avoid re-parsing WASM binaries. Only regenerate on cache miss.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON Schema from WASM component | Custom WIT parser | `wit_schema::generate_schema_cached()` | Phase 2 already handles all WIT types, caching, doc comment enrichment |
| EVM signing | Custom ECDSA | `WavsSigner::sign()` + `PrivateKeySigner` | Handles EIP-191 prefixing, secp256k1, signature encoding |
| MCP protocol notifications | Custom JSON-RPC messages | `Peer::notify_tool_list_changed()` | rmcp handles framing, serialization, protocol compliance |
| Timeout enforcement | Manual timer tracking | `tokio::time::timeout(Duration, future)` | Correct cancellation semantics, no resource leaks |
| HD key derivation | Manual BIP32 | `utils::evm_client::signing::make_signer(credential, Some(hd_index))` | Already used in `chain_ops.rs`; handles full BIP44 path |

**Key insight:** The existing codebase has all the building blocks -- execution via trigger simulation, signing via `WavsSigner`, on-chain submission via `chain_ops`, schema generation via `wit-schema`. Phase 3 is a wiring and orchestration exercise, not a new-capability exercise.

## Common Pitfalls

### Pitfall 1: Schema Merge Complexity
**What goes wrong:** The WIT-derived `inputSchema` from `generate_schema_cached()` describes the component's function parameters. The MCP tool also needs `trust_tier`, `timeout_ms`, and potentially `wait_for_receipt` parameters. Merging these into a single JSON Schema object can produce invalid schemas if done carelessly.
**Why it happens:** JSON Schema's `properties`, `required`, and `$defs` must be merged correctly. Overlapping property names between WIT and meta-parameters would break things.
**How to avoid:** Use a clear namespace: WIT params go under a `input` wrapper property, trust tier params are top-level. Or use a flat merge with reserved prefixes for meta-parameters.
**Warning signs:** Schema validation failures in MCP clients, agents unable to call tools.

### Pitfall 2: Synchronous Execution Pipeline Mismatch
**What goes wrong:** The existing `POST /dev/triggers` with `wait_for_completion: true` waits for the submission pipeline to complete (including aggregation). For Tier 1 (result_only), we only need the component output, not the full pipeline.
**Why it happens:** `wait_for_completion` polls `submission_manager.metrics.get_request_count()`, which only increments after the full pipeline runs. For Tier 1, we need just the `WasmResponse`.
**How to avoid:** For Tier 1, consider adding a direct execution endpoint on the WAVS node HTTP API that returns the `WasmResponse` without going through aggregation/submission. Alternatively, use `POST /dev/triggers` with `wait_for_completion: true` and `submit: "none"` -- since the service's submit config is `none`, the pipeline short-circuits after execution.
**Warning signs:** Tier 1 calls taking longer than expected, or failing because no aggregator is configured.

### Pitfall 3: Tool Name Collision and Sanitization
**What goes wrong:** Service names and workflow IDs can contain characters invalid for MCP tool names. A service named "My Service!" with workflow "default" would produce `wavs_exec_My Service!_default`.
**Why it happens:** Service `name` is a free-form UTF-8 string. Workflow IDs are constrained (3-36 lowercase alphanumeric) but service names are not.
**How to avoid:** Use a sanitization function: lowercase, replace non-alphanumeric with `_`, truncate, deduplicate. Consider using the service ID (hex hash of the ServiceManager) instead of the human name for uniqueness, with the human name only in the description.
**Warning signs:** MCP clients rejecting tool names, duplicate tool names from different services.

### Pitfall 4: Notification Timing with Peer Lifecycle
**What goes wrong:** `notify_tool_list_changed()` is called but the peer connection is not yet established (set_peer hasn't been called) or has been dropped.
**Why it happens:** `set_peer()` is called after connection setup. If a service is deployed during startup before the MCP client connects, there's no peer to notify.
**How to avoid:** Guard notification calls with `if let Some(peer) = self.get_peer()`. Log but don't error when no peer is available -- the client will get the updated list on next `tools/list` call anyway.
**Warning signs:** Panics or errors during service deploy when no MCP client is connected.

### Pitfall 5: Signing Key Availability for Tier 2
**What goes wrong:** Tier 2 requires the WAVS node's signing mnemonic to derive the per-service HD key. The MCP server may not have the signing mnemonic configured.
**Why it happens:** `--signing-mnemonic` is optional and only required for operator registration. Tier 2 signing reuses the same key.
**How to avoid:** Check `signing_mnemonic` availability at Tier 2 request time (not at startup). Return `SIGNING_FAILED` with a clear message if not configured. The `require_signing_mnemonic()` pattern already exists.
**Warning signs:** Agents getting generic errors instead of clear "signing mnemonic not configured" messages.

### Pitfall 6: Component Loading for Schema Generation
**What goes wrong:** `generate_schema_cached()` requires the WASM component bytes and a `wasmtime::Engine` to introspect the component type. The MCP server currently doesn't have access to WASM binaries -- it communicates with the WAVS node via HTTP.
**Why it happens:** Schema generation is a local operation requiring the binary. The MCP server is a thin client.
**How to avoid:** Two options: (1) Add a WAVS node HTTP endpoint that returns pre-generated schemas for deployed services (preferred -- schema generation happens at deploy time on the node). (2) Have the MCP server download component bytes and generate schemas locally (requires wasmtime dependency, heavier). Option 1 is recommended because the node already has the component bytes loaded.
**Warning signs:** MCP server binary size bloating with wasmtime, slow startup, or schema generation errors.

## Code Examples

### Example 1: Extending CLI Args with --exec-enabled
```rust
// In main.rs Args struct:
/// Enable execution tools (wavs_exec_*). When disabled, only management tools are available.
/// This is a safety gate -- execution tools can invoke component logic and (for Tier 3)
/// submit on-chain transactions.
#[arg(long, env = "WAVS_EXEC_ENABLED", default_value = "false")]
exec_enabled: bool,
```

### Example 2: Dynamic Tool Generation
```rust
// In exec.rs:
pub fn build_exec_tool(
    service_name: &str,
    service_id: &str,
    workflow_id: &str,
    component_schema: &serde_json::Value,
    component_source_desc: &str,
) -> Tool {
    let tool_name = format!(
        "wavs_exec_{}_{}",
        sanitize_tool_name(service_name),
        workflow_id
    );

    // Get the inputSchema from the component's "run" export (or first export)
    let wit_input_schema = component_schema
        .get("exports")
        .and_then(|e| e.as_object())
        .and_then(|exports| exports.values().next())
        .and_then(|v| v.get("inputSchema"))
        .cloned()
        .unwrap_or(serde_json::json!({"type": "object", "properties": {}}));

    // Merge WIT-derived input params with trust tier meta-parameters
    let merged_schema = merge_exec_schema(wit_input_schema);

    let description = format!(
        "Execute {} workflow '{}'. Source: {}. \
         Supports trust tiers: result_only, signed_result, on_chain.",
        service_name, workflow_id, component_source_desc
    );

    Tool {
        name: tool_name.into(),
        description: description.into(),
        input_schema: Arc::new(merged_schema.as_object().cloned().unwrap_or_default()),
    }
}
```

### Example 3: Trust Tier Dispatch
```rust
// In exec.rs handle_exec_tool():
match trust_tier.as_str() {
    "result_only" => {
        let result = self.execute_component(service, workflow, input, timeout).await?;
        ok(result.payload_as_string())
    }
    "signed_result" => {
        let result = self.execute_component(service, workflow, input, timeout).await?;
        let credential = self.require_signing_mnemonic()?;
        let signer = make_signer(&credential, Some(hd_index))?;
        let signature = envelope.sign(&signer, SignatureKind::evm_default()).await
            .map_err(|e| exec_error("SIGNING_FAILED", &e.to_string(), Some(&result)))?;
        ok(serde_json::to_string_pretty(&SignedResult {
            result: const_hex::encode(&result.payload),
            signature: const_hex::encode(&signature.data),
            signer_address: format!("{}", signer.address()),
            algorithm: "secp256k1",
            prefix: "eip191",
        })?)
    }
    "on_chain" => {
        // Two-step: estimate first, then submit on confirmation
        // ... gas estimation and submission logic ...
    }
    _ => exec_error("INVALID_PARAMS", "trust_tier must be result_only, signed_result, or on_chain", None),
}
```

### Example 4: Structured Error Response
```rust
fn exec_error(
    code: &str,
    message: &str,
    partial_result: Option<&WasmResponse>,
) -> Result<CallToolResult, McpError> {
    let mut error = serde_json::json!({
        "error_code": code,
        "message": message,
    });

    // D-15: Include raw result if component execution succeeded
    if let Some(result) = partial_result {
        error["partial_result"] = serde_json::json!({
            "payload": const_hex::encode(&result.payload),
        });
    }

    Ok(CallToolResult::error(vec![Content::text(
        serde_json::to_string_pretty(&error).unwrap_or_else(|_| error.to_string()),
    )]))
}
```

### Example 5: Service List Cache with TTL
```rust
use std::time::{Duration, Instant};

struct ServiceCache {
    services: Vec<ServiceInfo>,
    fetched_at: Instant,
    ttl: Duration,
}

impl ServiceCache {
    fn is_stale(&self) -> bool {
        self.fetched_at.elapsed() > self.ttl
    }
}

// In WavsMcpServer:
async fn get_services_cached(&self) -> Result<Vec<ServiceInfo>, McpError> {
    {
        let cache = self.service_cache.read().await;
        if let Some(ref cached) = *cache {
            if !cached.is_stale() {
                return Ok(cached.services.clone());
            }
        }
    }
    // Cache miss or stale -- refresh
    let services = self.client.list_services().await
        .map_err(|e| /* ... */)?;
    let parsed = parse_services(services)?;
    let mut cache = self.service_cache.write().await;
    *cache = Some(ServiceCache {
        services: parsed.clone(),
        fetched_at: Instant::now(),
        ttl: Duration::from_secs(5),
    });
    Ok(parsed)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Static MCP tools only | Dynamic tools from service registry | Phase 3 | Agents see execution tools without server restart |
| Fire-and-forget triggers | `wait_for_completion: true` on `/dev/triggers` | Already exists | Enables synchronous request-response for MCP tool calls |
| Full aggregator for signing | `WavsSigner` trait for single-operator ad-hoc signing | Already exists | Tier 2 signing without consensus overhead |

**Deprecated/outdated:**
- rmcp 0.1.5 is the version in Cargo.lock. The latest rmcp is 1.2.0+ on crates.io but upgrading is out of scope. The 0.1.5 API is sufficient for all Phase 3 needs.

## Architecture Decisions (Claude's Discretion Recommendations)

### Tool List Caching: Unified Cache, 5s TTL
Use a single service list cache in `WavsMcpServer` (not separate caches for list_tools vs call_tool). The same cached service list serves both `list_tools()` (to generate tool definitions) and `call_tool()` (to look up the service for execution). 5-second TTL balances freshness with performance. Immediate invalidation when the MCP server itself performs deploy/delete operations.

### Cryptographic Encoding: Hex (0x-prefixed)
Use `0x`-prefixed hex for all cryptographic data in Tier 2 responses (signature, signer address). This matches the EVM ecosystem convention used throughout the codebase (`const_hex`, alloy primitives). Agents working with WAVS are already in an EVM context.

### wait_for_receipt: Defer to v2
The `wait_for_receipt` option on Tier 3 adds complexity (polling for receipt within the 25s timeout budget) and is not in the core requirements. Implement the basic Tier 3 flow (submit tx, return tx_hash + chain_id) first. `wait_for_receipt` can be added in v2 alongside other advanced features.

### Execution Pathway: POST /dev/triggers with wait_for_completion
For Tier 1, use `POST /dev/triggers` with `wait_for_completion: true` and rely on the service having `submit: "none"`. This reuses the existing pipeline without WAVS node changes. The MCP server constructs a `SimulatedTriggerRequest` from the tool call parameters.

For getting the actual result back: the current `POST /dev/triggers` endpoint returns `200 OK` with no body -- it only waits for the submission count to increment. To get the component output, the MCP server needs an additional step. Two approaches:

**Recommended:** Add a new WAVS node endpoint (`POST /dev/execute`) that synchronously executes the component and returns the `WasmResponse` in the HTTP response body. This is the cleanest path and avoids the log-scraping or state-channel workarounds.

**Fallback:** If node changes are out of scope, use the existing `POST /dev/triggers` with `wait_for_completion: true` and then immediately query component logs (`GET /dev/logs`) to extract the result. This is fragile but functional.

### Notification Wiring: Peer-based from Deploy/Delete Handlers
Store the `Peer<RoleServer>` via `set_peer()`/`get_peer()` overrides. In `tool_deploy_service()`, `tool_deploy_dev_service()`, and `tool_delete_service()`, after a successful operation, call `peer.notify_tool_list_changed().await`. Also invalidate the service cache. If no peer is available (no client connected), skip the notification silently.

### Gas Estimation for Tier 3 Two-Step
For the two-step Tier 3 flow (D-09): when `trust_tier: "on_chain"` is first called, execute the component to get the result, then estimate gas cost using `provider.estimate_gas()`, and return the estimate. The agent then calls again with `confirm: true` to actually submit. Store the pending result in a short-lived cache (keyed by a nonce) so the confirmation step doesn't re-execute the component. TTL of 60 seconds for pending confirmations.

## Open Questions

1. **WAVS Node Execution Endpoint**
   - What we know: `POST /dev/triggers` with `wait_for_completion` waits for the pipeline to complete but returns no body. The component result is in `WasmResponse.payload` inside the engine.
   - What's unclear: Whether a new `/dev/execute` endpoint should be added to the WAVS node, or whether the MCP server should extract results from logs or an alternative channel.
   - Recommendation: Add a `POST /dev/execute` endpoint that returns `WasmResponse` as JSON. This is a small, focused change to the WAVS node and gives the MCP server clean access to execution results.

2. **Service JSON `exec_enabled` Field**
   - What we know: D-10 specifies a per-service `exec_enabled` flag in service.json for Tier 3 gating.
   - What's unclear: The current `Service` struct in `packages/types/src/service.rs` does not have an `exec_enabled` field. This needs to be added.
   - Recommendation: Add `exec_enabled: Option<bool>` to the `Service` struct (defaults to `None`, treated as `false` for backward compatibility). Only affects Tier 3 gating.

3. **Schema for Services with Multiple Exports**
   - What we know: Some components (aggregator world) have multiple exports (process-input, handle-timer-callback, handle-submit-callback). D-02 says "one tool per workflow."
   - What's unclear: Which export's inputSchema to use for the tool's inputSchema. Most operator components have a single "run" export.
   - Recommendation: Use the first export (typically "run") for the tool's inputSchema. Include all export names in the tool description so agents know what functions are available.

## Sources

### Primary (HIGH confidence)
- `packages/wavs-mcp/src/server.rs` -- Full MCP server implementation with 26 tools, `list_tools()`, `call_tool()`, `ServerHandler` impl
- `packages/wavs-mcp/src/client.rs` -- `WavsClient` HTTP client with `simulate_trigger()`, `list_services()`, `get_service_signer()`
- `packages/wavs-mcp/src/main.rs` -- CLI args (`Args` struct), credential loading, server startup
- `packages/wavs-mcp/src/chain_ops.rs` -- On-chain tx submission patterns (deploy, register, set URI)
- `packages/types/src/signing.rs` -- `WavsSignable`, `WavsSignature`, `SignatureKind`, `EventId`
- `packages/types/src/signing/signer.rs` -- `WavsSigner` trait with `sign()` method, `PrivateKeySigner` integration
- `packages/types/src/service.rs` -- `Service`, `Workflow`, `Trigger`, `Component`, `Submit`, `ServiceManager`, `WasmResponse`
- `packages/types/src/submission.rs` -- `Submission` struct (trigger_action, operator_response, envelope, signature)
- `packages/wit-schema/src/lib.rs` -- `generate_schema()`, `generate_schema_cached()`, `SchemaCache`, `SchemaOptions`
- `packages/wavs/src/http/handlers/debug.rs` -- `POST /dev/triggers` handler with `wait_for_completion` polling
- `packages/wavs/src/subsystems/engine.rs` -- `EngineCommand::ExecuteOperator`, `SubmissionRequest`, `run_trigger()` -> `Vec<WasmResponse>`
- `packages/wavs/src/subsystems/engine/wasm_engine.rs` -- `execute_operator_component()` returns `Vec<WasmResponse>`
- `packages/wavs/src/dispatcher.rs` -- Pipeline architecture: Trigger -> Engine -> Aggregator -> Submission

### Secondary (MEDIUM confidence)
- [rmcp 0.1.5 docs.rs](https://docs.rs/rmcp/0.1.5/rmcp/) -- `ToolsCapability { list_changed }`, `Peer::notify_tool_list_changed()`, `ServerHandler::set_peer()`/`get_peer()`
- [rmcp GitHub](https://github.com/4t145/rmcp) -- MCP Rust SDK architecture and notification patterns

### Tertiary (LOW confidence)
- None -- all findings verified against codebase and official rmcp documentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use in the project
- Architecture: HIGH -- patterns verified against existing codebase implementation
- Pitfalls: HIGH -- identified from real code paths and API contracts in the codebase
- Execution pathway: MEDIUM -- `POST /dev/execute` endpoint needs to be confirmed as the approach for synchronous result retrieval

**Research date:** 2026-03-25
**Valid until:** 2026-04-25 (stable domain, established codebase)
