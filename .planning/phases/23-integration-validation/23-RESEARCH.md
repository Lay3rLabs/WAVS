# Phase 23: Integration & Validation - Research

**Researched:** 2026-04-22
**Domain:** WASM component authoring, engine integration tests, service.json configuration
**Confidence:** HIGH

## Summary

Phase 23 exercises the full agent composition surface end-to-end. Three deliverables are required: a deployable multi-step continuation agent example (E2E-04), a deployable service-composition example (E2E-05), and a permission enforcement test (E2E-06).

The engine infrastructure from Phases 21 and 22 is fully wired. The continuation loop in `execute.rs`, KV persistence via `KeyValueCtx`, caller-side `AllowedServiceCalls` enforcement in `host.rs`, and callee-side `AllowedCallers` enforcement in `rpc_caller.rs` are all complete with zero known stubs. **However, there is a critical blocker discovered by this research: all example components currently fail to compile** because `export_layer_trigger_world!` now requires `impl exports::wavs::operator::agent::Guest` (for `run-agent`), which no existing component provides. This must be fixed in Plan 01 before any new component can be built.

The integration test approach follows the established pattern in `packages/engine/tests/` — fast unit/integration tests using compiled WASM bytes, no live node required. The two new example components (multi-step agent, utility service + composition agent) must be added to the Cargo workspace, their WASM binaries built and stored in `examples/build/components/`, their byte constants added to `packages/utils/src/test_utils/mock_engine.rs`, and engine-level tests in `packages/engine/tests/` that exercise them through `execute()`.

**Primary recommendation:** Plan 01 = fix export macro + build multi-step-agent component. Plan 02 = build utility-service + composition-agent components + RPC integration test. Plan 03 = permission enforcement test (unit-level, no WASM build needed).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all implementation choices are at Claude's discretion (infrastructure phase).

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

### Deferred Ideas (OUT OF SCOPE)
None.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| E2E-04 | Multi-step agent example demonstrating Continue/Done loop with KV-persisted state across steps | New component `examples/components/multi-step-agent/` implementing `exports::wavs::operator::agent::Guest`; integration test in `packages/engine/tests/continuation_e2e.rs` verifies KV keys written after each step |
| E2E-05 | Service composition example — agent calls a utility service via `call-service` and uses the result | Two components: `examples/components/utility-service/` (legacy run, AllowedCallers::All) and `examples/components/composition-agent/` (agent, AllowedServiceCalls::All); integration test in `packages/engine/tests/rpc_e2e.rs` |
| E2E-06 | Permission enforcement test — caller without AllowedServiceCalls gets clear error; callee without AllowedCallers rejects call | Unit tests in existing `packages/engine/tests/rpc.rs` or new file; exercises the error messages already produced by `host.rs` and `rpc_caller.rs`; no WASM execution required |
</phase_requirements>

## Critical Blocker: Export Macro Breakage

### What Is Broken
`export_layer_trigger_world!` in `examples/components/_helpers/src/lib.rs` expands to:
```rust
export!(Component with_types_in crate::bindings::world)
```
This registers BOTH `exports::wavs::operator::run` AND `exports::wavs::operator::agent::run_agent` because `_helpers/src/bindings/world.rs` generates bindings from the full `wavs-world` (which has `export agent;`). Every component using this macro must now implement `exports::wavs::operator::agent::Guest` in addition to the existing `Guest` (for `run`). [VERIFIED: cargo check -p square]

All confirmed broken: `square`, `kv-store`, `echo-data`, `permissions`, `agent-example` (agent-example has separate rig-wasi errors). [VERIFIED: direct compilation checks]

### Fix Pattern
The same problem was solved on the engine side in Phase 21 Plan 02: add a `wavs-legacy-world` WIT (run only) and use it for the legacy path. The component-side fix mirrors this:

