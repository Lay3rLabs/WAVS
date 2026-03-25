---
phase: 03-mcp-execution-interface
plan: 02
subsystem: api
tags: [mcp, execution, dynamic-tools, trust-tiers, wavs-mcp, service-cache, peer-notifications]

# Dependency graph
requires:
  - phase: 03-mcp-execution-interface
    plan: 01
    provides: exec.rs module with TrustTier, error codes, ServiceCache, ExecContext, sanitize_tool_name(), merge_exec_schema(), WavsClient.execute_component()
provides:
  - build_exec_tools() generating Tool definitions from deployed service workflows
  - handle_exec_tool() dispatching Tier 1 result_only execution via /dev/execute with timeout enforcement
  - Dynamic list_tools() merging static management tools with exec tools when --exec-enabled
  - call_tool() routing wavs_exec_* names through ExecContext to handle_exec_tool()
  - Peer-based list_changed notifications on service deploy/delete
  - Service cache integration with 5s TTL and immediate invalidation
affects: [03-03, mcp-trust-tiers, mcp-signed-result, mcp-on-chain]

# Tech tracking
tech-stack:
  added: []
  patterns: [dynamic MCP tool generation from service registry, Peer<RoleServer> storage via Arc<RwLock> for async notification dispatch, service cache integration for both list_tools and call_tool]

key-files:
  created: []
  modified:
    - packages/wavs-mcp/src/exec.rs
    - packages/wavs-mcp/src/server.rs

key-decisions:
  - "Permissive input schema (any object) for exec tools since MCP server lacks component bytes for WIT parsing"
  - "Peer<RoleServer> stored in Arc<RwLock> with tokio::spawn in set_peer to handle sync/async boundary"
  - "ExecContext constructed with None for signing/chain/pending fields -- Plan 03 will populate"
  - "notify_tools_changed() fires on both deploy and delete success paths (3 call sites)"

patterns-established:
  - "resolve_tool_service() maps wavs_exec_* tool names back to service_id + workflow_id via sanitized name matching"
  - "component_source_desc() extracts human-readable source description from workflow JSON (OCI URI / digest / download / local)"
  - "Service deploy/delete -> cache invalidate -> peer notify pattern for tool list freshness"

requirements-completed: [EXEC-01, EXEC-02, EXEC-05, EXEC-06, EXEC-07, EXEC-08]

# Metrics
duration: 6min
completed: 2026-03-25
---

# Phase 3 Plan 02: Execution Tool Pipeline Summary

**End-to-end MCP execution pipeline: dynamic tool discovery from deployed services via list_tools(), Tier 1 result_only dispatch via call_tool() with timeout enforcement, and peer-based list_changed notifications on service CRUD**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-25T20:44:26Z
- **Completed:** 2026-03-25T20:50:25Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added build_exec_tools() that generates MCP Tool definitions for every deployed service workflow, with tool names `wavs_exec_{sanitized_name}_{workflow_id}`, descriptive text including component source (OCI/digest/download/local), and permissive input schema wrapped via merge_exec_schema()
- Added handle_exec_tool() that dispatches Tier 1 (result_only) execution through WavsClient.execute_component() with tokio::time::timeout enforcement capped at 25s, payload extraction with hex/UTF-8 display, and structured error reporting for timeouts, component failures, and unknown services
- Wired dynamic exec tools into server.rs: list_tools() conditionally merges exec tools when --exec-enabled, call_tool() routes wavs_exec_* to handle_exec_tool() via ExecContext, set_peer/get_peer store Peer<RoleServer> for notifications, and deploy/delete/deploy_dev fire list_changed notifications

## Task Commits

Each task was committed atomically:

1. **Task 1: Add dynamic exec tool generation and Tier 1 execution to exec.rs** - `5eca2d83` (feat)
2. **Task 2: Wire exec tools into server.rs -- dynamic list_tools, call_tool dispatch, peer notifications, service cache** - `674b67ef` (feat)

## Files Created/Modified

- `packages/wavs-mcp/src/exec.rs` - Added build_exec_tools(), handle_exec_tool(), resolve_tool_service(), component_source_desc(), plus 6 new unit tests
- `packages/wavs-mcp/src/server.rs` - Added service_cache/peer/pending_confirmations fields, get_services_cached(), notify_tools_changed(), set_peer/get_peer overrides, exec tool merge in list_tools(), wavs_exec_* dispatch in call_tool(), ToolsCapability with list_changed

## Decisions Made

- Used permissive input schema (`additionalProperties: true`) for exec tools because the MCP server does not have access to component WASM bytes for full WIT interface parsing; the schema wraps inputs under `"input"` property alongside meta-params
- Stored Peer<RoleServer> in `Arc<tokio::sync::RwLock<Option<Peer>>>` because set_peer() is synchronous but peer storage must be async-safe; tokio::spawn bridges the gap
- ExecContext constructed with None for signing_mnemonic, mcp_chain_credential, and pending_confirmations in Plan 02; Plan 03 will populate these for Tier 2/3 support
- notify_tools_changed() fires on all 3 service mutation paths (deploy, delete, deploy_dev) after success, using try_read on peer to silently no-op when no client connected

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Tier 1 (result_only) execution pipeline is fully functional end-to-end
- Plan 03 can add Tier 2 (signed_result) and Tier 3 (on_chain) by filling in the placeholder match arms in handle_exec_tool() and populating the ExecContext fields
- pending_confirmations field is wired into WavsMcpServer but not yet used (reserved for Tier 3 two-step flow)

## Self-Check: PASSED

All created/modified files verified on disk, all commit hashes found in git log.

---
*Phase: 03-mcp-execution-interface*
*Completed: 2026-03-25*
