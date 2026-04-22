---
phase: 22-service-to-service-rpc
verified: 2026-04-22T00:00:00Z
status: passed
score: 4/4 must-haves verified
---

# Phase 22: Service-to-Service RPC Verification Report

**Phase Goal:** An agent or component can synchronously call another deployed service via `call-service`, with both the caller's `AllowedServiceCalls` and the callee's `AllowedCallers` checked before dispatch, cycle detection preventing A->B->A deadlocks, and a depth cap stopping unbounded nesting
**Verified:** 2026-04-22
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A component calling `call_service(target_id, payload)` receives the target service's response bytes synchronously within the same trigger execution | ✓ VERIFIED | `RpcCallerImpl::call` in `rpc_caller.rs` calls `execute_operator_component_with_rpc` and returns `responses.into_iter().next().map(|r| r.payload)` synchronously within the trigger's async call chain |
| 2 | A component with `allowed_service_calls: None` that attempts `call_service()` receives a clear permission error and the call does not reach the target | ✓ VERIFIED | `host.rs` lines 117–132: `AllowedServiceCalls::None` branch returns `Err(format!("call-service denied: caller '{}' does not have permission to call '{}'", ...))` before any dispatch |
| 3 | A callee service with `allowed_callers: None` rejects an inbound `call-service` invocation with a clear error | ✓ VERIFIED | `rpc_caller.rs` lines 63–73: `AllowedCallers::None` / `None` branch returns `Err(format!("call-service denied: callee '{}' does not accept calls from '{}'", ...))` before `execute_operator_component_with_rpc` is called |
| 4 | A call chain A -> B -> A is detected and rejected with a cycle error before infinite recursion occurs | ✓ VERIFIED | `host.rs` lines 135–141: `self.call_stack.contains(&callee_id)` returns `Err(format!("call-service cycle detected: '{}' is already in the call chain {:?}", ...))` |

**Score:** 4/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `packages/engine/src/rpc.rs` | RpcCaller trait definition | ✓ VERIFIED | Exports `RpcCaller` trait, `RpcResult` type alias, `RpcFuture<'a>` type alias; 19 lines, non-stub |
| `packages/engine/src/bindings/operator/host.rs` | Async `call_service` implementation with permission + cycle checks | ✓ VERIFIED | `async fn call_service` at line 107; contains `AllowedServiceCalls`, `call_stack.contains`, `RPC_MAX_DEPTH`; fully implemented |
| `packages/engine/src/worlds/operator/component.rs` | `call_stack` and `rpc_caller` fields on `OperatorHostComponent` | ✓ VERIFIED | `pub call_stack: Vec<String>` line 31, `pub rpc_caller: Option<Arc<dyn RpcCaller>>` line 33 |
| `packages/engine/src/utils/error.rs` | RPC error variants | ✓ VERIFIED | `RpcPermissionDenied`, `RpcCycleDetected`, `RpcDepthExceeded` all present at lines 76–93 |
| `packages/wavs/src/subsystems/engine/rpc_caller.rs` | `RpcCallerImpl` struct implementing `RpcCaller` trait | ✓ VERIFIED | `impl<S: CAStorage + Send + Sync + 'static> RpcCaller for RpcCallerImpl<S>` at line 27; callee `AllowedCallers` check at lines 63–73 |
| `packages/engine/tests/rpc.rs` | Unit tests for RPC permission enforcement and cycle detection | ✓ VERIFIED | 6 tests: `rpc_permission_denied_error_format`, `rpc_cycle_detected_error_format`, `rpc_depth_exceeded_error_format`, `rpc_permission_denied_error_fields`, `rpc_cycle_detection_logic`, `rpc_depth_limit_logic` — all pass |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `packages/engine/src/bindings/operator/host.rs` | `packages/engine/src/rpc.rs` | `rpc_caller.call()` invocation in async `call_service` | ✓ WIRED | Line 161: `rpc_caller.call(callee_id, payload, new_call_stack).await` |
| `packages/engine/src/bindings/operator/world.rs` | wasmtime async feature | `"host.call-service": async` in both bindgen blocks | ✓ WIRED | Lines 16 and 42 in `world.rs`; wasmtime `"async"` feature confirmed at Cargo.toml line 178 |
| `packages/wavs/src/subsystems/engine/rpc_caller.rs` | `packages/wavs/src/subsystems/engine/wasm_engine.rs` | `RpcCallerImpl` calls `execute_operator_component_with_rpc` | ✓ WIRED | Line 104–111 in `rpc_caller.rs`: `self.engine.execute_operator_component_with_rpc(callee_service, trigger_action, Some(nested_rpc), call_stack).await` |
| `packages/wavs/src/subsystems/engine/wasm_engine.rs` | `packages/engine/src/worlds/instance.rs` | `InstanceDepsBuilder.rpc_caller` field injection | ✓ WIRED | `wasm_engine.rs` lines 206–207: `rpc_caller` and `call_stack` threaded into `InstanceDepsBuilder`; `instance.rs` lines 92–94 confirm struct fields present and used at line 291–293 |
| `packages/wavs/src/subsystems/engine.rs` | `rpc_caller.rs` | `EngineManager::run_trigger` constructs `RpcCallerImpl` and calls `execute_operator_component_with_rpc` | ✓ WIRED | Lines 215–227: `RpcCallerImpl { engine, services }` constructed and passed to `execute_operator_component_with_rpc` with `call_stack: vec![]` |