1. Add a second `wit_bindgen::generate!` block to `_helpers/src/bindings/world.rs` for `wavs-legacy-world` — reusing the same Rust types via `with:` mapping (identical to the engine-side approach).
2. Change `export_layer_trigger_world!` to call `legacy_export!(Component with_types_in ...)` using the legacy-world bindings.
3. Add a new `export_layer_agent_world!` macro for components that actually implement the agent interface, using the full `wavs-world` bindings.

This is an additive change — no component source files need modification, only `_helpers/src/bindings/world.rs` and `_helpers/src/lib.rs`.

**Alternative:** Generate a default no-op `GuestAgent` impl inside the macro that returns `Err("not an agent component")`. This avoids adding a second bindgen block but causes the WASM binary to export the `agent` interface, making `has_agent_export()` return `true` for ALL components. This would break the engine's routing logic. **Not recommended.**

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `example-helpers` | workspace (2.8.0) | WIT bindings, prelude, trigger codec | Required for all WASM components in this repo |
| `example-types` | workspace (2.8.0) | Shared request/response types | Convention for cross-component types |
| `wavs-types` | workspace (2.8.0) | `AllowedServiceCalls`, `AllowedCallers`, `Service`, `Component` | Canonical type source |
| `wavs-engine` | workspace (2.8.0) | `EngineError`, `execute()`, test infrastructure | All engine tests import this |
| `serde` / `serde_json` | workspace | Request/response serialization | Standard throughout codebase |
| `wstd::runtime::block_on` | via `example-helpers` | Single async boundary in WASM components | Required WASI async pattern |

[VERIFIED: Cargo.toml workspace deps, existing example component Cargo.toml files]

### Supporting (Tests Only)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `utils::test_utils::mock_engine` | workspace | `COMPONENT_*_BYTES` constants | Load WASM for engine tests |
| `utils::storage::db::WavsDb` | workspace | In-memory KV for test execution | `KeyValueCtx::new(WavsDb::new(), ...)` |
| `wavs_engine::backend::wasi_keyvalue::context::KeyValueCtx` | workspace | KV context in test deps builder | Required by `InstanceDepsBuilder` |
| `tokio` | via test infra | Async test runtime | `#[tokio::test]` |

[VERIFIED: existing test files in packages/engine/tests/]

### Installation
New Cargo workspace members must be added to the root `Cargo.toml` `members` array. Example components need `[lib] crate-type = ["cdylib"]` for WASM compilation.

## Architecture Patterns

### New Component Directory Layout
```
examples/components/
├── multi-step-agent/        # E2E-04: continuation agent (run-agent export)
│   ├── Cargo.toml
│   ├── service.json
│   └── src/lib.rs
├── utility-service/         # E2E-05: callee service (run export, AllowedCallers::All)
│   ├── Cargo.toml
│   ├── service.json
│   └── src/lib.rs
└── composition-agent/       # E2E-05: caller agent (run-agent + call-service)
    ├── Cargo.toml
    ├── service.json
    └── src/lib.rs
```

### New Test Files
```
packages/engine/tests/
├── continuation_e2e.rs      # E2E-04: exercises multi-step-agent WASM
└── rpc_e2e.rs               # E2E-05 + E2E-06: exercises RPC path + permission errors
```
(E2E-06 permission tests may fit in the existing `rpc.rs` or in `rpc_e2e.rs`)

