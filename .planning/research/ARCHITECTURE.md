# Architecture Research

**Domain:** Agent continuation and service-to-service RPC for WAVS (WASM AVS runtime)
**Researched:** 2026-04-20
**Confidence:** HIGH — based on direct codebase inspection of all relevant subsystems

---

## Existing Architecture (Baseline)

Understanding the current system precisely is essential because both v3.0 features integrate _into_ it rather than replacing it.

### Current Execution Flow

```
TriggerManager
    │  crossbeam channel: DispatcherCommand::Trigger(TriggerAction)
    ▼
Dispatcher (main loop, packages/wavs/src/dispatcher.rs)
    │  crossbeam channel: EngineCommand::ExecuteOperator { service, action }
    ▼
EngineManager (packages/wavs/src/subsystems/engine.rs)
    │  ctx.rt.spawn(async) → WasmEngine::execute_operator_component()
    │  crossbeam channel: DispatcherCommand::EngineResponse(EngineResponse::Operator)
    ▼
Dispatcher
    │  crossbeam channel: SubmissionCommand::Submit(SubmissionRequest)
    ▼
SubmissionManager
    │  crossbeam channel: DispatcherCommand::SubmissionResponse(Submission)
    ▼
Dispatcher
    │  crossbeam channel: AggregatorCommand::Broadcast(Submission)
    ▼
Aggregator (P2P quorum)
    │  crossbeam channel: DispatcherCommand::AggregatorExecute { submission, service, kind }
    ▼
Dispatcher
    │  crossbeam channel: EngineCommand::ExecuteAggregator { submission, service, kind }
    ▼
EngineManager → WasmEngine::execute_aggregator_component()
    │  crossbeam channel: DispatcherCommand::EngineResponse(EngineResponse::Aggregator)
    ▼
Dispatcher → AggregatorCommand::Actions → on-chain submission
```

### Key Existing Types

| Type | Location | Role |
|------|----------|------|
| `EngineCommand` | `subsystems/engine.rs` | Commands sent from Dispatcher to EngineManager. Currently: `Kill`, `ExecuteOperator`, `ExecuteAggregator` |
| `EngineResponse` | `subsystems/engine.rs` | Responses sent EngineManager to Dispatcher. Currently: `Operator(SubmissionRequest)`, `Aggregator { submission, actions, kind }` |
| `DispatcherCommand` | `dispatcher.rs` | All subsystem to Dispatcher messages. Currently: `Trigger`, `ChangeServiceUri`, `EngineResponse`, `SubmissionResponse`, `AggregatorExecute`, `SubmissionConfirmed`, `SubmissionFailed` |
| `OperatorHostComponent` | `engine/src/worlds/operator/component.rs` | Wasmtime `Store` data — host capabilities exposed to WASM. Has: `wasi:http`, `wasi:keyvalue`, chain configs, permissions |
| `WavsWorld` (WIT) | `wit-definitions/operator/wit/operator.wit` | Guest interface. Entry: `export run: func(trigger-action) -> result<list<wasm-response>, string>` |
| `Permissions` | `types/src/service.rs` | Per-component capability flags: `allowed_http_hosts: AllowedHostPermission`, `file_system`, `raw_sockets`, `dns_resolution` |
| `AllowedHostPermission` | `types/src/service.rs` | `All` / `Only(Vec<String>)` / `None` — enforced via `configure_linker()` in `worlds/instance.rs` |

---

## v3.0 Integration Design

### Feature 1: Agent Continuation Mode

**What it is:** The component's `run` function returns `Continue { state: bytes }` or `Done { responses: list<wasm-response> }` instead of a flat list. When `Continue` is returned, the Engine re-invokes the component with the accumulated state, looping until `Done` is returned.

**Where it lives in the existing architecture:**

The continuation loop belongs **inside `EngineManager::run_trigger()`** (or a new `run_trigger_with_continuation()` alongside it). The Dispatcher and all downstream subsystems (Submission, Aggregator) are unaffected — they still receive `SubmissionRequest` exactly as today. The loop is entirely an Engine-internal concern.