### Data-Flow Trace (Level 4)

Not applicable — this phase delivers infrastructure (trait, host function, permission checks), not a UI component or dashboard rendering dynamic data.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| 6 RPC unit tests pass | `cargo test -p wavs-engine --test rpc` | 6 passed; 0 failed | ✓ PASS |
| `wavs-engine` crate compiles | `cargo check -p wavs-engine` | Finished — no errors | ✓ PASS |
| `wavs` crate compiles | `cargo check -p wavs` | Finished — no errors | ✓ PASS |
| All wavs-engine tests pass | `cargo test -p wavs-engine` | 24 tests total across suites, all pass | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| RPC-01 | 22-01, 22-02 | `call-service` host function using `func_wrap_async` — re-entrant `Arc<WasmEngine>` calls `execute_operator_component` directly | ✓ SATISFIED | `"host.call-service": async` in both bindgen blocks; `execute_operator_component_with_rpc` re-entrantly executes callee via `Arc<WasmEngine>` |
| RPC-02 | 22-01 | `AllowedServiceCalls` permission enforcement — engine checks caller's permission before dispatching call | ✓ SATISFIED | `host.rs` lines 117–132: caller `AllowedServiceCalls` checked; `None` is default deny |
| RPC-03 | 22-01, 22-02 | `AllowedCallers` callee-side enforcement — engine checks callee accepts calls from the caller service | ✓ SATISFIED | `rpc_caller.rs` lines 63–73: callee `AllowedCallers` checked independently; `None` / absent default is deny |
| RPC-04 | 22-01 | Call depth limit (default 5) with cycle detection — prevents A→B→A deadlocks and unbounded nesting | ✓ SATISFIED | `host.rs` lines 112–148: `RPC_MAX_DEPTH = 5` constant; `call_stack.contains()` cycle check; `call_stack.len() >= RPC_MAX_DEPTH` depth check |

No orphaned requirements — all four Phase 22 requirements (RPC-01 through RPC-04) are accounted for across plans 22-01 and 22-02. E2E-04, E2E-05, E2E-06 are correctly mapped to Phase 23.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `packages/wavs/src/subsystems/engine/rpc_caller.rs` | 85 | Comment: `// Trigger::Manual is used as the placeholder trigger type.` | ℹ️ Info | The word "placeholder" is in a comment describing intent; `Trigger::Manual` is a real variant in the `Trigger` enum and the code path is fully implemented. Not a stub. |

No blockers. No warnings. The `Trigger::Manual` comment is informational only — `Manual` is a proper enum variant used intentionally for synthetic RPC triggers.

### Human Verification Required

None. All four success criteria are verifiable programmatically:
- Permission checks are code-inspectable
- Cycle detection logic is unit-tested
- Depth limit is constant-inspectable
- Both crates compile cleanly
- 24/24 engine tests pass

### Gaps Summary

No gaps. All phase success criteria are achieved:

1. **Synchronous call-service pipeline** — `RpcCallerImpl` resolves callee, enforces permissions, executes via `execute_operator_component_with_rpc`, returns first response payload. The entire path runs within a single trigger execution (no async fire-and-forget).

2. **Caller permission enforcement (RPC-02)** — `AllowedServiceCalls` is checked in `host.rs` before any dispatch; default `None` is deny-all.

3. **Callee permission enforcement (RPC-03)** — `AllowedCallers` is checked in `rpc_caller.rs` independently of caller-side checks; default `None` / absent is deny-all.

4. **Cycle detection and depth cap (RPC-04)** — `call_stack.contains()` blocks A→B→A; `RPC_MAX_DEPTH = 5` blocks unbounded nesting. Both checked in `host.rs` before delegating to `rpc_caller`.

---

_Verified: 2026-04-22_
_Verifier: Claude (gsd-verifier)_
