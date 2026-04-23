---
phase: 23-integration-validation
verified: 2026-04-23T14:24:28Z
status: human_needed
score: 5/6 must-haves verified
human_verification:
  - test: "Deploy composition-agent and utility-service to a live WAVS node, send a manual trigger to composition-agent with a known payload, and confirm the response contains both 'utility-response:' and 'composition-result:' prefixes in the on-chain submission"
    expected: "Final on-chain result payload reads 'composition-result: utility-response: <payload>' proving the full call_service path worked end-to-end including real service registration and routing"
    why_human: "The rpc_e2e test uses a MockRpcCaller that executes callee WASM inline without going through the production RpcCallerImpl in the wavs crate. The real end-to-end path (ServiceRegistry lookup, AllowedCallers enforcement, cross-service dispatch) is not exercised in the engine-level tests."
  - test: "Deploy a service with AllowedCallers::None (default), then deploy a composition-agent that targets it, send a trigger, and verify the WAVS node returns an error containing 'call-service denied: callee does not accept calls from'"
    expected: "WAVS node rejects the RPC call at the callee boundary via RpcCallerImpl, and the error message is visible in node logs or error response"
    why_human: "The callee_without_allowed_callers_rejected_error_format test in rpc_e2e.rs constructs the error string synthetically (approach a from plan) rather than executing through RpcCallerImpl in packages/wavs/src/subsystems/engine/rpc_caller.rs. The production code path is not exercised in any automated test."
---

# Phase 23: Integration & Validation Verification Report

**Phase Goal:** The full agent composition surface is exercised end-to-end — a multi-step continuation agent, a service-composition agent that calls a utility service, and a permission enforcement test that proves both AllowedServiceCalls and AllowedCallers reject unauthorized calls
**Verified:** 2026-04-23T14:24:28Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | All existing example components (square, kv-store, echo-data, permissions) compile with cargo check | VERIFIED | `cargo check -p square -p kv-store -p echo-data -p permissions` exits 0 in 4.94s |
| 2 | A new multi-step-agent component exports both run and run-agent interfaces | VERIFIED | `examples/components/multi-step-agent/src/lib.rs` implements `Guest::run` and `GuestAgent::run_agent`; `export_layer_agent_world!(Component)` at line 87 |
| 3 | The multi-step-agent runs 3+ continuation steps and returns Done with KV-persisted state at each step | VERIFIED | `continuation_e2e::multi_step_agent_runs_to_completion` confirms 4-step loop with JSON summary; `multi_step_agent_kv_checkpoints_exist` confirms checkpoint:0..3 in WavsDb |
| 4 | A utility-service component receives a payload and returns a prefixed response | VERIFIED | `utility-service/src/lib.rs` prepends "utility-response: " to Raw payload; compiled to WASM at `examples/build/components/utility_service.wasm` |
| 5 | A composition-agent calls utility-service via call-service and incorporates its response | VERIFIED | `rpc_e2e::composition_agent_calls_utility_service` passes — response contains both "utility-response:" and "composition-result:"; actual WASM-to-WASM execution via MockRpcCaller |
| 6 | Both AllowedServiceCalls and AllowedCallers reject unauthorized calls with clear human-readable errors | PARTIAL | AllowedServiceCalls denial: VERIFIED via live WASM execution in `caller_without_allowed_service_calls_denied` (error contains "call-service denied" + "does not have permission"). AllowedCallers rejection: format documented in `callee_without_allowed_callers_rejected_error_format` but NOT exercised via actual WASM execution — test constructs the error string synthetically |

