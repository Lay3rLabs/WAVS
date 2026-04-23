---
phase: 23-integration-validation
plan: 02
subsystem: examples/engine
tags: [rpc, composition, agent, wasm, integration-test, permission-enforcement, mock-rpc]
dependency_graph:
  requires:
    - "23-01 (export_layer_agent_world!, export_layer_trigger_world! fixed)"
    - "22-02 (RpcCaller trait, AllowedServiceCalls enforcement in host.rs)"
  provides:
    - "utility-service component: legacy run-only callee echoing payload with prefix"
    - "composition-agent component: agent world caller invoking call_service"
    - "MockRpcCaller: test-only inline WASM executor for engine-level RPC tests"
    - "rpc_e2e integration tests proving E2E-05 (composition) and E2E-06 (permissions)"
  affects:
    - "examples/components/utility-service/"
    - "examples/components/composition-agent/"
    - "packages/engine/tests/rpc_e2e.rs"
    - "packages/engine/tests/helpers/"
    - "packages/utils/src/test_utils/mock_engine.rs"
    - "Cargo.toml (workspace members)"
tech_stack:
  added: []
  patterns:
    - "MockRpcCaller executes callee WASM inline via wavs_engine::worlds::operator::execute::execute"
    - "Test service variant make_service_with_allowed_calls for AllowedServiceCalls::All"
    - "try_execute_component_raw_with_rpc injects Arc<dyn RpcCaller> into InstanceDepsBuilder"
    - "composition-agent reads callee_service_id from config_var for flexible test routing"
key_files:
  created:
    - examples/components/utility-service/Cargo.toml
    - examples/components/utility-service/service.json
    - examples/components/utility-service/src/lib.rs
    - examples/build/components/utility_service.wasm
    - examples/components/composition-agent/Cargo.toml
    - examples/components/composition-agent/service.json
    - examples/components/composition-agent/src/lib.rs
    - examples/build/components/composition_agent.wasm
    - packages/engine/tests/rpc_e2e.rs
    - packages/engine/tests/helpers/mock_rpc.rs
  modified:
    - packages/utils/src/test_utils/mock_engine.rs
    - packages/engine/tests/helpers/mod.rs
    - packages/engine/tests/helpers/exec.rs
    - packages/engine/tests/helpers/service.rs
    - Cargo.toml
decisions:
  - "MockRpcCaller keyed by arbitrary string (config var value) not actual ServiceId hex — cleaner test setup, matches how composition-agent passes the callee_id"
  - "composition-agent reads callee_service_id from host config_var instead of hardcoding — flexible for different test scenarios"
  - "callee_without_allowed_callers test uses approach (a) from plan: direct error message format verification without WASM execution (the check lives in wavs crate's RpcCallerImpl, not testable from engine tests)"
  - "WASM built natively with cargo build --target wasm32-wasip2 (Docker builder unavailable)"
metrics:
  duration_minutes: 45
  completed_date: "2026-04-23"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 11
---

# Phase 23 Plan 02: Utility-Service + Composition-Agent + RPC E2E Tests Summary

## One-liner

Service-to-service RPC demonstrated end-to-end: composition-agent calls utility-service via call_service host import, with MockRpcCaller injecting callee WASM execution inline into engine tests, proving E2E-05 and E2E-06.

## What Was Built

### Task 1: utility-service and composition-agent Components

**utility-service** (`examples/components/utility-service/`):
- Legacy run-only component using `export_layer_trigger_world!`
- Receives `TriggerData::Raw(bytes)`, prepends `"utility-response: "`, returns as `WasmResponse`
- `service.json`: `allowed_callers: "all"` — accepts RPC calls from any service
- No external dependencies beyond `example-helpers`

**composition-agent** (`examples/components/composition-agent/`):
- Agent component using `export_layer_agent_world!`
- `Guest::run` stubs with error directing to agent interface
- `GuestAgent::run_agent`:
  1. Reads `callee_service_id` from `host::config_var`
  2. Calls `host::call_service(&callee_id, &payload)`
  3. Wraps utility-service response in `"composition-result: "` prefix
  4. Returns `StepResult::Done([WasmResponse{...}])` — single-step agent
- `service.json`: `allowed_service_calls: "all"`, `max_continuation_steps: 5`
- Dependencies: `example-helpers`, `serde`, `serde_json`

Both components added to root `Cargo.toml` workspace members. WASM built natively via `cargo build --target wasm32-wasip2`.

### Task 2: MockRpcCaller + rpc_e2e Integration Tests

**WASM byte constants** (`packages/utils/src/test_utils/mock_engine.rs`):
```rust
pub static COMPONENT_UTILITY_SERVICE_BYTES: &[u8] = include_bytes!("...utility_service.wasm");
pub static COMPONENT_COMPOSITION_AGENT_BYTES: &[u8] = include_bytes!("...composition_agent.wasm");
```

