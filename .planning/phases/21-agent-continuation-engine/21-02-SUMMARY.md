---
phase: 21-agent-continuation-engine
plan: 02
subsystem: engine
tags: [agent, continuation, wasm, testing, legacy-compat, wit]
dependency_graph:
  requires:
    - "21-01 (execute.rs continuation loop, ContinuationLimit error, KV persistence)"
  provides:
    - "Integration tests for continuation engine error format, KV key format, legacy fallback"
    - "wavs-legacy-world WIT + bindings for backward-compatible execute_legacy()"
    - "Workspace-wide compile fixes for Phase 20 struct field changes"
  affects:
    - "packages/engine/tests/continuation.rs"
    - "packages/engine/src/bindings/operator/world.rs"
    - "packages/engine/src/worlds/operator/execute.rs"
    - "wit-definitions/operator/wit/operator.wit"
    - "packages/wavs/src/subsystems/engine/wasm_engine.rs"
    - "packages/cli/src/command/exec_component.rs"
    - "packages/cli/src/command/exec_aggregator.rs"
tech_stack:
  added: []
  patterns:
    - "Dual-world wasmtime bindgen: wavs-world (run+agent) vs wavs-legacy-world (run only)"
    - "Type reuse across bindgen worlds via with: {wavs:operator/input: super::..., wavs:operator/output: super::...}"
    - "Unit tests for error Display impl, KV key format string construction"
    - "Integration tests for legacy WASM execution through refactored execute() router"
key_files:
  created:
    - packages/engine/tests/continuation.rs
  modified:
    - wit-definitions/operator/wit/operator.wit
    - packages/engine/src/bindings/operator/world.rs
    - packages/engine/src/worlds/operator/execute.rs
    - packages/engine/tests/helpers/service.rs
    - packages/engine/tests/aggregator_basic.rs
    - packages/cli/src/command/exec_component.rs
    - packages/cli/src/command/exec_aggregator.rs
    - packages/wavs/src/subsystems/engine/wasm_engine.rs
    - packages/wavs/src/subsystems/engine.rs
    - packages/wavs/src/subsystems/trigger.rs
    - packages/wavs/src/subsystems/aggregator.rs
    - packages/wavs/src/subsystems/submission.rs
    - packages/wavs/src/http/handlers/debug.rs
    - packages/wavs/benches/common/src/engine_setup.rs
decisions:
  - "wavs-legacy-world added to WIT to instantiate pre-agent WASM binaries without the agent export"
  - "Legacy bindings reuse types from main wavs-world bindgen via with: mapping to avoid type duplication"
  - "ServiceId hash used in error tests (not string-from) since ServiceId is a hash type"
  - "correlation_id removed from all TriggerAction struct constructors (field removed in Phase 20)"
metrics:
  duration_minutes: 95
  completed_date: "2026-04-22"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 14
---

# Phase 21 Plan 02: Continuation Engine Tests and Caller Fixes Summary

## One-liner

Legacy-world WIT + bindings fix backward-compat execute_legacy(), workspace compile errors from Phase 20 field changes fixed across 12 files, 6-test continuation.rs suite proves error format, KV key pattern, and legacy fallback.

## What Was Built

### Task 1: Verify Callers Compile and Fix Pre-Existing Issues

The `execute()` function signature was **unchanged** from Plan 01 (same 4 params: deps, trigger, max_payload_size, max_salt_size). No signature-driven caller updates were needed.

However, running `cargo test -p wavs-engine` revealed two categories of pre-existing bugs:

**Bug 1: execute_legacy() used WavsWorld which requires the agent export**

The `wavs-world` WIT now has `export agent` in addition to `export run`. Pre-compiled WASM binaries (square.wasm, kv-store.wasm, etc.) were compiled before this change and only export `run`. When `execute_legacy()` called `WavsWorld::instantiate_async()`, it failed with:
```
Wasm instantiate: no exported instance named `wavs:operator/agent@2.7.0`
```

**Fix:** Added `wavs-legacy-world` to the WIT with only `export run` (no agent). Generated `WavsLegacyWorld` bindings in `world.rs` using `with:` to reuse the same Rust types from the main `wavs-world` bindgen. Updated `execute_legacy()` to call `WavsLegacyWorld::instantiate_async()` instead.

**Bug 2: Phase 20 struct field changes broke all callers**