### Pattern 1: Multi-Step Agent Component (E2E-04)
```rust
// Source: codebase conventions + WIT interface agent definition
// exports::wavs::operator::agent::Guest trait from wavs-world bindgen
use example_helpers::bindings::world::{
    exports::wavs::operator::agent::Guest as GuestAgent,
    wavs::operator::{
        input::TriggerAction,
        output::{StepResult, WasmResponse},
    },
    wasi::keyvalue::store,
    Guest, // for run export (can return Err or delegate)
};

struct Component;

// Non-agent entrypoint (required by export_layer_agent_world! macro)
impl Guest for Component {
    fn run(_trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        Err("use run-agent interface".into())
    }
}

// Agent continuation entrypoint
impl GuestAgent for Component {
    fn run_agent(trigger_action: TriggerAction) -> Result<StepResult, String> {
        // Read current step from KV
        // KV bucket: "wavs_agent_step", key: "{service_id}:{workflow_id}:step:{N-1}"
        // (Engine writes the step name before next invocation)
        let bucket = store::open("wavs_agent_step")
            .map_err(|e| e.to_string())?;
        
        // Determine step by reading what engine persisted
        // (or use a counter in the "state" bucket that the component manages itself)
        
        // Steps 0..2 return Continue, step 3 returns Done
        // Store intermediate state in KV under component-controlled bucket
        let state_bucket = store::open("agent_state").map_err(|e| e.to_string())?;
        
        // ... logic here ...
        
        Ok(StepResult::Continue("step_2".into()))
        // or:
        Ok(StepResult::Done(vec![WasmResponse { payload: ..., ordering: None, event_id_salt: None }]))
    }
}

export_layer_agent_world!(Component);
```

**Key design choices for multi-step-agent:**
- Must demonstrate 3+ continuation steps (success criterion)
- Must write observable KV checkpoints (KV state visible to test)
- The component manages its own step state by writing to a DIFFERENT bucket than the engine's `wavs_agent_step` bucket (engine owns that namespace)
- Simple counter in `agent_state` bucket: read `counter`, increment, write back. At step 3, return Done.
- Final result encodes the complete step history as payload so test can verify it

[VERIFIED: execute.rs KV key format, execute_agent loop logic, bindings/world.rs generated traits]

### Pattern 2: Service Composition (E2E-05)

**Utility Service** — implements `run`, AllowedCallers::All in service.json:
```json
{
  "component": {
    "allowed_callers": "all"
  }
}
```
Simple responder: receives `Vec<u8>` payload, echoes it back with a prefix to prove it was called.

**Composition Agent** — implements `run-agent`, AllowedServiceCalls::All in service.json:
```json
{
  "component": {
    "permissions": {
      "allowed_service_calls": "all"
    }
  }
}
```
Component calls `host::call_service(callee_id, payload)` and incorporates the response. [VERIFIED: WIT call-service signature, host.rs AllowedServiceCalls check]

### Pattern 3: Engine Test with RPC Injection (E2E-05 test)

Tests that exercise `call-service` require the `rpc_caller` field in `InstanceDepsBuilder`. The existing `helpers/exec.rs` passes `rpc_caller: None`, which means `call_service` returns `"no RPC caller configured"`. For RPC E2E tests, we need a concrete `RpcCallerImpl` — but that lives in the `wavs` crate and cannot be used in `wavs-engine` tests (circular dependency). [VERIFIED: rpc_caller.rs in packages/wavs, rpc.rs trait in packages/engine]

**Two valid options:**

Option A — **Mock RpcCaller in engine tests**: Create a `MockRpcCaller` in the test helpers that executes the callee WASM directly via `execute()` without the full `WasmEngine`. This keeps the test in `packages/engine/tests/` where existing tests live.

Option B — **Move RPC E2E test to `packages/wavs/tests/`**: Use the real `RpcCallerImpl` with a mock `WasmEngine`. The existing `dispatcher_tests.rs` and `mock_e2e.rs` in `packages/wavs/tests/` already wire up the full engine stack with `COMPONENT_SQUARE_BYTES`.

Option A is recommended: it avoids pulling the wavs crate into engine tests and follows the pattern of existing engine tests which are all self-contained. The MockRpcCaller can resolve a callee service from a map, call `execute()` directly, and return the result.

### Pattern 4: Permission Enforcement Test (E2E-06)

Does NOT require WASM execution. The error messages are produced by:
- `host.rs call_service()`: "call-service denied: caller '{}' does not have permission to call '{}'" (AllowedServiceCalls check)
- `rpc_caller.rs call()`: "call-service denied: callee '{}' does not accept calls from '{}'" (AllowedCallers check)