```
Current:  run_trigger() → execute_operator_component() → Vec<WasmResponse>
v3.0:     run_trigger() → loop { execute_operator_step() → Continue(state) | Done(responses) }
                          └─ on Done: → Vec<WasmResponse> (same as today)
```

**Data flow changes:**

1. **WIT interface change** (new return variant) — `operator.wit` gets a new output type:
   ```wit
   variant step-result {
     continue(list<u8>),          // persisted state for next step
     done(list<wasm-response>),   // terminal, same as today's return
   }
   // new export replaces or supplements run:
   export run: func(trigger-action: trigger-action) -> result<step-result, string>;
   ```
   Backward compat: keep old `run` export path working for non-agent components (they never return `Continue`).

2. **State persistence between steps** — `wasi:keyvalue` is already a host capability. The engine auto-persists continuation state under a well-known key (`continuation:<service_id>:<correlation_id>`) between steps. The component can also read/write KV directly for conversation history (wavs-rig already does this for memory).

3. **No new channel messages needed** — the loop runs inside the single `ctx.rt.spawn` task that currently calls `execute_operator_component`. The Dispatcher sees only the final `Done` result, as a normal `EngineResponse::Operator`.

4. **Fuel/time budgeting** — continuation steps each run against the workflow's per-step fuel/time limits. A new `max_continuation_steps: Option<u32>` field in `Workflow` (or `Component`) caps infinite loops. Exceeding it returns an error identical to `EngineError::OutOfFuel`.

**New/modified components:**

| Component | Change Type | What Changes |
|-----------|------------|--------------|
| `wit-definitions/operator/wit/operator.wit` | Modified | New `step-result` variant; `run` return type updated |
| `packages/engine/src/worlds/operator/execute.rs` | Modified | `execute()` becomes a step; new `execute_with_continuation()` loop wrapper |
| `packages/engine/src/worlds/operator/component.rs` | Minor modify | `OperatorHostComponent` gains continuation state slot (or uses KV directly) |
| `packages/wavs/src/subsystems/engine.rs` | Minor modify | `run_trigger()` calls new `execute_with_continuation()` instead of `execute_operator_component()` |
| `packages/types/src/service.rs` | Modified | `Component` or `Workflow` gains `max_continuation_steps: Option<u32>` |
| `packages/engine/src/bindings/` | Regenerated | WIT bindings regenerated after operator.wit change |

**Dispatcher untouched.** EngineCommand and EngineResponse enum variants stay the same. No new channels.

---

### Feature 2: Service-to-Service Synchronous RPC via `call-service`

**What it is:** A host function exposed to the WASM guest that synchronously executes another deployed service's operator component and returns its `WasmResponse` bytes. The caller specifies a target `service_id` and `workflow_id`; the engine runs that component inline and returns the result.

**Where it lives:** This is a new **host function** added to `OperatorHostComponent`. It runs the target service synchronously within the same `ctx.rt.spawn` task that is executing the calling component. No new Crossbeam channels are needed — the engine already owns `Arc<WasmEngine<S>>` and can call `execute_operator_component()` recursively.

**Data flow:**

```
Component A's run() call
    │ calls host: call-service("target_service_id", "workflow_id", input_bytes)
    ▼
OperatorHostComponent::call_service() [new host fn impl]
    │ validates AllowedServiceCalls permission
    │ looks up target service from Services registry
    │ calls WasmEngine::execute_operator_component(target_service, synthetic_trigger)
    │ (this is an async call made synchronous within the WASI context via block_on / executor)
    ▼
Returns Vec<u8> (serialized WasmResponse payload) to calling component
    │
Component A continues with result
```

**Synchronization model:** The host function is `async fn call_service(...)` but exposed through wasmtime's host binding mechanism, which handles the async bridge into the sync WASI component execution context. This is the same pattern used by `wasi:http/outgoing-handler` today — the host function is async on the host side, WASI components see it as blocking.

**WIT changes:**