Phase 20 added `max_continuation_steps`, `allowed_callers` to `Component`; added `allowed_service_calls` to `Permissions`; removed `exec_enabled` from `Service`; removed `correlation_id` from `TriggerAction`. These changes were not propagated to callers across the codebase.

Fixed across 12 files: `service.rs` (test helper), `aggregator_basic.rs`, `wasm_engine.rs`, `trigger.rs`, `aggregator.rs`, `submission.rs`, `engine.rs`, `debug.rs`, `exec_component.rs`, `exec_aggregator.rs`, `engine_setup.rs` (benches).

**Verification results:**
- `cargo check -p wavs-engine` ✓
- `cargo check -p wavs` ✓
- `cargo check -p wavs-cli` ✓ (2 unused-import warnings, no errors)
- `cargo test -p wavs-types` ✓ (2 doc-tests pass)
- `cargo test -p wavs-engine` ✓ (18 tests, 0 failures)

### Task 2: Add Continuation Integration Tests

Created `packages/engine/tests/continuation.rs` with 6 tests:

| Test | What it proves |
|------|----------------|
| `continuation_limit_error_format` | `ContinuationLimit` Display includes step count and workflow_id |
| `continuation_limit_error_fields` | Error variant fields are accessible and correct |
| `kv_key_format_correctness` | KV key `{ns}/wavs_agent_step/{svc}:{wfl}:step:{N}` matches engine code |
| `kv_key_format_step_zero` | Key format correct at step 0 (first continuation step) |
| `legacy_component_still_works` | square.wasm executes via `execute()→execute_legacy()` path, 7²=49 |
| `legacy_component_multiple_values` | Multiple inputs validate legacy routing: 3²=9, 10²=100, 0²=0 |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] execute_legacy() used WavsWorld requiring agent export — broke all legacy components**

- **Found during:** Task 1 test run (`cargo test -p wavs-engine --test basic`)
- **Issue:** Phase 20 added `export agent` to `wavs-world` WIT. Pre-compiled WASM binaries don't have this export. `WavsWorld::instantiate_async()` fails with `"no exported instance named wavs:operator/agent@2.7.0"` for all legacy components.
- **Fix:** Added `wavs-legacy-world` WIT (run-only, same imports, no agent export). Generated `WavsLegacyWorld` bindings reusing types from main world via `with:` directive. Updated `execute_legacy()` to use `WavsLegacyWorld::instantiate_async()`.
- **Files modified:** `operator.wit`, `world.rs`, `execute.rs`
- **Commit:** `4e010990b`

**2. [Rule 1 - Bug] Phase 20 struct field changes not propagated to 12 callers**

- **Found during:** Task 1 `cargo test -p wavs-engine` (compile errors)
- **Issue:** `TriggerAction::correlation_id` (removed), `Service::exec_enabled` (removed), `Component::allowed_callers`, `Component::max_continuation_steps`, `Permissions::allowed_service_calls` (all added). Multiple files still used old field names.
- **Fix:** Bulk removed `correlation_id` / `exec_enabled` field initializers; added `allowed_callers: None`, `max_continuation_steps: None`, `allowed_service_calls: Default::default()` where needed. Fixed `source.digest()` callers that treated it as `Option` (it's not).
- **Files modified:** 12 files across engine tests, wavs subsystems, CLI, benchmarks
- **Commit:** `4e010990b`

## Threat Model Compliance

| Threat ID | Mitigation | Status |
|-----------|-----------|--------|
| T-21-05 (test spoofing) | Tests use trusted source binaries from examples/build/ | ACCEPTED |

## Known Stubs

None — all tests use real logic. The legacy-world fix is a full implementation, not a stub.

## Threat Flags

None — no new network endpoints, auth paths, or file access patterns introduced.

## Self-Check: PASSED

- `packages/engine/tests/continuation.rs`: FOUND
- `wit-definitions/operator/wit/operator.wit` (wavs-legacy-world): FOUND
- `packages/engine/src/bindings/operator/world.rs` (legacy mod): FOUND
- Commit `4e010990b` (Task 1): FOUND
- Commit `8fc311fef` (Task 2): FOUND
- `cargo test -p wavs-engine --test continuation`: 6 passed, 0 failed
- `cargo test -p wavs-engine`: 18 passed total, 0 failed
