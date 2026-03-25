---
phase: 03-mcp-execution-interface
plan: 01
subsystem: api
tags: [mcp, execution, wasm, trust-tiers, wavs-mcp, axum, schema-merging]

# Dependency graph
requires:
  - phase: 02-wit-to-schema-tooling
    provides: wit-schema library and SchemaCache for auto-generating inputSchema from component WIT
provides:
  - exec.rs module with TrustTier enum, error codes, exec_error() helper, sanitize_tool_name(), merge_exec_schema(), ServiceCache, ExecContext, PendingConfirmations
  - --exec-enabled / WAVS_EXEC_ENABLED CLI flag for wavs-mcp binary
  - POST /dev/execute WAVS node endpoint returning Vec<WasmResponse> JSON
  - WavsClient.execute_component() method for synchronous component execution
affects: [03-02, 03-03, mcp-execution-tools, mcp-trust-tiers]

# Tech tracking
tech-stack:
  added: [wit-schema (workspace dep in wavs-mcp), wasmtime (workspace dep in wavs-mcp)]
  patterns: [schema merging with input wrapper to avoid property collisions, ServiceCache with RwLock and TTL, ExecContext struct for extensible function signatures, structured MCP error codes with optional partial_result]

key-files:
  created:
    - packages/wavs-mcp/src/exec.rs
  modified:
    - packages/wavs-mcp/Cargo.toml
    - packages/wavs-mcp/src/main.rs
    - packages/wavs-mcp/src/server.rs
    - packages/wavs/src/http/handlers/debug.rs
    - packages/wavs/src/http/server.rs
    - packages/wavs-mcp/src/client.rs

key-decisions:
  - "Schema merging uses 'input' wrapper property to namespace WIT params away from meta-params (trust_tier, timeout_ms, confirm)"
  - "ServiceCache uses tokio::sync::RwLock with configurable TTL for thread-safe cached reads"
  - "ExecContext is a struct (not individual params) so Plan 03 can add fields without breaking handle_exec_tool signature"
  - "PendingConfirmations uses nonce-keyed HashMap with 60s auto-expiry for Tier 3 two-step flow"
  - "POST /dev/execute bypasses trigger/aggregator/submission pipeline -- calls engine.execute_operator_component() directly"

patterns-established:
  - "exec_error() for structured MCP error results with error_code, message, and optional partial_result"
  - "sanitize_tool_name() for safe MCP tool name generation from free-form service names"
  - "merge_exec_schema() for combining WIT inputSchema with execution meta-parameters"
  - "ServiceCache pattern for 5s TTL cached service list shared across list_tools and call_tool"

requirements-completed: [EXEC-05, EXEC-07, EXEC-08]

# Metrics
duration: 8min
completed: 2026-03-25
---

# Phase 3 Plan 01: Execution Foundation Summary

**Execution types, error codes, schema merging, service cache, ExecContext, --exec-enabled flag, and POST /dev/execute endpoint for synchronous component result retrieval**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-25T20:30:04Z
- **Completed:** 2026-03-25T20:38:27Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Created exec.rs module with all foundational types for MCP execution tools: TrustTier enum, 6 structured error code constants, exec_error() helper with partial result support, sanitize_tool_name(), merge_exec_schema(), ServiceCache with TTL, ExecContext struct, PendingConfirmations with auto-expiry
- Added --exec-enabled / WAVS_EXEC_ENABLED CLI flag to wavs-mcp binary for safety gating execution tools
- Added POST /dev/execute endpoint to WAVS node that synchronously runs a component via engine.execute_operator_component() and returns Vec<WasmResponse> as JSON -- solving the critical gap where POST /dev/triggers returns 200 with no body
- Added execute_component() method to WavsClient for the MCP server to call the new endpoint

## Task Commits

Each task was committed atomically:

1. **Task 1: Create exec.rs module with types, errors, schema merging, service cache, ExecContext, and tool name sanitization** - `2346b416` (feat)
2. **Task 2: Add POST /dev/execute endpoint to WAVS node that returns WasmResponse** - `23507a91` (feat)

## Files Created/Modified

- `packages/wavs-mcp/src/exec.rs` - New module: TrustTier, error codes, exec_error(), sanitize_tool_name(), merge_exec_schema(), ServiceCache, ExecContext, PendingConfirmations, MAX_TIMEOUT_MS, unit tests
- `packages/wavs-mcp/Cargo.toml` - Added wit-schema and wasmtime workspace dependencies
- `packages/wavs-mcp/src/main.rs` - Added `mod exec;`, --exec-enabled CLI arg, pass to WavsMcpServer::new()
- `packages/wavs-mcp/src/server.rs` - Added exec_enabled field to WavsMcpServer, updated constructor
- `packages/wavs/src/http/handlers/debug.rs` - Added ExecuteRequest struct, handle_dev_execute handler, dev_execute_inner function
- `packages/wavs/src/http/server.rs` - Registered /dev/execute route in protected dev endpoints
- `packages/wavs-mcp/src/client.rs` - Added execute_component() method to WavsClient

## Decisions Made

- Schema merging uses `"input"` wrapper property to namespace WIT params away from meta-params (trust_tier, timeout_ms, confirm) -- avoids property name collisions per Pitfall 1 from RESEARCH.md
- ExecContext is a struct with lifetime parameter so handle_exec_tool signature is stable across Plans 02 and 03
- PendingConfirmations uses nanosecond-based hex nonce and 60-second auto-expiry on take()
- POST /dev/execute calls engine.execute_operator_component() directly, returning the raw Vec<WasmResponse> -- this is the cleanest path for Tier 1 execution and avoids the log-scraping workaround

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- exec.rs provides all public types and functions that Plans 02 (dynamic tool generation) and 03 (trust tier dispatch) depend on
- POST /dev/execute endpoint is ready for Plan 02 to call via WavsClient.execute_component()
- --exec-enabled flag is wired but not yet gating anything (Plan 02 will wire it into list_tools/call_tool)

## Self-Check: PASSED

All created files verified on disk, all commit hashes found in git log.

---
*Phase: 03-mcp-execution-interface*
*Completed: 2026-03-25*
