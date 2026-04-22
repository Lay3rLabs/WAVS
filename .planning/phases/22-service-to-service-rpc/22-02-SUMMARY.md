---
phase: 22-service-to-service-rpc
plan: 02
subsystem: engine
tags: [rpc, service-to-service, wasm, permissions, wavs-crate]
dependency_graph:
  requires: [22-01]
  provides: [concrete-rpc-caller, callee-permission-enforcement, rpc-unit-tests]
  affects: [packages/wavs/src/subsystems/engine, packages/engine/tests]
tech_stack:
  added: []
  patterns: [newtype-rpc-caller, inner-method-refactor, synthetic-trigger-action]
key_files:
  created:
    - packages/wavs/src/subsystems/engine/rpc_caller.rs
    - packages/engine/tests/rpc.rs
  modified:
    - packages/wavs/src/subsystems/engine/wasm_engine.rs
    - packages/wavs/src/subsystems/engine.rs
decisions:
  - "RpcCallerImpl newtype pattern: holds Arc<WasmEngine<S>> + Services, avoids adding Services to WasmEngine"
  - "execute_operator_component refactored to delegate to execute_operator_component_inner for DRY code"
  - "Trigger::Manual used as synthetic trigger type for RPC calls (callee sees TriggerData::Raw)"
  - "Nested calls get fresh RpcCallerImpl constructed per-call to support unbounded nesting up to depth limit"
metrics:
  duration: "~5 minutes"
  completed: "2026-04-22"
  tasks_completed: 2
  files_modified: 4
---

# Phase 22 Plan 02: Concrete RpcCallerImpl + RPC Unit Tests Summary

## One-liner

Concrete `RpcCallerImpl` in wavs crate wires callee service resolution, `AllowedCallers` enforcement, and `execute_operator_component_with_rpc` injection; 6 unit tests cover all RPC error paths.

## What Was Built

### Task 1: RpcCallerImpl + execute_operator_component_with_rpc + EngineManager injection

**`packages/wavs/src/subsystems/engine/rpc_caller.rs`** (new file):
- `RpcCallerImpl<S>` newtype wrapping `Arc<WasmEngine<S>>` + `Services`
- Implements `RpcCaller` trait from `wavs-engine` crate
- Parses callee `ServiceId` from hex string via `FromStr`
- Resolves callee service from `Services::get`
- Enforces callee-side `AllowedCallers` permission (RPC-03): `All`, `Only(ids)`, `None` (default, reject-all)
- Builds synthetic `TriggerAction` with `TriggerData::Raw(payload)` and `Trigger::Manual`
- Constructs nested `RpcCallerImpl` for each call so callee can make further RPC calls
- Dispatches to `execute_operator_component_with_rpc` with extended call stack

**`packages/wavs/src/subsystems/engine/wasm_engine.rs`** (modified):
- Added `execute_operator_component_with_rpc(service, trigger_action, rpc_caller, call_stack)`
- Refactored `execute_operator_component` to delegate to private `execute_operator_component_inner`
- Both methods share identical logic; only differ in which rpc_caller/call_stack they inject
- `InstanceDepsBuilder` now receives `rpc_caller` and `call_stack` from the caller

**`packages/wavs/src/subsystems/engine.rs`** (modified):
- Added `pub mod rpc_caller;` declaration
- Added `use rpc_caller::RpcCallerImpl;` import
- `run_trigger` now constructs `RpcCallerImpl { engine, services }` and calls `execute_operator_component_with_rpc` with it and `call_stack: vec![]`

### Task 2: RPC unit tests

**`packages/engine/tests/rpc.rs`** (new file, 6 tests):
- `rpc_permission_denied_error_format`: verifies `RpcPermissionDenied` Display includes caller_id, callee_id, reason
- `rpc_cycle_detected_error_format`: verifies `RpcCycleDetected` Display includes callee_id and call chain
- `rpc_depth_exceeded_error_format`: verifies `RpcDepthExceeded` Display includes limit and chain
- `rpc_permission_denied_error_fields`: verifies struct field access via pattern matching
- `rpc_cycle_detection_logic`: tests `Vec<String>.contains()` cycle detection logic
- `rpc_depth_limit_logic`: tests `len() >= RPC_MAX_DEPTH` depth limit check

## Verification Results

```
cargo check -p wavs       → Finished (no errors, no warnings)
cargo test -p wavs-engine → 24 tests total: 18 existing + 6 new, all pass
```

## Deviations from Plan

None — plan executed exactly as written.

The plan offered flexibility on the synthetic trigger type (Cron or Manual). `Trigger::Manual` was chosen over `Trigger::Cron` because it more accurately describes the intent (manually constructed RPC dispatch) and is already in the enum.

## Known Stubs

None. The RPC path is fully wired end-to-end:
- Component calls `call_service` → host.rs performs caller-side checks → `RpcCallerImpl::call` performs callee-side checks → `execute_operator_component_with_rpc` runs the callee WASM component → response returns to caller.

## Threat Flags

None. All STRIDE threats from the plan's threat model are addressed:
- T-22-06 (callee consent): `AllowedCallers` check in `RpcCallerImpl::call` before dispatch
- T-22-08 (recursive DoS): Host depth limit (5) + cycle detection checked before `rpc_caller.call()` is invoked
- T-22-09 (ServiceId spoofing): `ServiceId::parse()` validates hex format; `Services::get` returns error for unknown IDs

## Self-Check: PASSED

- `/workspace/WAVS/packages/wavs/src/subsystems/engine/rpc_caller.rs` — FOUND
- `/workspace/WAVS/packages/engine/tests/rpc.rs` — FOUND
- Commit `5d67f602e` — FOUND (feat(22-02): RpcCallerImpl...)
- Commit `c0efef156` — FOUND (test(22-02): RPC unit tests...)
- `cargo check -p wavs` — PASSED
- `cargo test -p wavs-engine` — 6/6 new tests PASSED, 18/18 existing tests PASSED