```wit
// In operator.wit, add to the host interface:
import host: interface {
    // ... existing functions ...

    // Synchronously execute another deployed service and return its response payload.
    // Returns error string if service not found, permission denied, or execution fails.
    call-service: func(
        service-id: string,
        workflow-id: string,
        input: list<u8>
    ) -> result<list<u8>, string>;
}
```

**Permission check — `AllowedServiceCalls`:**

New field on `Permissions` struct in `types/src/service.rs`:

```rust
pub struct Permissions {
    pub allowed_http_hosts: AllowedHostPermission,
    pub file_system: bool,
    pub raw_sockets: bool,
    pub dns_resolution: bool,
    // NEW:
    pub allowed_service_calls: AllowedServiceCalls,  // All / Only(Vec<ServiceId>) / None
}

pub enum AllowedServiceCalls {
    All,
    Only(Vec<String>),  // service_id strings
    #[default]
    None,
}
```

The host function implementation checks this field before executing the target. Deny returns `Err("service call not permitted")` to the component.

**Cycle prevention:** The host function must detect and break call cycles (A calls B calls A). Simplest approach: thread-local or `Store`-data call stack depth counter; reject if depth > N (default: 5). This prevents stack overflow without requiring global state.

**New/modified components:**

| Component | Change Type | What Changes |
|-----------|------------|--------------|
| `wit-definitions/operator/wit/operator.wit` | Modified | `call-service` added to `host` interface |
| `packages/engine/src/worlds/operator/component.rs` | Modified | `OperatorHostComponent` gains `call_service_impl` — needs access to `Arc<WasmEngine<S>>` and `Services` |
| `packages/engine/src/worlds/operator/` (host impl) | Modified | Implement `call-service` host function in the bindings impl block |
| `packages/engine/src/worlds/instance.rs` | Modified | `InstanceDepsBuilder` passes `Arc<WasmEngine<S>>` and `Services` into `OperatorHostComponent` |
| `packages/engine/src/common/base_engine.rs` | Minor | Ensure `WasmEngine` is `Arc`-shareable for re-entrant calls |
| `packages/types/src/service.rs` | Modified | `Permissions` gains `allowed_service_calls: AllowedServiceCalls` |
| `packages/engine/src/bindings/` | Regenerated | WIT bindings regenerated |

**Dispatcher untouched.** No new channels. No new `EngineCommand` variants. The engine re-enters itself within the same Tokio task.

---

## Combined System Architecture (v3.0)

```
+------------------------------------------------------------------+
|                        WAVS Dispatcher                            |
|  crossbeam channels: Trigger -> Engine -> Submit -> Aggregate    |
+---------------------------+--------------------------------------+
                            | EngineCommand::ExecuteOperator
                            v
+------------------------------------------------------------------+
|                       EngineManager                               |
|  ctx.rt.spawn -> run_trigger_with_continuation()                  |
|  +------------------------------------------------------------+  |
|  |  Continuation Loop [NEW]                                    |  |
|  |  while step == Continue {                                   |  |
|  |      execute_operator_step(state) -> Continue | Done        |  |
|  |      auto-persist state to wasi:keyvalue                    |  |
|  |  }                                                          |  |
|  +-------------------------+---------------------------------+  |  |
|                            | Done(Vec<WasmResponse>)            |
|                            v                                     |
|  DispatcherCommand::EngineResponse(Operator) [unchanged]         |
+------------------------------------------------------------------+

+------------------------------------------------------------------+
|              WasmEngine::execute_operator_step()                  |
|                                                                   |
|  +------------------------------------------------------------+  |
|  |  OperatorHostComponent (Wasmtime Store data)                |  |
|  |  +- wasi:http/outgoing-handler  (existing)                  |  |
|  |  +- wasi:keyvalue               (existing)                  |  |
|  |  +- host::config-var            (existing)                  |  |
|  |  +- host::get-evm-chain-config  (existing)                  |  |
|  |  +- host::call-service [NEW] ----------------------------+  |  |
|  |  +- host::log           (existing)                       |  |  |
|  +------------------------------------------------------------+  |  |
|                                                              |  |
|  call-service host fn impl:                                  |  |
|  +- check AllowedServiceCalls permission                     |  |
|  +- check call depth (cycle prevention)                      |  |
|  +- look up target Service from Services registry            |  |
|  +- build synthetic TriggerAction with input bytes           |  |
|  +- Arc<WasmEngine>::execute_operator_component(target) <----+  |
+------------------------------------------------------------------+
```

