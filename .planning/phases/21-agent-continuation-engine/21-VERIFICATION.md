---
phase: 21-agent-continuation-engine
verified: 2026-04-22T00:00:00Z
status: human_needed
score: 3/5 must-haves verified (2 deferred to Phase 23)
deferred:
  - truth: "An agent component that returns Continue three times then Done is invoked four times total by the engine within a single trigger execution"
    addressed_in: "Phase 23"
    evidence: "Phase 23 SC-1: 'A deployable multi-step agent example exists that triggers, runs 3+ continuation steps with KV-persisted state, and returns a final result'"
  - truth: "Between each continuation step, the agent's conversation history and tool results are readable from KV under the wavs_agent_step: key"
    addressed_in: "Phase 23"
    evidence: "Phase 23 SC-1: 'a developer can deploy it and observe each step's KV checkpoint'"
human_verification:
  - test: "End-to-end continuation loop invocation count"
    expected: "A WASM component that exports the agent interface and returns Continue three times then Done is invoked exactly four times by the engine before returning the Done result to the aggregator"
    why_human: "No WASM component with run-agent / StepResult::Continue export exists yet (agent_example.wasm uses run, not run-agent). The engine code is correct but the loop invocation count cannot be proven without a real agent WASM binary that exercises the continue path."
  - test: "KV state readable between steps"
    expected: "After each Continue step, the step name written at wavs_agent_step:{service_id}:{workflow_id}:step:N is readable by the component on the next invocation via bucket.open('wavs_agent_step').get('{service_id}:{workflow_id}:step:N')"
    why_human: "Requires a real agent WASM component to read back from KV on re-invocation. The host-side write path is verified in code, but the round-trip read cannot be proven without an end-to-end test component."
---

# Phase 21: Agent Continuation Engine Verification Report

**Phase Goal:** An agent component returning `Continue` is automatically re-invoked by the engine, with conversation and tool results persisted to KV between steps under the `wavs_agent_step:` key prefix, and a hard step limit that terminates runaway agents with a clear error
**Verified:** 2026-04-22
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #   | Truth | Status | Evidence |
| --- | ----- | ------ | -------- |
| 1   | An agent component that returns `Continue` three times then `Done` is invoked four times total | DEFERRED Phase 23 | Loop code exists in execute_agent(); no agent WASM with run-agent export to test end-to-end |
| 2   | KV state readable under `wavs_agent_step:` between steps | DEFERRED Phase 23 | Host-side write verified in code; round-trip read requires real agent WASM |
| 3   | When `max_continuation_steps` exceeded, engine returns `ContinuationLimit` error | VERIFIED | `EngineError::ContinuationLimit` variant confirmed in error.rs; step-limit check at line 169 of execute.rs; test `continuation_limit_error_format` passes |
| 4   | Named `continue("step_name")` handoffs — step name persisted to KV for retrieval | VERIFIED | `StepResult::Continue(step_name)` branch writes to `{namespace}/wavs_agent_step/{correlation_id}:step:{N}`; KV key format test passes |
| 5   | Compiled WASM module not evicted from LRU between steps | VERIFIED | `let _component_pin = deps.component.clone()` at execute.rs:161 holds Arc clone for loop lifetime |

**Score:** 3/5 truths verified (2 deferred to Phase 23)

### Deferred Items