**Score:** 5.5/6 truths verified (AllowedCallers enforcement logic exists in production code but callee rejection is not exercised end-to-end in automated tests)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `examples/components/_helpers/src/bindings/world.rs` | Dual bindgen: wavs-legacy-world for legacy, wavs-world for agents | VERIFIED | `pub mod legacy_world` at line 24 with `with:` remappings; main world bindgen also present |
| `examples/components/_helpers/src/lib.rs` | export_layer_trigger_world uses legacy world; export_layer_agent_world uses full world | VERIFIED | Both macros defined; trigger_world delegates to legacy_world::export!, agent_world delegates to world::export! |
| `examples/components/multi-step-agent/src/lib.rs` | Multi-step continuation agent with KV state persistence | VERIFIED | GuestAgent impl at line 32; StepResult::Continue at line 62; StepResult::Done at line 78; agent_state bucket at line 36 |
| `packages/engine/tests/continuation_e2e.rs` | Integration test exercising multi-step agent through engine | VERIFIED | 2 tests pass: multi_step_agent_runs_to_completion, multi_step_agent_kv_checkpoints_exist |
| `examples/components/utility-service/src/lib.rs` | Simple echo-with-prefix callee service | VERIFIED | Guest impl with "utility-response: " prefix; export_layer_trigger_world! |
| `examples/components/composition-agent/src/lib.rs` | Agent that calls utility-service via call_service | VERIFIED | GuestAgent impl calls host::call_service at line 57; reads callee_service_id from config_var |
| `packages/engine/tests/rpc_e2e.rs` | Integration tests for RPC composition and permission enforcement | VERIFIED | 3 tests: composition_agent_calls_utility_service, caller_without_allowed_service_calls_denied, callee_without_allowed_callers_rejected_error_format |
| `examples/build/components/multi_step_agent.wasm` | Compiled WASM binary | VERIFIED | File exists; used by continuation_e2e tests via COMPONENT_MULTI_STEP_AGENT_BYTES |
| `examples/build/components/utility_service.wasm` | Compiled WASM binary | VERIFIED | File exists; used by rpc_e2e tests via COMPONENT_UTILITY_SERVICE_BYTES |
| `examples/build/components/composition_agent.wasm` | Compiled WASM binary | VERIFIED | File exists; used by rpc_e2e tests via COMPONENT_COMPOSITION_AGENT_BYTES |
| `packages/engine/tests/helpers/mock_rpc.rs` | MockRpcCaller for engine-level RPC tests | VERIFIED | Full implementation executing callee WASM via wavs_engine::worlds::operator::execute::execute |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `_helpers/src/lib.rs` | `_helpers/src/bindings/world.rs` | export_layer_trigger_world uses legacy_world | WIRED | legacy_world::export!($Component) call present |
| `_helpers/src/lib.rs` | `_helpers/src/bindings/world.rs` | export_layer_agent_world uses full world | WIRED | world::export!($Component) call present |
| `multi-step-agent/src/lib.rs` | `_helpers/src/lib.rs` | export_layer_agent_world! macro invocation | WIRED | `export_layer_agent_world!(Component)` at line 87 |
| `continuation_e2e.rs` | `mock_engine.rs` | COMPONENT_MULTI_STEP_AGENT_BYTES constant | WIRED | imported and used to load WASM for test execution |
| `composition-agent/src/lib.rs` | `_helpers/src/bindings/world.rs` | call_service host import from WIT bindings | WIRED | `host::call_service(&callee_id, &payload)` at line 57 |
| `rpc_e2e.rs` | `mock_engine.rs` | COMPONENT_UTILITY_SERVICE_BYTES + COMPONENT_COMPOSITION_AGENT_BYTES | WIRED | Both constants imported and used in test setup |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `continuation_e2e.rs` | `summary: Vec<String>` | WASM execution of multi_step_agent.wasm through engine | Yes — engine executes real WASM, KV checkpoints written by component | FLOWING |
| `rpc_e2e.rs::composition_agent_calls_utility_service` | `response: String` | composition_agent.wasm → MockRpcCaller → utility_service.wasm → "utility-response: " prefix | Yes — two-layer real WASM execution; response contains both prefixes | FLOWING |
| `rpc_e2e.rs::caller_without_allowed_service_calls_denied` | `err: String` | WASM execution hits host.rs AllowedServiceCalls::None check, returns real error | Yes — actual engine permission denial | FLOWING |
| `rpc_e2e.rs::callee_without_allowed_callers_rejected_error_format` | `err: String` | Constructed synthetically via format! macro | No — does not execute RpcCallerImpl code path | STATIC (format-only test) |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| continuation_e2e tests pass | `cargo test -p wavs-engine --test continuation_e2e` | 2 passed, 0 failed in 6.72s | PASS |
| rpc_e2e tests pass | `cargo test -p wavs-engine --test rpc_e2e` | 3 passed, 0 failed in 11.71s | PASS |
| Full engine test suite (no regressions) | `cargo test -p wavs-engine` | 10 test suites, 29+ tests, 0 failed | PASS |
| Legacy components still compile | `cargo check -p square -p kv-store -p echo-data -p permissions` | All exit 0 in 4.94s | PASS |
| New components compile | `cargo check -p multi-step-agent -p utility-service -p composition-agent` | All exit 0 in 1.19s | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| E2E-04 | 23-01-PLAN.md | Multi-step agent example demonstrating Continue/Done loop with KV-persisted state across steps | SATISFIED | multi-step-agent demonstrates 4-step KV-checkpointed continuation; 2 integration tests pass |
| E2E-05 | 23-02-PLAN.md | Service composition example — agent calls a utility service via call-service and uses the result | SATISFIED | composition_agent_calls_utility_service test passes; response contains both "utility-response:" and "composition-result:" proving the call traversed two WASM components |
| E2E-06 | 23-02-PLAN.md | Permission enforcement test — caller without AllowedServiceCalls gets clear error; callee without AllowedCallers rejects call | PARTIAL | AllowedServiceCalls denial: SATISFIED (live WASM test). AllowedCallers rejection: PARTIAL (format-only synthetic test, not live WASM execution through RpcCallerImpl) |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found | — | — | — | — |