---

## Component Boundaries

| Component | Owns | Communicates With | v3.0 Changes |
|-----------|------|-------------------|--------------|
| `Dispatcher` | Channel routing, service lifecycle | All subsystems via crossbeam | None |
| `EngineManager` | Spawn tasks, route results | Dispatcher (in/out), WasmEngine | Adds continuation loop in `run_trigger` |
| `WasmEngine` | Wasmtime instantiation, execution | EngineManager (called), host functions (calls back) | Adds `execute_operator_step`; `OperatorHostComponent` gains `call-service` |
| `OperatorHostComponent` | WASI store data, host fn impls | WasmEngine (holds ref), re-enters WasmEngine for `call-service` | Gains `Arc<WasmEngine>`, `Services`, depth counter, `allowed_service_calls` check |
| `TriggerManager` | Event monitoring, firing | Dispatcher | None |
| `SubmissionManager` | Signing, submission | Dispatcher | None |
| `Aggregator` | Quorum, P2P | Dispatcher | None |
| `packages/types` | Service, Permissions, WasmResponse types | All | Adds `AllowedServiceCalls`, `max_continuation_steps`, possibly `StepResult` type |

---

## Architectural Patterns

### Pattern 1: In-Task Continuation Loop

**What:** The re-invocation loop for continuation lives entirely within the single `ctx.rt.spawn` task that executes the operator component. No new OS threads, no new Tokio tasks, no new channels.

**When to use:** Always for continuation. Keeps the concurrency model simple — the Dispatcher's view is unchanged; each trigger still produces at most one `EngineResponse::Operator` per workflow invocation.

**Trade-offs:** Pro: zero impact on Dispatcher, Submission, Aggregator. Con: a long-running agent with many continuation steps ties up one Tokio task. Acceptable given that fuel/step limits cap execution time. If concurrency ever matters, the loop can be made interruptible.

### Pattern 2: Re-entrant WasmEngine for call-service

**What:** `call-service` calls `Arc<WasmEngine>::execute_operator_component()` recursively within the same async task. `WasmEngine` is already `Arc`-wrapped and stateless per call (all state lives in `Store`).

**When to use:** Always for service-to-service RPC. Avoids introducing a new synchronous channel round-trip through the Dispatcher (which would deadlock: the engine is blocked waiting for the channel result while the Dispatcher is blocked waiting for the engine to finish).

**Trade-offs:** Pro: no deadlock risk, no new channels, minimal latency. Con: re-entrant execution means a misbehaving callee can hold fuel/time from the caller's budget. Mitigate with per-call fuel sub-limits and depth checking.

**Deadlock note — critical:** Do NOT route `call-service` through the Dispatcher via a new channel. The `EngineManager::start()` loop is a blocking `while let Ok(command) = rx.recv()`. If the engine sends a new command to itself via the Dispatcher while already executing, and the response expects synchronous delivery, you face a classic deadlock. The re-entrant `Arc<WasmEngine>` approach is the correct solution.

### Pattern 3: State Persistence via Existing KV

**What:** Continuation state is persisted to `wasi:keyvalue` (already a host capability) under a deterministic key per service/trigger/step. No new storage backend.

**When to use:** Default auto-persist for agents. Components can also read/write KV directly for richer state (wavs-rig memory already uses KV for conversation history).

**Trade-offs:** Pro: zero new infrastructure, operators already have KV. Con: KV is local to each operator — state is not shared across operators in a multi-operator deployment. This is acceptable for agent use cases (each operator runs the agent independently and submits independently).