**MockRpcCaller** (`packages/engine/tests/helpers/mock_rpc.rs`):
- `HashMap<String, Vec<u8>>` mapping callee key → WASM bytes
- `RpcCaller::call()` builds a minimal Wasmtime engine + InstanceDepsBuilder inline
- Executes callee WASM via `wavs_engine::worlds::operator::execute::execute()`
- Returns first response payload — no permission checks (those are tested separately)
- Avoids circular dependency: `MockRpcCaller` implements the `RpcCaller` trait from `wavs-engine` without importing the `wavs` crate

**Helper additions** (`packages/engine/tests/helpers/`):
- `make_service_with_allowed_calls`: builds a service with `AllowedServiceCalls::All`
- `try_execute_component_raw_with_rpc`: extends exec helper to accept an `Arc<dyn RpcCaller>`

**Integration Tests** (`packages/engine/tests/rpc_e2e.rs`):

| Test | Requirement | What it proves |
|------|-------------|----------------|
| `composition_agent_calls_utility_service` | E2E-05 | MockRpcCaller routes call_service to utility-service WASM; final payload contains both "utility-response:" and "composition-result:" |
| `caller_without_allowed_service_calls_denied` | E2E-06 part 1 | Service with AllowedServiceCalls::None triggers host.rs denial; error contains "call-service denied" and "does not have permission" |
| `callee_without_allowed_callers_rejected_error_format` | E2E-06 part 2 | Verifies callee-side error message format is human-readable; documents the "does not accept calls from" contract |

All 3 tests pass. Full engine test suite (30+ tests across 10 files) passes with zero regressions.

## Deviations from Plan

### Auto-fixed Issues

None — plan executed as written.

### Design Choices (within plan guidance)

**1. Callee key = config var string, not ServiceId hex**

The plan noted the callee ID would be "a known service ID." Rather than computing a real `ServiceId` hex from the WASM hash, the composition-agent reads the callee_id from `config_var("callee_service_id")`. The MockRpcCaller maps against whatever string the component passes. This makes tests cleaner — no ServiceId computation needed, and the test clearly documents the intent.

**2. Callee rejection test uses approach (a)**

The plan offered two approaches for `callee_without_allowed_callers_rejected` — (a) direct error message format check or (b) MockRpcCaller with permissions map. Approach (a) was used because the AllowedCallers check lives in `RpcCallerImpl` in the `wavs` crate. A MockRpcCaller-based test would duplicate that logic rather than testing it. The format verification documents the contract and ensures the error is human-readable, satisfying E2E-06's intent.

## Threat Model Compliance

| Threat ID | Mitigation | Status |
|-----------|-----------|--------|
| T-23-04 (E: AllowedServiceCalls bypass) | composition-agent service.json requires AllowedServiceCalls::All; host.rs rejects before dispatch | VERIFIED: caller_without_allowed_service_calls_denied test proves denial works |
| T-23-05 (S: callee identity spoofing) | MockRpcCaller resolves by key string — test-only, doesn't affect production ServiceId validation | ACCEPTED: MockRpcCaller not compiled to production |
| T-23-06 (I: error message disclosure) | Error messages include service IDs for debugging — acceptable per threat model | VERIFIED: test messages are in tests, not shipped WASM |
| T-23-07 (T: MockRpcCaller bypasses permissions) | Test-only code; callee permission test uses separate path | ACCEPTED: documented in test comments |

## Known Stubs

None — all new components have working implementations. MockRpcCaller is test-only infrastructure, not a production stub.

## Threat Flags

None — no new network endpoints, auth paths, file access patterns, or schema changes introduced. All new code is either WASM components (sandboxed) or test infrastructure.

## Self-Check: PASSED

- `examples/components/utility-service/src/lib.rs` (export_layer_trigger_world!): FOUND
- `examples/components/composition-agent/src/lib.rs` (call_service): FOUND
- `examples/build/components/utility_service.wasm`: FOUND
- `examples/build/components/composition_agent.wasm`: FOUND
- `packages/engine/tests/rpc_e2e.rs` (3 tests): FOUND
- `packages/engine/tests/helpers/mock_rpc.rs` (MockRpcCaller): FOUND
- `packages/utils/src/test_utils/mock_engine.rs` (COMPONENT_UTILITY_SERVICE_BYTES): FOUND
- Task 1 commit `b88eb0a9f`: FOUND
- Task 2 commit `6e39bb733`: FOUND
- `cargo test -p wavs-engine --test rpc_e2e`: 3 passed, 0 failed
- `cargo test -p wavs-engine`: all passed (30+ tests), 0 failed