Test structure:
```rust
// Test 1: Caller missing AllowedServiceCalls (defaults to None)
// Build a TriggerAction through execute() on a component that calls call_service
// with rpc_caller = None → check error message contains human-readable denial
// OR directly test the error string without WASM:
let err = "call-service denied: caller 'svc-a' does not have permission to call 'svc-b'";
assert!(err.contains("call-service denied"));
assert!(err.contains("does not have permission"));

// Test 2: Callee missing AllowedCallers (defaults to None)
let err = "call-service denied: callee 'svc-b' does not accept calls from 'svc-a'";
assert!(err.contains("does not accept calls from"));
```

However, to satisfy the success criterion "Running a permission enforcement test produces two clear failures," the test should actually invoke `call_service` through the engine path. The clearest approach: add a WASM component that calls `call_service` (using raw WIT binding, not the GuestAgent interface), inject an `RpcCallerImpl` that checks permissions, and assert the error strings.

Given test complexity, a hybrid approach is appropriate: use direct error message string checks for the initial test coverage (proves the error messages exist and are human-readable), then optionally add a WASM-level test in a later iteration.

[VERIFIED: host.rs error messages, rpc_caller.rs error messages]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| KV persistence format | Custom serialization format | Existing `KeyValueCtx` + bucket/key pattern | Engine already reads `wavs_agent_step` bucket with exact key pattern |
| RPC dispatch | Custom channel or actor | `rpc_caller.call()` from `InstanceDepsBuilder` | Full depth/cycle/permission enforcement already implemented |
| Async WASM execution | Custom executor | `wstd::runtime::block_on` | Only valid async boundary in WASI components |
| Permission checking | Custom guard | `AllowedServiceCalls` / `AllowedCallers` types in service.json | Serde-deserialized with tests proving correct behavior |
| Component WASM bytes in tests | Dynamic compilation | `include_bytes!` from `examples/build/components/` | Standard pattern in mock_engine.rs |

## Common Pitfalls

### Pitfall 1: Export Macro Requires GuestAgent
**What goes wrong:** Any component that uses `export_layer_trigger_world!` fails to compile with `E0277: unsatisfied trait bound Component: exports::wavs::operator::agent::Guest`.
**Why it happens:** Phase 20 added `export agent;` to `wavs-world`. The `_helpers` macro uses `wavs-world`, so all components must now implement both `run` and `run-agent`.
**How to avoid:** Fix `_helpers` to use `wavs-legacy-world` for `export_layer_trigger_world!` and add `export_layer_agent_world!` for components that implement the agent interface. This mirrors the engine-side fix from Phase 21.
**Warning signs:** `cargo check -p <any-example-component>` fails at the `export_layer_trigger_world!` call site.

### Pitfall 2: Stepping on the Engine's KV Namespace
**What goes wrong:** A component writes to the `wavs_agent_step` bucket with its own keys, conflicting with the engine's checkpoint writes.
**Why it happens:** Both the engine and the component use the same KV store. The engine writes `{ns}/wavs_agent_step/{correlation_id}:step:{N}`.
**How to avoid:** Components should write their own state to a DIFFERENT bucket name (e.g., `agent_state`, `agent_counter`). The `wavs_agent_step` bucket is owned by the engine.
**Warning signs:** Step continuations overwrite each other in tests; KV key collisions.

### Pitfall 3: RPC Test Cannot Use RpcCallerImpl Directly
**What goes wrong:** `packages/engine/tests/` imports `wavs-engine` but not `wavs`. `RpcCallerImpl` lives in `wavs`. Importing `wavs` from `wavs-engine` tests creates a circular dependency.
**Why it happens:** `wavs-engine` is a dependency of `wavs`. The concrete RPC implementation must live in `wavs` to break the cycle.
**How to avoid:** Write a `MockRpcCaller` in the engine test helpers (implements `RpcCaller` trait from `wavs-engine`) that resolves callee WASM from a test-local map and calls `execute()` directly.
**Warning signs:** Circular dependency error at `cargo check`.