No TODOs, FIXMEs, placeholder returns, or hardcoded empty data were found in any phase-23 files.

### Human Verification Required

#### 1. Service Composition End-to-End with Live WAVS Node

**Test:** Deploy utility-service and composition-agent to a live WAVS node using their service.json configs. Send a manual trigger to composition-agent with a known byte payload. Examine the WAVS node output or on-chain submission.

**Expected:** The final result payload is "composition-result: utility-response: <original payload>". This proves the full production dispatch path works: composition-agent WASM → host.rs call_service → production RpcCallerImpl (wavs crate) → service registry lookup → utility-service WASM execution → response back through the call chain.

**Why human:** The `composition_agent_calls_utility_service` test uses `MockRpcCaller` which executes callee WASM inline, bypassing the production `RpcCallerImpl` in `packages/wavs/src/subsystems/engine/rpc_caller.rs`. The real dispatch path includes service registry lookup, AllowedCallers enforcement, and call-stack tracking. These are not exercised in any automated engine-level test due to the wavs/wavs-engine circular dependency constraint documented in the Phase 23-02 summary.

#### 2. AllowedCallers Callee Rejection via Live Execution

**Test:** Configure a service with `allowed_callers: null` (the default), deploy a composition-agent that targets it, send a trigger, and observe the WAVS node error response.

**Expected:** The node returns an error containing "call-service denied: callee '...' does not accept calls from '...'". Both service IDs appear in the error message.

**Why human:** The `callee_without_allowed_callers_rejected_error_format` test in `rpc_e2e.rs` constructs the expected error string using `format!()` and asserts properties on the constructed string. It does not execute any WASM or call any production code. The actual enforcement is at `packages/wavs/src/subsystems/engine/rpc_caller.rs:66-72` and is only reachable through the wavs crate's `RpcCallerImpl`, which cannot be imported in engine-level tests without creating a circular dependency. The production logic has been code-reviewed and is correct, but live execution has not been demonstrated.

### Gaps Summary

No blocking gaps found. All 5 core truths are fully verified with passing automated tests. The partial status on E2E-06 (AllowedCallers rejection) is a known limitation of the test architecture (circular dependency between `wavs-engine` and `wavs` crates) documented in the plan and summary. The production enforcement logic exists and is correct at `rpc_caller.rs:62-72`, and the error format contract is verified. Two human verification items remain to exercise the full production dispatch path.

---

_Verified: 2026-04-23T14:24:28Z_
_Verifier: Claude (gsd-verifier)_