---

## Data Flow: Continuation Mode

```
TriggerAction arrives
    |
EngineManager::run_trigger_with_continuation(action, service)
    |
step 0: execute_operator_step(trigger_action, state=None)
    -> Continue(state_bytes)
    | persist state_bytes to KV["continuation:<svc_id>:<correlation_id>:step:0"]
step 1: execute_operator_step(trigger_action, state=Some(state_bytes))
    -> Continue(state_bytes_2)
    | persist ...
step N: execute_operator_step(trigger_action, state=Some(state_bytes_N))
    -> Done(Vec<WasmResponse>)
    |
DispatcherCommand::EngineResponse(EngineResponse::Operator(SubmissionRequest))
    |
[normal pipeline: Submit -> Aggregate -> On-chain]
```

## Data Flow: Service-to-Service RPC

```
ComponentA::run(trigger_action) executing inside WasmEngine
    |
calls host function: call-service("service_b_id", "workflow_0", input_bytes)
    |
OperatorHostComponent::call_service() [host impl]
    +- check AllowedServiceCalls::Only(["service_b_id"]) -> OK
    +- check call_depth <= 5 -> OK; increment depth
    +- services.get("service_b_id") -> Service B
    +- build TriggerData::Manual { data: input_bytes }
    +- Arc<WasmEngine>::execute_operator_component(service_b, synthetic_trigger).await
            |
        ComponentB::run(synthetic_trigger) executes
            -> Done(Vec<WasmResponse>)
            |
        returns Vec<WasmResponse>[0].payload as bytes to ComponentA
    | decrement depth
returns Ok(result_bytes) to ComponentA
    |
ComponentA continues reasoning with result_bytes
```

---

## Suggested Build Order

The features have clear dependencies. Build in this sequence:

### Phase 1: WIT + Types Foundation (no behavior change)
1. Extend `Permissions` in `packages/types/src/service.rs` to add `AllowedServiceCalls` enum and field.
2. Add `max_continuation_steps: Option<u32>` to `Component` in types.
3. Update `operator.wit` with `step-result` variant and `call-service` host function signature.
4. Regenerate WIT bindings (`packages/engine/src/bindings/`).

**Rationale:** Everything downstream depends on these types. Do it first so all code compiles against the new interface. No behavior changes yet.

### Phase 2: Continuation Mode — Engine Loop
5. Add `execute_operator_step()` to `WasmEngine` (single step, returns `StepResult`).
6. Add `run_trigger_with_continuation()` to `EngineManager` wrapping the loop.
7. Wire KV auto-persist of continuation state.
8. Add step limit enforcement (return `EngineError` on exceeded).
9. Update `EngineManager::run_trigger()` to call the continuation-aware version.

**Rationale:** No changes to Dispatcher, Submission, or Aggregator. Can be tested in isolation by writing a component that returns `Continue` N times then `Done`. No `call-service` needed yet.

### Phase 3: Service-to-Service RPC
10. Update `InstanceDepsBuilder` to accept `Arc<WasmEngine<S>>` and `Services`.
11. Add `call_depth: usize` counter to `OperatorHostComponent`.
12. Implement `call-service` host function in the operator world binding impl.
13. Add `AllowedServiceCalls` permission check inside the host fn impl.
14. Write cycle detection (depth limit).

**Rationale:** Depends on Phase 1 (types + WIT) but not on Phase 2 (continuation). Can develop in parallel with Phase 2 if needed, but sequential is simpler.

### Phase 4: Integration + Permissions UI
15. Expose `AllowedServiceCalls` in service.json schema and documentation.
16. Add `allowed_service_calls` to Tauri component detail page (if relevant).
17. E2E test: agent A triggers, calls service B, returns combined result.

**Rationale:** Visible surface — do last so the core is proven before wiring up UI.

---

## Anti-Patterns

### Anti-Pattern 1: Routing call-service Through the Dispatcher

**What people do:** Add a new `EngineCommand::CallService` and have the host fn send it on the channel then wait for a response channel.