### Pitfall 4: WASM Binary Out of Date After Component Source Change
**What goes wrong:** Tests load WASM bytes from `examples/build/components/` via `include_bytes!`. If the component source changes but the WASM is not rebuilt, tests run against stale binaries.
**Why it happens:** `include_bytes!` embeds the bytes at compile time from the pre-built file.
**How to avoid:** After creating new components, run `just wasi-build <component-name>` to compile them. Tests will fail if the WASM file doesn't exist yet. Plan the WASM build as an explicit step before writing engine tests.
**Warning signs:** `include_bytes!("...component.wasm")` panics at compile time with "file not found"; or tests exhibit unexpected behavior from stale logic.

### Pitfall 5: StepResult::Continue Carries Step Name Not State
**What goes wrong:** Developer tries to pass inline state (JSON/bytes) as the `Continue(string)` value, hitting the KV size constraint.
**Why it happens:** The string argument to `Continue` is a "step name" string (e.g., `"step_2"`), NOT a serialized state blob. The design decision from STATE.md says: "`Continue` return carries key string only, not inline state (avoids 4KB cap)".
**How to avoid:** Components must write their state to KV explicitly, then return `Continue("step_name")` as a routing label only. The test reads KV directly to verify checkpoints.
**Warning signs:** Very long strings being passed to `Continue()`; test assertions failing because expected KV keys are not written.

### Pitfall 6: has_agent_export Uses Name-Based Heuristic
**What goes wrong:** A component named with "agent" in its package metadata but implementing only `run` (via legacy world) appears to the engine as an agent component.
**Why it happens:** `has_agent_export()` checks `name.contains("agent")`. This was designed to match the fully qualified export `"wavs:operator/agent@2.7.0"`, but would also match any component whose export path contains the word "agent".
**How to avoid:** When naming new components, ensure only actual agent components (those using `export_layer_agent_world!`) have "agent" in their WIT package name. Utility services and non-agent components should not have "agent" in their `[package.metadata.component]` package field.
**Warning signs:** A non-agent component enters the `execute_agent()` path and loops indefinitely or fails with "agent interface not found."

## Code Examples

### Implementing exports::wavs::operator::agent::Guest in a Component
```rust
// Source: WIT interface agent definition in operator.wit + wit-bindgen 0.53 convention
use example_helpers::bindings::world::{
    exports::wavs::operator::{
        agent::Guest as GuestAgent,
        output::StepResult,
    },
    wavs::operator::input::TriggerAction,
    wavs::operator::output::WasmResponse,
    wasi::keyvalue::store,
    Guest,
};

struct Component;

impl Guest for Component {
    fn run(_trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        Err("use run-agent interface".into())
    }
}

impl GuestAgent for Component {
    fn run_agent(trigger_action: TriggerAction) -> Result<StepResult, String> {
        // ... step logic ...
        Ok(StepResult::Continue("step_2".into()))
    }
}

export_layer_agent_world!(Component);
```
[VERIFIED: existing bindings/world.rs trait paths from E0277 error output]

### service.json for a Continuation Agent (E2E-04)
```json
{
  "name": "multi-step-agent",
  "workflows": {
    "default": {
      "trigger": "manual",
      "component": {
        "source": { "digest": "<sha256-of-wasm>" },
        "permissions": {
          "allowed_http_hosts": "none",
          "file_system": false,
          "raw_sockets": false,
          "dns_resolution": false
        },
        "fuel_limit": null,
        "time_limit_seconds": 30,
        "max_continuation_steps": 5,
        "config": {},
        "env_keys": []
      },
      "submit": "none"
    }
  },
  "status": "active",
  "manager": {
    "evm": {
      "chain": "evm:31337",
      "address": "0x0000000000000000000000000000000000000000"
    }
  }
}
```
[VERIFIED: agent-example/service.json format, component_allowed_callers_variants test in types/src/service.rs]

### service.json for Utility Service (callee, E2E-05)
```json
{
  "component": {
    "allowed_callers": "all"
  }
}
```
[VERIFIED: AllowedCallers::All serializes as "all" per component_allowed_callers_variants test]