Items not yet met but explicitly addressed in later milestone phases.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | Agent invoked 4 times on 3 Continue + 1 Done | Phase 23 | SC-1: "multi-step agent example...runs 3+ continuation steps" |
| 2 | KV state readable from component between steps | Phase 23 | SC-1: "developer can...observe each step's KV checkpoint" |

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `packages/engine/src/utils/error.rs` | ContinuationLimit error variant | VERIFIED | Lines 36-41: `ContinuationLimit { service_id, workflow_id, steps }` with correct Display format |
| `packages/engine/src/worlds/operator/execute.rs` | Agent detection, continuation loop, KV persistence, LRU pinning | VERIFIED | `has_agent_export()`, `execute_agent()`, `execute_legacy()`, `_component_pin`, `set_fuel()`, `wavs_agent_step` KV write all present |
| `packages/engine/src/backend/wasi_keyvalue/context.rs` | `pub fn db()` accessor for host-side KV writes | VERIFIED | Line 35: `pub fn db(&self) -> WavsDb { self.db.clone() }` |
| `packages/engine/tests/continuation.rs` | Integration tests for continuation loop | VERIFIED | 6 tests: error format (2), KV key format (2), legacy fallback (2). All pass. |
| `packages/wavs/src/subsystems/engine/wasm_engine.rs` | Production caller compiles with updated engine | VERIFIED | `cargo check -p wavs-engine` passes; wasm_engine.rs calls `execute::execute()` at line 182 with same 4-param signature |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| `execute.rs` | `WavsWorld::instantiate_async` + `wavs_operator_agent().call_run_agent()` | agent detection + loop | WIRED | Lines 189-199: instantiates WavsWorld, calls `wavs_operator_agent().call_run_agent()` in agent loop |
| `execute.rs` | `WavsLegacyWorld::instantiate_async` + `call_run()` | legacy path | WIRED | Lines 92-99: `execute_legacy()` uses `legacy::WavsLegacyWorld` (run-only world) |
| `execute.rs` | `db.kv_store.insert(kv_key, ...)` | host-side KV write | WIRED | Lines 231-236: inserts `{namespace}/wavs_agent_step/{correlation_id}:step:{N}` |
| `execute.rs` | `deps.store.as_operator_mut().set_fuel(fuel_limit)` | per-step fuel reset | WIRED | Lines 257-260: fuel reset between continuation steps |
| `execute_agent()` | `EngineError::ContinuationLimit` | step counter check | WIRED | Lines 169-174: `if step >= max_steps { return Err(ContinuationLimit { ... }) }` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| `execute_agent()` | `max_steps` | `host.service.workflows.get(workflow_id).and_then(|w| w.component.max_continuation_steps).unwrap_or(10)` | Yes — reads from service config | FLOWING |
| `execute_agent()` | `db` / `kv_namespace` | `host.keyvalue_ctx.db()` / `host.service.id().to_string()` | Yes — real WavsDb (DashMap) clone | FLOWING |
| `execute_agent()` | `StepResult` from `call_run_agent` | wasmtime component execution | Requires real agent WASM | DEFERRED |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| `cargo check -p wavs-engine` | `cargo check -p wavs-engine` | `Finished dev profile` | PASS |
| ContinuationLimit error format test | `cargo test -p wavs-engine --test continuation` | 6 passed, 0 failed | PASS |
| Legacy component execution (7²=49) | `cargo test -p wavs-engine --test continuation` | `legacy_component_still_works` ok | PASS |
| KV key format correctness | `cargo test -p wavs-engine --test continuation` | `kv_key_format_correctness` ok | PASS |
| Basic engine test | `cargo test -p wavs-engine --test basic` | 1 passed, 0 failed | PASS |
| KV engine tests | `cargo test -p wavs-engine --test keyvalue` | 7 passed, 0 failed | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ---------- | ----------- | ------ | -------- |
| CONT-01 | 21-01, 21-02 | Engine re-invocation loop calling execute_operator_step, checks Continue/Done, repeats | SATISFIED | `execute_agent()` in execute.rs: loop on `call_run_agent()`, branches on `StepResult::Continue`/`StepResult::Done` |
| CONT-02 | 21-01, 21-02 | Auto-persist agent state to KV between steps under `wavs_agent_step:` key pattern | SATISFIED (CODE) | Host writes `{svc_id}/wavs_agent_step/{svc_id}:{wfl_id}:step:{N}`; readable via bucket `wavs_agent_step`. Note: REQUIREMENTS.md says `continuation:` prefix but ROADMAP (authoritative) says `wavs_agent_step:` — implementation matches ROADMAP. Round-trip read deferred to Phase 23. |
| CONT-03 | 21-01, 21-02 | Step limit enforcement — clear error when max_continuation_steps exceeded | SATISFIED | `ContinuationLimit { service_id, workflow_id, steps }` variant; enforced at execute_agent() line 169; tested by `continuation_limit_error_format` |
| CONT-04 | 21-01, 21-02 | Developer-defined multi-step workflows using named continue("step_name") handoffs | SATISFIED (CODE) | `StepResult::Continue(step_name)` written to KV; component can read step name on re-invocation. End-to-end routing test deferred to Phase 23 |
| CONT-05 | 21-01, 21-02 | Component LRU pinning between continuation steps | SATISFIED | `let _component_pin = deps.component.clone()` at execute.rs:161 holds the Arc-backed compiled module for the loop's lifetime |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| None found | — | — | — | No stubs, TODOs, or placeholders in phase deliverables |

### Human Verification Required

#### 1. Agent Continuation Loop Invocation Count

**Test:** Build or locate a WASM component that exports the `agent` interface (`run-agent` function returning `StepResult`). Configure it to return `Continue("step1")`, `Continue("step2")`, `Continue("step3")` on the first three calls, then `Done([response])` on the fourth. Execute it via the WAVS engine and verify it was invoked exactly four times.

**Expected:** The engine's `execute_agent()` loop calls `call_run_agent()` four times, the first three return Continue, the fourth returns Done, and the final `Vec<WasmResponse>` is what reaches the aggregator.

**Why human:** No WASM component implementing the `run-agent` / `export agent` interface exists yet in the test suite. `agent_example.wasm` uses the `run` (legacy) export path — it does NOT exercise the continuation loop. This test requires either compiling a new test component or waiting for Phase 23's example agent.

#### 2. KV State Round-Trip Between Steps

**Test:** Using the same agent component from Test 1, after each `Continue` step, open the `wavs_agent_step` KV bucket from within the component code and call `get("{service_id}:{workflow_id}:step:N")`. Verify the returned value matches the string that was passed in the previous `Continue("step_name")` call.

**Expected:** The host writes `step_name` bytes to the KV store between steps. The component reads the value back and can use it to route to the correct handler function on re-invocation.

**Why human:** The host-side write path is verified in code and tests. Verifying the component-side read requires a real agent WASM component that reads from KV and asserts the value — only possible with Phase 23's integration example.

### Gaps Summary

No blocking gaps — all engine infrastructure is correctly implemented and compiles. The two human verification items (invocation count, KV round-trip) require a real agent WASM component that exercises the continuation path. Phase 23 explicitly covers this ("multi-step agent example...runs 3+ continuation steps with KV-persisted state"). The engine code is wired end-to-end; this is a test coverage gap, not an implementation gap.

One minor discrepancy noted: REQUIREMENTS.md CONT-02 specifies key pattern `continuation:<service_id>:<correlation_id>:step:N` but the ROADMAP success criteria (authoritative) and phase goal both specify `wavs_agent_step:` prefix. The implementation follows the ROADMAP. REQUIREMENTS.md should be updated to reflect the `wavs_agent_step` bucket name, but this does not block the phase.

---

_Verified: 2026-04-22_
_Verifier: Claude (gsd-verifier)_
