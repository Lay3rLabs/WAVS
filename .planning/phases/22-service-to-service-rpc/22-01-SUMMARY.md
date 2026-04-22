---
phase: 22-service-to-service-rpc
plan: "01"
subsystem: engine
tags: [rpc, async, wasmtime, permissions, cycle-detection]
dependency_graph:
  requires: []
  provides: [RpcCaller trait, async call_service host function, RPC error variants, InstanceDepsBuilder RPC fields]
  affects: [packages/engine, packages/wavs, packages/cli]
tech_stack:
  added: [wasmtime async feature, wasmtime-fiber]
  patterns: [trait-object injection for circular dep avoidance, fiber-based async host function]
key_files:
  created:
    - packages/engine/src/rpc.rs
  modified:
    - Cargo.toml
    - packages/engine/src/bindings/operator/world.rs
    - packages/engine/src/bindings/operator/host.rs
    - packages/engine/src/worlds/operator/component.rs
    - packages/engine/src/worlds/instance.rs
    - packages/engine/src/lib.rs
    - packages/engine/src/utils/error.rs
    - packages/wavs/src/subsystems/engine/wasm_engine.rs
    - packages/wavs/benches/common/src/engine_setup.rs
    - packages/cli/src/command/exec_aggregator.rs
    - packages/cli/src/command/exec_component.rs
    - packages/engine/tests/helpers/exec.rs
    - packages/engine/tests/helpers/aggregator_exec.rs
decisions:
  - "Used 'host.call-service' as bindgen imports key (not 'call-service') because call-service is defined inside an inline 'host' interface, making WorldKey::Name('host') produce 'host.call-service' in lookup"
  - "RpcCaller trait object injection avoids circular wavs-engine -> wavs dependency"
  - "call_stack: Vec<String> threaded through InstanceDepsBuilder so callee gets extended stack"
metrics:
  duration_minutes: 35
  completed_date: "2026-04-22"
  tasks_completed: 2
  files_created: 1
  files_modified: 12
---

# Phase 22 Plan 01: Engine RPC Infrastructure Summary

**One-liner:** Fiber-based async `call_service` host function with `AllowedServiceCalls` permission enforcement, cycle detection, and depth limiting via injected `Arc<dyn RpcCaller>` trait object.

## What Was Built

The engine-side foundation for service-to-service RPC. A WASM component calling `call_service(target_id, payload)` now gets:

1. **Async execution** — `call_service` is registered via `func_wrap_async` (enabled by wasmtime `"async"` feature + `imports: { "host.call-service": async }` in bindgen). All other host functions remain sync.

2. **Permission check (RPC-02)** — `AllowedServiceCalls` from the caller's workflow permissions is checked before dispatch. Default is `None` (deny-all).

3. **Cycle detection (RPC-04)** — `call_stack.contains(&callee_id)` rejects A→B→A patterns.

4. **Depth limit (RPC-04)** — `call_stack.len() >= RPC_MAX_DEPTH` (5) rejects unbounded nesting.

5. **Trait injection** — `rpc_caller: Option<Arc<dyn RpcCaller>>` on `OperatorHostComponent` lets the `wavs` crate (Plan 02) inject the concrete executor without a circular dependency.

## Tasks Completed

| Task | Description | Commit |
|------|-------------|--------|
| 1 | wasmtime async feature + bindgen async import + RpcCaller trait + error variants | 8fb479093 |
| 2 | OperatorHostComponent RPC fields + async call_service + InstanceDepsBuilder wiring | c3fea8c11 |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Incorrect bindgen imports key for call-service**

- **Found during:** Task 1 (first cargo check)
- **Issue:** Plan specified `imports: { "call-service": async }` but this produced "unused imports rules" error. The function lives inside an inline interface named `host` in the WIT world, making the bindgen lookup key `"host.call-service"` (WorldKey::Name("host") + function "call-service" concatenated as "host.call-service").
- **Fix:** Changed to `imports: { "host.call-service": async }` in both bindgen blocks.
- **Files modified:** `packages/engine/src/bindings/operator/world.rs`
- **Commit:** 8fb479093

This is exactly assumption A1 from RESEARCH.md: "If `imports: { 'call-service': async }` does not compile, fall back to alternative." The correct fix was using the fully-qualified interface path, not `default: async`.

## Verification Results

```
cargo check -p wavs-engine  -> Finished dev profile (0 errors)
cargo check -p wavs         -> Finished dev profile (0 errors)
```

Acceptance criteria verified:
- `"async"` in Cargo.toml wasmtime features
- `"host.call-service": async` in both bindgen blocks
- `pub trait RpcCaller` defined in `packages/engine/src/rpc.rs`
- `pub mod rpc` exported from `packages/engine/src/lib.rs`
- `RpcPermissionDenied`, `RpcCycleDetected`, `RpcDepthExceeded` in `EngineError`
- `call_stack` and `rpc_caller` fields on `OperatorHostComponent`
- `async fn call_service` with `RPC_MAX_DEPTH`, `call_stack.contains`, `AllowedServiceCalls` check
- All 7 `InstanceDepsBuilder` construction sites updated with `rpc_caller: None, call_stack: vec![]`

## Known Stubs

The `rpc_caller` field defaults to `None` everywhere — this is intentional. Plan 02 wires the concrete `RpcCaller` implementation in `wasm_engine.rs`. Until then, any service calling `call_service` with `AllowedServiceCalls::All` or `Only(...)` will receive "call-service not available: no RPC caller configured". The permission check runs first, so services with `AllowedServiceCalls::None` (default) receive the permission error.

## Threat Surface

All STRIDE mitigations from the plan's threat register are implemented:
- T-22-01 (caller spoofing): caller ID read from `self.service.id()` — unforgeable by WASM
- T-22-02 (depth DoS): `RPC_MAX_DEPTH = 5` enforced before dispatch
- T-22-03 (cycle DoS): `call_stack.contains` check before dispatch
- T-22-04 (unauthorized calls): `AllowedServiceCalls` checked; default is `None` (deny-all)
- T-22-05 (call_chain disclosure): accepted — service IDs are operational metadata

## Self-Check: PASSED

Files created/modified exist and commits verified:
- `packages/engine/src/rpc.rs` — exists
- `packages/engine/src/bindings/operator/host.rs` — contains `async fn call_service`
- Commit 8fb479093 — verified in git log
- Commit c3fea8c11 — verified in git log