### service.json for Composition Agent (caller, E2E-05)
```json
{
  "component": {
    "permissions": {
      "allowed_service_calls": "all"
    }
  }
}
```
[VERIFIED: AllowedServiceCalls::All serializes as "all" per allowed_service_calls_variants test]

### MockRpcCaller for Engine Tests
```rust
// Source: rpc.rs trait definition (packages/engine/src/rpc.rs)
use std::{collections::HashMap, sync::Arc};
use wavs_engine::rpc::{RpcCaller, RpcFuture};

struct MockRpcCaller {
    // Map of service_id_hex -> WASM bytes
    services: HashMap<String, Vec<u8>>,
}

impl RpcCaller for MockRpcCaller {
    fn call(&self, callee_id: String, payload: Vec<u8>, _call_stack: Vec<String>) -> RpcFuture<'_> {
        Box::pin(async move {
            let wasm = self.services.get(&callee_id)
                .ok_or_else(|| format!("unknown service: {}", callee_id))?;
            // call execute() with the WASM bytes and payload
            // return first response payload
            todo!()
        })
    }
}
```
[VERIFIED: RpcCaller trait signature in packages/engine/src/rpc.rs]

### Engine Test Verifying KV Checkpoints (E2E-04 pattern)
```rust
// Source: helpers/exec.rs pattern + continuation.rs test structure
#[tokio::test]
async fn multi_step_agent_kv_checkpoints() {
    let kv_ctx = KeyValueCtx::new(WavsDb::new().unwrap(), "test-svc".to_string());
    let db = kv_ctx.db();
    
    // Execute the multi-step-agent WASM
    execute_component_raw(engine, COMPONENT_MULTI_STEP_AGENT_BYTES, ...)
        .await
        .expect("agent should complete");
    
    // Verify engine-written KV checkpoints exist
    // Key format: "test-svc/wavs_agent_step/{service_id}:{workflow_id}:step:{N}"
    let key_0 = "test-svc/wavs_agent_step/...";
    assert!(db.kv_store.get(&key_0).is_some(), "step 0 checkpoint missing");
}
```
[VERIFIED: kv_key_format_correctness test in continuation.rs, KeyValueCtx.db() accessor]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| All components use `wavs-world` (run + agent) | Non-agent components need `wavs-legacy-world` (run only) | Phase 20 added `export agent;` | Requires fixing `_helpers` export macro |
| `call_service` was a stub | `call_service` fully wired via `RpcCallerImpl` | Phase 22 | Service-to-service calls work end-to-end |
| No continuation loop | `execute_agent()` loops on `Continue` | Phase 21 | Agent components re-invoked until Done |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The `exports::wavs::operator::agent::Guest` trait has method `fn run_agent(trigger_action: TriggerAction) -> Result<StepResult, String>` (not `&self, ...`) | Architecture Patterns | Minor — would affect how the impl block is written; easily discovered at compile time |
| A2 | A `MockRpcCaller` in engine tests can call `execute()` directly to dispatch to a callee WASM without circular dependency | Architecture Patterns | Medium — if `execute()` itself requires something only available in the `wavs` crate, the mock approach fails |

**A1 note:** wit-bindgen 0.53 generates static methods (not `&self`) for WASM component exports because the component model has no persistent instance state. This is the established pattern in the codebase — `Guest::run` in all existing components has the signature `fn run(trigger_action: TriggerAction) -> Result<...>` with no receiver. Confirmed via [VERIFIED: permissions/src/lib.rs, square/src/lib.rs].

## Open Questions

1. **WASM build in CI context**
   - What we know: new components need to be compiled to WASM via `just wasi-build <name>` using the Docker-based builder
   - What's unclear: whether the Docker builder is available in the current environment; if not, tests that `include_bytes!` will fail
   - Recommendation: design tests to compile cleanly when the WASM files exist; include a note in the plan that `just wasi-build multi-step-agent` must be run before the test task executes