**Why it's wrong:** Deadlock risk. `EngineManager::start()` is a blocking `while let Ok(command) = rx.recv()` loop. It spawns operator execution as a Tokio task. That task's host fn would need to synchronously receive a response from the Dispatcher, but if the Dispatcher is also waiting for the engine task to finish before processing the next command, you have a cycle. Even if Tokio tasks avoid true deadlock, the synchronization complexity is unnecessary.

**Do this instead:** `Arc<WasmEngine>::execute_operator_component()` called directly inside the host function. No channels involved.

### Anti-Pattern 2: Storing Continuation State in EngineManager Memory

**What people do:** Keep a `HashMap<CorrelationId, StateBytes>` in `EngineManager` to store continuation state between steps.

**Why it's wrong:** The continuation loop runs within a single task — state does not need to persist across tasks. Using in-memory maps adds lifetime complexity and breaks on node restart. KV store is already available, namespaced per service, and persists across restarts.

**Do this instead:** Use `wasi:keyvalue` with a deterministic key. Auto-persist in the loop, read at step start.

### Anti-Pattern 3: New WIT World for Agent Components

**What people do:** Create a separate `agent-world` WIT world with the continuation interface instead of extending `wavs-world`.

**Why it's wrong:** Breaks backward compatibility. Operators would need to know which world a component uses before instantiating it. The current dispatch path (`WavsWorld::instantiate_async`) works uniformly on all operator components.

**Do this instead:** Extend `wavs-world` with the new return variant. Non-agent components never return `Continue` — the engine handles both cases in one instantiation path.

### Anti-Pattern 4: Per-Step Full Re-Compilation

**What people do:** Call `load_component_from_source()` at every continuation step (loads and compiles WASM).

**Why it's wrong:** WASM compilation is expensive (100–500ms for large components). A 10-step agent adds 1–5 seconds of overhead.

**Do this instead:** The existing `WasmEngine` already uses an LRU cache keyed by `ComponentDigest`. Ensure the continuation loop passes the same component digest each step. Cache hit = instantiation only (fast), no recompilation.

---

## Integration Points Summary

| Boundary | Communication | v3.0 Impact |
|----------|---------------|-------------|
| Dispatcher to EngineManager | Crossbeam channels (`EngineCommand` / `DispatcherCommand`) | None — same channel, same variants |
| EngineManager to WasmEngine | Direct async method calls | New `execute_operator_step()` method; existing `execute_operator_component()` preserved |
| WasmEngine to OperatorHostComponent | Wasmtime `Store` data | `OperatorHostComponent` gains `Arc<WasmEngine>`, `Services`, `allowed_service_calls`, depth counter |
| OperatorHostComponent to WasmEngine (call-service) | Re-entrant async call within same task | New re-entrant path; must be through `Arc` not `&mut` |
| Operator component to host | WIT interface | `step-result` variant added; `call-service` host fn added |
| `Permissions` / service.json | Serde deserialization | `allowed_service_calls` field added; default `None` preserves backward compat |

---

## Sources

- Direct inspection: `packages/wavs/src/dispatcher.rs` (lines 1–460)
- Direct inspection: `packages/wavs/src/subsystems/engine.rs` (full)
- Direct inspection: `packages/engine/src/worlds/operator/execute.rs` (full)
- Direct inspection: `packages/engine/src/worlds/operator/component.rs` (full)
- Direct inspection: `packages/engine/src/worlds/instance.rs` (lines 1–260)
- Direct inspection: `packages/wavs/src/subsystems/engine/wasm_engine.rs` (lines 1–340)
- Direct inspection: `wit-definitions/operator/wit/operator.wit` (full)
- Direct inspection: `packages/types/src/service.rs` (lines 600–700, Permissions, AllowedHostPermission)
- Direct inspection: `.planning/PROJECT.md` (milestone context and requirements)

---
*Architecture research for: WAVS v3.0 — Agent Continuation and Service-to-Service RPC*
*Researched: 2026-04-20*
