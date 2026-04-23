---
phase: 23-integration-validation
plan: 01
subsystem: examples/engine
tags: [export-macro, legacy-world, agent-world, wasm, continuation, kv, integration-test]
dependency_graph:
  requires:
    - "21-02 (wavs-legacy-world WIT, execute_agent continuation loop)"
    - "20-01 (export agent in wavs-world WIT)"
  provides:
    - "export_layer_trigger_world! uses legacy world (no GuestAgent required)"
    - "export_layer_agent_world! for full agent components"
    - "multi-step-agent example demonstrating 4-step continuation with KV checkpoints"
    - "continuation_e2e integration tests proving agent loop works end-to-end"
  affects:
    - "examples/components/_helpers/src/bindings/world.rs"
    - "examples/components/multi-step-agent/"
    - "packages/engine/tests/continuation_e2e.rs"
    - "packages/utils/src/test_utils/mock_engine.rs"
    - "Cargo.toml (workspace members)"
tech_stack:
  added: []
  patterns:
    - "Dual-world wit_bindgen on component side: legacy_world (run only) + main world (run+agent)"
    - "Type remapping via with: in legacy_world bindgen to share types across worlds"
    - "Blanket impl in export_layer_trigger_world! bridges world::Guest to legacy_world::Guest"
    - "KV-persisted step counter in agent_state bucket for multi-step continuation"
key_files:
  created:
    - examples/components/multi-step-agent/Cargo.toml
    - examples/components/multi-step-agent/service.json
    - examples/components/multi-step-agent/src/lib.rs
    - examples/build/components/multi_step_agent.wasm
    - packages/engine/tests/continuation_e2e.rs
  modified:
    - examples/components/_helpers/src/bindings/world.rs
    - packages/utils/src/test_utils/mock_engine.rs
    - Cargo.toml
decisions:
  - "Type remapping in legacy_world bindgen uses versioned keys (wavs:operator/input@2.7.0 etc) plus all transitive wavs:types dependencies to avoid type duplication"
  - "export_layer_trigger_world! provides a blanket impl from world::Guest to legacy_world::Guest inside the macro, avoiding any component source file changes"
  - "Multi-step-agent uses step counter in agent_state bucket (not wavs_agent_step which is engine-owned)"
  - "WASM built natively with cargo build --target wasm32-wasip2 (Docker builder unavailable)"
metrics:
  duration_minutes: 60
  completed_date: "2026-04-23"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 8
---

# Phase 23 Plan 01: Export Macro Fix + Multi-Step Agent Summary

## One-liner

Split `_helpers` export macros into legacy-world (run-only, no GuestAgent) and agent-world (run+agent), unblocking all example components, then built multi-step-agent demonstrating 4-step KV-checkpointed continuation with 2 passing integration tests.

## What Was Built

### Task 1: Fix _helpers Export Macros

The root cause: `wavs-world` WIT (added in Phase 20) exports both `run` AND `agent`, so `wit_bindgen::generate!` for `wavs-world` requires components to implement both `Guest::run` AND `GuestAgent::run_agent`. The `export_layer_trigger_world!` macro used the full world, forcing every legacy component to add a `GuestAgent` impl.

**Fix in `examples/components/_helpers/src/bindings/world.rs`:**

1. Added `pub mod legacy_world { wit_bindgen::generate!({ world: "wavs-legacy-world", ... }) }` with `with:` remappings for all type dependencies:
   - `wavs:operator/input@2.7.0` → `super::wavs::operator::input`
   - `wavs:operator/output@2.7.0` → `super::wavs::operator::output`
   - `wavs:types/service@2.7.0`, `events@2.7.0`, `core@2.7.0`, `chain@2.7.0` → corresponding super paths

2. Changed `export_layer_trigger_world!` to:
   - Generate a blanket `impl legacy_world::Guest for $Component` that delegates to `world::Guest::run`
   - Call `legacy_world::export!($Component with_types_in legacy_world)`
   - Using `$Component:ident` (not `:ty`) to allow use in impl blocks

3. Added `export_layer_agent_world!` that calls `world::export!($Component with_types_in world)` for full agent components.

**Key discovery**: The `with:` type remapping requires versioned interface paths (`wavs:operator/input@2.7.0`) and ALL transitive type dependencies must also be remapped. Without mapping `wavs:types/*`, the `TriggerAction` type inside `legacy_world::Guest` would use a different `Trigger` enum than the one `world::Guest` uses, causing the blanket impl to fail.

**Verification:**
- `cargo check -p square` ✓
- `cargo check -p kv-store` ✓
- `cargo check -p echo-data` ✓
- `cargo check -p permissions` ✓

### Task 2: Multi-Step Agent Component + Engine Integration Test