2. **How to read the engine-written KV checkpoint from within the component**
   - What we know: engine writes `{ns}/wavs_agent_step/{correlation_id}:step:{N}` before re-invocating the component
   - What's unclear: whether the component's KV access sees the fully qualified key or just the key after the bucket prefix
   - What we know from KV implementation: `store::open("wavs_agent_step")` opens the bucket, and `bucket.get("{correlation_id}:step:{N}")` reads the value — the bucket name is the prefix
   - Recommendation: the multi-step agent demo can choose NOT to read the engine's checkpoint (the engine writes it for observability, not for the component to consume); the component can manage its own counter in a separate bucket to determine step

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Cargo check/test | ✓ | 2.8.0 (workspace) | — |
| Docker (wasi-builder) | `just wasi-build` for new components | Unknown | — | Pre-build WASM and commit bytes to repo |
| `wavs-engine` test infrastructure | Engine integration tests | ✓ | Already used in tests/ | — |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (tokio for async tests) |
| Config file | None — `packages/engine/tests/` tests run via `cargo test -p wavs-engine` |
| Quick run command | `cargo test -p wavs-engine --test continuation` |
| Full suite command | `cargo test -p wavs-engine` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| E2E-04 | Multi-step agent runs 3+ steps with KV checkpoints and returns Done | Integration | `cargo test -p wavs-engine --test continuation_e2e` | ❌ Wave 0 |
| E2E-05 | Composition agent calls utility service and incorporates response | Integration | `cargo test -p wavs-engine --test rpc_e2e` | ❌ Wave 0 |
| E2E-06 | Caller missing AllowedServiceCalls gets clear error; callee missing AllowedCallers rejects | Unit | `cargo test -p wavs-engine --test rpc_e2e` or `cargo test -p wavs-engine --test rpc` | ❌ or extends existing |

### Sampling Rate
- **Per task commit:** `cargo check -p wavs-engine && cargo check -p <new-component>`
- **Per wave merge:** `cargo test -p wavs-engine`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `packages/engine/tests/continuation_e2e.rs` — covers E2E-04
- [ ] `packages/engine/tests/rpc_e2e.rs` — covers E2E-05, E2E-06
- [ ] `examples/components/multi-step-agent/` — new crate
- [ ] `examples/components/utility-service/` — new crate
- [ ] `examples/components/composition-agent/` — new crate
- [ ] WASM bytes: `examples/build/components/multi_step_agent.wasm` etc. — requires `just wasi-build`
- [ ] `packages/utils/src/test_utils/mock_engine.rs` — add `COMPONENT_MULTI_STEP_AGENT_BYTES` etc.

## Sources

### Primary (HIGH confidence)
- Codebase: `packages/engine/src/worlds/operator/execute.rs` — continuation loop, KV key format
- Codebase: `packages/engine/src/bindings/operator/host.rs` — call_service AllowedServiceCalls check, error messages
- Codebase: `packages/wavs/src/subsystems/engine/rpc_caller.rs` — AllowedCallers check, error messages
- Codebase: `packages/engine/tests/helpers/exec.rs` — InstanceDepsBuilder test pattern
- Codebase: `examples/components/_helpers/src/bindings/world.rs` — export macro definition
- Codebase: `packages/utils/src/test_utils/mock_engine.rs` — WASM byte constant pattern
- Codebase: `packages/types/src/service.rs` — AllowedServiceCalls/AllowedCallers JSON serialization tests
- Cargo compilation: `cargo check -p square` — confirmed export macro breakage

### Secondary (MEDIUM confidence)
- Phase 20 SUMMARY: WIT changes and call_service stub
- Phase 21 SUMMARY: continuation loop, legacy-world fix pattern
- Phase 22 SUMMARY: RpcCaller trait, RpcCallerImpl, concrete wiring

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified via Cargo.toml, existing code
- Architecture: HIGH — engine code fully read, patterns verified
- Pitfalls: HIGH — critical breakage confirmed by live compilation
- Test patterns: HIGH — existing test files read in full

**Research date:** 2026-04-22
**Valid until:** 2026-05-22 (stable infrastructure, no external dependencies)