**Component (`examples/components/multi-step-agent/src/lib.rs`):**
- `impl Guest for Component` → stub returning `Err("use run-agent interface")`
- `impl GuestAgent for Component` → reads/writes `agent_state` KV bucket:
  - Step 0-2: writes `checkpoint:{N}` = `"completed step {N}"`, increments counter, returns `StepResult::Continue("step_{N+1}")`
  - Step 3: collects all 4 checkpoints into JSON array, returns `StepResult::Done([WasmResponse{payload: json_bytes}])`
- `export_layer_agent_world!(Component)` at bottom

**WASM Build:**
- Built natively via `cargo build --target wasm32-wasip2 -p multi-step-agent`
- Output at `examples/build/components/multi_step_agent.wasm`
- `wasm-tools component wit` confirms exports: `run` + `wavs:operator/agent@2.7.0`

**Integration Tests (`packages/engine/tests/continuation_e2e.rs`):**

| Test | What it proves |
|------|----------------|
| `multi_step_agent_runs_to_completion` | Agent completes in 4 steps, returns JSON summary with 4 checkpoint strings |
| `multi_step_agent_kv_checkpoints_exist` | After completion, `test-svc/agent_state/checkpoint:0..3` exist in WavsDb |

Both tests pass: `2/2 ok` in 6.51s.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `with:` remapping needs versioned keys and all transitive type dependencies**

- **Found during:** Task 1 implementation — `wit_bindgen::generate!` with `"wavs:operator/input"` (unversioned) failed with "unused remappings". With versioned keys, mismatched types on transitive `Trigger` and `TriggerData` types.
- **Issue:** The legacy_world uses `trigger-action` which depends on `wavs:types/service@2.7.0.{trigger}` and `wavs:types/events@2.7.0.{trigger-data}`. Without remapping these transitive types, the `legacy_world::TriggerAction.config.trigger` field has a different Rust type than `world::TriggerAction.config.trigger`.
- **Fix:** Added 4 additional `with:` entries for `wavs:types/service@2.7.0`, `wavs:types/events@2.7.0`, `wavs:types/core@2.7.0`, `wavs:types/chain@2.7.0`
- **Files modified:** `examples/components/_helpers/src/bindings/world.rs`
- **Commit:** `3e73d4c03`

**2. [Rule 1 - Bug] `$Component:ty` metavar cannot be used in `impl` blocks — must use `$Component:ident`**

- **Found during:** Task 1 macro implementation
- **Issue:** The blanket impl `impl legacy_world::Guest for $Component` requires `$Component` to be an identifier, not a type fragment. Using `:ty` causes "no rules expected `ty` metavariable" error.
- **Fix:** Changed `$Component:ty` to `$Component:ident` in both macros.
- **Files modified:** `examples/components/_helpers/src/bindings/world.rs`
- **Commit:** `3e73d4c03`

**3. [Rule 3 - Blocking] Worktree on wrong base — needed to rebase onto `wavs-for-agents`**

- **Found during:** Pre-flight check. Worktree was on `worktree-agent-add876b8` (based on upstream `main@e5e97f390`) while Phase 21-22 code is on `wavs-for-agents@d90598856`.
- **Fix:** Created `wavs-for-agents-23-01` branch from `d90598856` and switched worktree to it. Rebase attempt failed due to CI file conflicts; clean checkout used instead.
- **Impact:** Zero — all Phase 23 work committed cleanly on the correct base.

## Threat Model Compliance

| Threat ID | Mitigation | Status |
|-----------|-----------|--------|
| T-23-01 (KV tampering) | multi-step-agent writes to `agent_state` bucket, never to `wavs_agent_step` | VERIFIED in component source and tests |
| T-23-02 (DoS continuation) | Engine enforces `max_continuation_steps`; service.json sets max 5 | ACCEPTED (engine tested in Phase 21) |
| T-23-03 (export routing) | Legacy components use `export_layer_trigger_world!` → no agent export | VERIFIED: `has_agent_export()` returns false for square.wasm |

## Known Stubs

None — all tests use real WASM execution through the real engine.

## Threat Flags

None — no new network endpoints, auth paths, or file access patterns introduced.

## Self-Check: PASSED

- `examples/components/_helpers/src/bindings/world.rs` (legacy_world mod): FOUND
- `examples/components/multi-step-agent/src/lib.rs` (GuestAgent impl): FOUND
- `examples/build/components/multi_step_agent.wasm`: FOUND
- `packages/engine/tests/continuation_e2e.rs`: FOUND
- `packages/utils/src/test_utils/mock_engine.rs` (COMPONENT_MULTI_STEP_AGENT_BYTES): FOUND
- Task 1 commit `3e73d4c03`: FOUND
- Task 2 commit `fd69757d8`: FOUND
- `cargo test -p wavs-engine --test continuation_e2e`: 2 passed, 0 failed
- `cargo test -p wavs-engine`: all passed, 0 failed
