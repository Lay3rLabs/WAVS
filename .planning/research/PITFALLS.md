# Pitfalls Research

**Domain:** Agent continuation mode and synchronous service-to-service RPC in WASI/Wasmtime runtime (v3.0)
**Researched:** 2026-04-20
**Confidence:** HIGH — based on direct codebase inspection of engine, dispatcher, KV store, and WIT interface code, plus verified Wasmtime embedding API behavior

---

## Critical Pitfalls

### Pitfall 1: Continuation Loop Runs Inside a Single Wasmtime Invocation — Re-instantiation Is Required, Not Resume

**What goes wrong:**
Developers assume agent continuation means the component is "paused and resumed" — that the WASM execution stack is preserved across steps. It is not. Every WAVS operator execution (`execute_operator_component`) creates a fresh Wasmtime `Store`, instantiates a new component, calls `call_run`, and then discards the store. There is no mechanism to serialize and restore a live WASM stack. Continuation must be implemented by re-invoking the component from scratch, passing the persisted state as input, not by suspending execution mid-function.

**Why it happens:**
The word "continuation" in agent frameworks usually implies coroutine-style suspension (async generators, delimited continuations). In WASM component model p2, the component model has no stack suspension primitive. WASI p3's async streams add this, but WAVS is on p2. Developers who come from Python/asyncio or Rust async backgrounds expect `Continue` to mean "resume from where I left off." In this runtime it means "re-invoke with the same persistent state."

**How to avoid:**
The WIT return type for continuation must be a discriminated union — `Continue { state: list<u8> }` / `Done { result: wasm-response }`. The engine re-invocation loop reads the `state` blob from the previous invocation's return value and passes it back as the next invocation's trigger input. The component never preserves execution state; it only serializes application-level state. Design the state format to be self-describing (include a version field and a step counter) so the component can reconstruct its progress from scratch on each entry.

**Warning signs:**
- Developer writes code that assumes local variables persist across `Continue` returns — this will always produce a reset state
- Component tries to use `wstd::io` or file system to "pause" across invocations — the file system is preopen-scoped per execution; a new store does not inherit open file handles
- Tests pass in a loop on the host side but fail when the engine re-invokes with a real new Wasmtime store

**Phase to address:**
Phase 1 (WIT interface + engine re-invocation loop) — the fundamental execution model must be clear before any state serialization work begins.

---

### Pitfall 2: Multi-Operator Divergence — LLM Temperature Breaks Consensus on Continuation Steps

**What goes wrong:**
In multi-operator deployments, all operators independently execute the same component. If the agent component uses an LLM call at any continuation step, each operator gets a different LLM response (temperature > 0). Operator A returns `Continue { state: A_state }` and operator B returns `Continue { state: B_state }`. The aggregator collects these, but `A_state != B_state`. The existing quorum logic compares `SubmissionRequest` payloads — if they differ, quorum is never reached and the workflow stalls permanently.

**Why it happens:**
The existing consensus model is designed for deterministic computation — each operator runs the same WASM, processes the same on-chain input, and should produce the same output. LLM inference is non-deterministic by design. The mismatch is architectural: consensus was built for "compute" not "reason." Developers who worked on single-operator deployments do not notice this until they test multi-operator.

**How to avoid:**
Two approaches, one must be chosen explicitly:

1. **Designate a lead operator for reasoning steps.** One operator does the LLM call; its `Continue` state is used as the canonical next state; other operators verify the state is valid (schema check, not LLM re-inference). This breaks the current symmetric execution model and requires protocol changes.
2. **Force temperature=0 for all continuation LLM calls.** Deterministic LLM output allows quorum to reach agreement. Most providers support this. Document it as a requirement for multi-operator agent services. Validate by running the same prompt twice with temp=0 and confirming identical outputs for the specific model/provider.

Option 2 is far simpler. Option 1 is needed if the agent requires creative reasoning. Default to option 2 and document the constraint clearly.

**Warning signs:**
- Multi-operator deployment stalls on the first continuation step; single-operator works fine
- Quorum queue (keyed by `(EventId, SubmitAction)`) shows entries from all operators but no quorum is reached — payloads differ
- Log inspection shows different LLM responses across operators for the same trigger

**Phase to address:**
Phase 1 (engine re-invocation loop) — the consensus strategy for continuation must be chosen before any multi-step state is persisted, because state format is not separable from the consensus approach.

---

### Pitfall 3: Synchronous `call-service` Host Function Blocks the Tokio Runtime Thread

**What goes wrong:**
The `call-service` host function is invoked synchronously from inside WASM (the component calls it as a regular WIT import). On the host side, executing a service call requires dispatching to the engine, running another Wasmtime component, and returning the result — all async operations. If the host function implementation calls `tokio::runtime::Handle::current().block_on(...)` to bridge sync-to-async, it blocks the Tokio worker thread currently executing the outer component. Under any load, this deadlocks: the outer engine is occupying a thread trying to call the inner engine, which needs a thread from the same pool.

**Why it happens:**
The WAVS engine runs component execution as async tasks: `ctx.rt.spawn(async move { ... })` in `engine.rs`. The Tokio runtime has a fixed thread pool. A blocking `block_on` inside an async task blocks that thread. If all threads are blocked waiting on each other, the runtime stalls. This is the classic sync-inside-async deadlock in Rust, amplified by WASM boundaries hiding the async context.

**How to avoid:**
The `call-service` host function must be registered via Wasmtime's `func_wrap_async` (async host function variant), not `func_wrap` (sync). In async mode, Wasmtime suspends the component via WASM epoch yields, allowing the Tokio runtime to execute the inner service call on a separate task. The outer component resumes when the inner call completes. This requires `Config::async_support(true)` which is already set in WAVS (async component execution is used throughout). Use `LinkerInstance::func_wrap_async` for the `call-service` host function implementation.

**Warning signs:**
- WAVS node stops processing all requests after the first `call-service` invocation — the runtime is deadlocked
- `jstack`/`tokio-console` shows all worker threads blocked in `block_on` waiting for another async task
- Reducing the Tokio thread pool to 1 (via `--worker-threads 1`) reliably deadlocks on the first inter-service call — this is a fast way to reproduce in tests

**Phase to address:**
Phase 2 (call-service host function) — get the async/sync boundary right before any service graph is tested.

---

### Pitfall 4: KV Store Scoped to Service ID — Continuation State and RPC Results Are Isolated by Wrong Boundary

**What goes wrong:**
The existing `KeyValueCtx` is namespaced by `service.id().to_string()` (see `wasm_engine.rs` line 161). All KV reads and writes inside a component are prefixed with this service ID. When service A calls service B via `call-service`, service B's execution context uses service B's ID — correct. But if service A tries to read state that service B wrote (expecting a shared KV namespace), it reads nothing, because A's KV namespace is `service_a:` and B's is `service_b:`. Developers who expect inter-service shared state via KV will be silently wrong.

**Why it happens:**
The KV isolation-by-service is a security and isolation feature, not a bug. But when building service-to-service workflows, developers often want to share intermediate results without encoding everything in the RPC response payload. The WAVS KV model does not support cross-service reads. Additionally, the continuation state stored by a component is keyed within its own service namespace — so re-invocations of service A always see service A's state, even if called by service B.

**How to avoid:**
Make the data model explicit in documentation and design: `call-service` is synchronous RPC, not shared memory. All inter-service data must be passed through the return value of `call-service`, not via KV side-channels. The continuation state for service A is stored under service A's KV namespace, keyed by the continuation chain ID (e.g., the triggering event ID). When designing the state format, never assume another service's KV is readable.

**Warning signs:**
- Service A reads a key immediately after calling service B and expects to see B's write — returns `None`
- Developer adds a `"shared:"` bucket prefix hoping to escape the namespace — the prefix is still applied on top of the service ID; there is no escape
- Tests that run A and B in the same Wasmtime store (e.g., a unit test with a shared `WavsDb`) pass but deployed multi-service tests fail — in deployment, each service has its own KV context

**Phase to address:**
Phase 2 (call-service host function) — establish the data-passing contract before any multi-service workflow is built.

---

### Pitfall 5: Continuation State Grows Across Steps — No Size Cap Means Eventual Payload Rejection

**What goes wrong:**
Each continuation step serializes all agent state (conversation history + tool results + step metadata) into the `Continue { state: list<u8> }` return value. The next invocation receives this blob as input. If each step appends to the state without trimming, the state blob grows with each step. At some point it exceeds `max_wasm_payload_size` (4 KB cap at the aggregator, `config.max_wasm_payload_size` in dispatcher). The engine rejects the continuation response with `ResponseSizeExceeded`. The agent is stuck: it cannot continue because it cannot serialize its state, and it cannot finish because it has not yet reached `Done`.

**Why it happens:**
The existing 4 KB cap (see `WasmResponseSizeError` and `validate_size` in `execute.rs`) was designed for final submission payloads, not intermediate continuation state. The assumption was that component outputs are small on-chain data (hashes, decisions, small structs). Agent conversation history + tool results can easily reach 50-200 KB after several reasoning steps. The cap was not revisited for the continuation use case.

**How to avoid:**
Two-part mitigation:

1. **KV-backed continuation state.** Do not pass the full state through the WIT return value. Instead, the component writes its state to KV (which has no size enforcement beyond available storage), then returns `Continue { state: <kv_key: bytes> }` — only the KV key, not the state blob itself. The engine re-invocation passes the KV key as input; the next step reads state from KV. The `Continue` payload stays tiny (< 64 bytes).

2. **Token budget enforcement at write time.** Apply the same token-budget trim logic from v2.0 (conversation history trimming) at each continuation step. The state that gets written to KV must not grow without bound.

**Warning signs:**
- Agent works for 2-3 steps then fails with `ResponseSizeExceeded` — the state crossed the 4 KB threshold
- Each continuation adds conversation history directly to the WIT return value rather than to KV
- The `Continue` state serialization does not include a size check before returning

**Phase to address:**
Phase 1 (WIT interface design) — the decision to use KV-backed state vs. inline state must be made before the WIT interface is finalized. Changing the interface later breaks all existing components.

---

### Pitfall 6: Infinite Continuation Loop — No Step Limit Means Runaway Agent Burns Resources

**What goes wrong:**
An agent component that returns `Continue` unconditionally (e.g., due to a bug in its termination condition, or an LLM that never decides it is "done") re-invokes indefinitely. Each step consumes fuel, epoch time, and KV writes. The engine has no visibility into how many continuation steps a given agent has taken. The operator node processes the agent forever. With multiple services, a runaway agent starves other service executions by holding Tokio tasks.

**Why it happens:**
The termination condition is in the component logic, which the engine trusts. The existing epoch timeout and fuel limit apply per step (each new invocation gets a fresh fuel budget and a fresh timeout). They do not apply across all steps of a continuation chain. An agent that returns `Continue` immediately (before doing any real work) can cycle through hundreds of steps within seconds, each step trivially completing within fuel/epoch limits.

**How to avoid:**
The engine's re-invocation loop must enforce a maximum step count per continuation chain. Store `(event_id, step_count)` in the engine's tracking state. If `step_count > MAX_CONTINUATION_STEPS` (default: 10, configurable per service in service.json), terminate the chain and emit an error. This is analogous to how the existing `QuorumQueue` TTL prevents stale aggregator queues from growing forever. Add `max_continuation_steps` to the service workflow config alongside `fuel_limit` and `time_limit_seconds`.

**Warning signs:**
- Activity feed shows a service producing events continuously with no submission
- KV write count for a service ID grows unboundedly within a short time window
- CPU usage on the WAVS node spikes after deploying an agent service with a buggy termination condition

**Phase to address:**
Phase 1 (engine re-invocation loop) — the step limit is a safety invariant. It must be built into the loop, not bolted on after.

---

### Pitfall 7: Re-instantiation Cost Per Continuation Step — LRU Cache Eviction Breaks Agent Latency

**What goes wrong:**
Each continuation step calls `load_component_from_source`, which checks the LRU cache (default size: 20 components). If multiple active agent services cycle through continuation steps concurrently, they can evict each other from the cache. Each eviction forces a re-parse and re-compile of the WASM component (expensive — hundreds of milliseconds for a complex component). A single agent with 10 continuation steps may tolerate this; 10 concurrent agents thrashing the LRU cannot. The symptom is unpredictable latency spikes with no error, invisible from the activity feed.

**Why it happens:**
The LRU cache is designed for the steady-state where a set of services runs periodically. For continuation mode, the same component re-executes sequentially — but if the LRU has been evicted by other services between steps, it must recompile. The `wasm_lru_size = 20` default was set for simple services, not for agents that chain 10+ re-invocations in rapid succession.

**How to avoid:**
For agent services in continuation mode, the engine should pin the component in memory for the duration of the continuation chain. This can be implemented with a simple `Arc<Component>` ref held in the continuation-chain tracking state (keyed by event ID). The component is not evicted from the LRU while a continuation chain is active. When the chain reaches `Done` or hits the step limit, the pin is released. This adds minimal memory overhead (a WASM component reference, not a full store).

**Warning signs:**
- Continuation steps 1-5 are fast; steps 6+ are slow with no code difference — cache eviction between steps
- `just start-jaeger` traces show `load_component_from_source` taking > 200 ms on later continuation steps when it was < 5 ms earlier
- Increasing `wasm_lru_size` in the config fixes the latency spike

**Phase to address:**
Phase 1 (engine re-invocation loop) — pin the component ref when entering the loop; unpin on exit.

---

### Pitfall 8: `call-service` Circular Dependency — Service A Calls Service B Calls Service A

**What goes wrong:**
Service A's `AllowedServiceCalls` permits calling service B. Service B's `AllowedServiceCalls` permits calling service A. A trigger on service A causes it to call B; B calls A; A calls B again; this cycles indefinitely. Unlike the continuation step limit (which is per-chain), this is a cycle across two different services with no natural termination. Neither service exceeds its step limit — they alternate. The cycle consumes Tokio tasks and KV writes continuously.

**Why it happens:**
`AllowedServiceCalls` is a whitelist, not a DAG. The permission system says "A may call B" but does not reason about whether a call graph is acyclic. This is the same class of problem as import cycles in module systems — individually valid imports that create a dependency cycle.

**How to avoid:**
The engine must track a call chain (list of service IDs currently in the call stack) and pass it through each `call-service` invocation context. Before executing a `call-service` call, check if the target service ID is already in the call chain. If yes, reject with `CircularServiceCall` error rather than executing. This is analogous to call stack overflow detection. The call chain can be passed as a hidden engine-level parameter, not exposed to the component. Maximum call depth (default: 5) also prevents non-circular but deeply nested calls.

**Warning signs:**
- Two services appear to be executing simultaneously in the activity feed, both consuming Tokio tasks, with no submission events
- Disabling `AllowedServiceCalls` for one of the two services breaks the cycle — confirms mutual dependency
- Tokio task count grows monotonically after deploying both services

**Phase to address:**
Phase 2 (call-service host function + permission enforcement) — cycle detection must be in the first implementation; retrofitting it later requires changing the host function signature.

---

### Pitfall 9: WIT Interface Versioning — Adding `Continue` Variant Breaks All Existing Operator Components

**What goes wrong:**
The current operator WIT interface returns `list<wasm-response>` from `call-run`. Adding a `Continue`/`Done` variant changes the return type to a discriminated union. This is a breaking WIT change. All existing operator components — echo, kv-store, aggregator examples, all demo AVS projects — were compiled against the old interface. They cannot be loaded by the new engine without recompilation. The engine cannot distinguish between a component compiled against the old interface and one compiled against the new interface without inspecting the component's WIT export.

**Why it happens:**
WIT does not have interface versioning in the semver sense. The package version (`wavs:operator@2.7.0`) is a semver string, but Wasmtime's component type checking is structural, not nominal. Changing the return type from `list<wasm-response>` to `variant { continue(state), done(list<wasm-response>) }` changes the exported function signature. Old components have the old signature. The linker will reject them when it attempts to instantiate against the new world.

**How to avoid:**
Two viable paths:

1. **New world, new version.** Bump to `wavs:operator@3.0.0` and define `WavsContinuationWorld` with the new return type. The engine linker can attempt the new world first, then fall back to the old world for components compiled against `@2.7.0`. This maintains backward compatibility but requires the engine to maintain two linkers.

2. **Additive wrapper.** Keep the `call-run` signature identical; add a separate exported function `call-run-continuation` with the new return type. The engine calls `call-run-continuation` if the component exports it; falls back to `call-run` for legacy components.

Path 2 is lower risk because it requires no change to the world definition for existing components and no dual-linker complexity.

**Warning signs:**
- Engine fails to instantiate existing demo components after the WIT change with `type mismatch` from Wasmtime
- `wasm-tools component wit` on an old component shows `@2.7.0` world; new engine expects `@3.0.0`
- The linker creation (in `instance.rs`) fails for legacy components — this surfaces as `EngineError::Instantiate`

**Phase to address:**
Phase 1 (WIT interface design) — the versioning strategy must be decided before the interface is published. Changing it after components are deployed is a migration operation, not a patch.

---

### Pitfall 10: `call-service` Permission Declared by Caller — Operator Cannot Audit the Full Call Graph

**What goes wrong:**
The design says `AllowedServiceCalls` is declared by the caller in their `service.json`. This means service A declares "I am allowed to call service B." Service B's operator has no way to restrict which services are allowed to call it — any service that declares the permission can call B. An operator running service B for one AVS team finds that another AVS team's service A is calling B with arbitrary inputs, consuming B's execution budget and generating submissions that count against B's quota.

**Why it happens:**
Caller-declared permissions follow the same pattern as `AllowedHostPermission` (caller declares what it can access). This is correct for network access (the operator knows what network the service needs). For service-to-service calls, it creates an asymmetry: the callee has no say. In the existing network policy, the operator can set `AllowedHostPermission::None` to block all outbound calls. The analogous callee-side protection for `call-service` does not exist in the v3.0 design.

**How to avoid:**
Add a corresponding `AllowedCallers` field to the target service's `service.json`. An empty or absent `AllowedCallers` means the service is callable by any service in the same node (the permissive default for MVP). A non-empty `AllowedCallers` whitelist restricts who can call the service. The engine enforces this at `call-service` invocation time by checking the caller's service ID against the target's `AllowedCallers`. This is a minimal change that prevents unintended cross-AVS service calls.

**Warning signs:**
- A service receives `call-service` invocations from an unexpected caller (visible in engine logs)
- KV writes for a service increase unexpectedly — it is being called by another service that has whitelisted it
- The WAVS node runs services from multiple AVS teams; one team's service behavior is affected by another team's call pattern

**Phase to address:**
Phase 2 (permission enforcement) — callee-side enforcement should be in the first implementation. Adding it later is a security fix, not a feature.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Pass full continuation state inline in WIT return value (not via KV) | Simpler engine code | Hits 4 KB payload cap after a few steps; must be refactored | Never for production agents; only for stateless echo-style continuation tests |
| No step limit on continuation chains | Faster initial implementation | Runaway agents burn resources indefinitely | Never; the step limit is a safety invariant |
| Caller-only permission (no `AllowedCallers`) | Fewer config fields | Cross-AVS service calls are unrestricted | MVP only; add callee-side enforcement before multi-tenant deployments |
| Synchronous host function for `call-service` via `block_on` | Easier to write than async host function | Deadlocks under any load on the shared Tokio thread pool | Never; the async host function variant is not significantly harder |
| Temperature > 0 for continuation LLM calls in multi-operator setup | Better reasoning quality | Quorum never reached; workflow stalls permanently | Only on single-operator deployments |
| No cycle detection in `call-service` | Simpler permission check | Circular call graphs deadlock or loop indefinitely | Never; cycle detection is O(depth) and trivial to add |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Engine re-invocation loop | Calling `execute_operator_component` recursively from within an async task | Spawn a new async task for each continuation step; do not nest executions in the same call chain |
| `call-service` host function | Using `func_wrap` (sync) with `block_on` inside | Use `func_wrap_async` (Wasmtime async host function) so the outer component can yield while the inner executes |
| Continuation state KV key naming | Using a static key like `"continuation_state"` (collides across concurrent invocations) | Key by event ID: `format!("continuation:{event_id}")` — each trigger chain gets its own namespace |
| WIT return type for continuation | Returning `list<u8>` opaque blob and hoping the engine interprets it | Define a proper WIT discriminated variant — `Continue { state: list<u8> }` / `Done { payload: wasm-response }` |
| Fuel budget for continuation | Using the same fuel limit as simple query components | Agent continuation steps require 10-50x more fuel per step than simple components; configure per-service |
| LRU component cache | Letting continuation steps evict each other from the LRU cache | Pin the component `Arc` for the duration of an active continuation chain |
| `AllowedServiceCalls` | Listing target service IDs without a corresponding `AllowedCallers` on the callee | Add `AllowedCallers` to callee `service.json` to establish bilateral permission |
| Cross-service state | Reading a KV key from another service's namespace | All inter-service data must be passed through `call-service` return values; KV namespaces are per-service and not shared |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Re-compiling WASM between continuation steps (cache eviction) | Steps 1-N fast; later steps slow with no code change | Pin component `Arc` for the active continuation chain | When LRU has fewer slots than concurrently running agents |
| Unbounded continuation state in KV | State grows with each step; eventually exceeds practical read size | Apply token-budget trimming at each continuation step | At ~5-10 steps for agents that accumulate full conversation history |
| Tokio task starvation from blocking `call-service` | All services stop processing after first inter-service call | Use `func_wrap_async` for the `call-service` host function | On the first inter-service call under any concurrent load |
| Full step re-invocation overhead for tiny decision steps | 100ms overhead per step even for a simple "check condition" step | Allow agents to batch multiple sub-steps within a single continuation step before returning `Continue` | At > 20 continuation steps where most steps are fast conditional checks |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| No `AllowedCallers` enforcement on callee | Any service on the node can call any other service | Add bilateral permission: caller whitelist AND callee allowlist |
| Continuation state contains raw trigger data unsanitized | State persisted to KV contains potentially adversarial data that gets re-fed to LLM on next step | Sanitize and structure trigger data before adding to continuation state |
| Circular `call-service` graphs | Infinite execution loop; resource exhaustion | Cycle detection via call chain tracking; enforced at host function level |
| Step count not persisted | Component can reset its step counter via malicious state serialization | Engine tracks step count independently; do not trust the component-reported step count |
| `call-service` result returned directly to LLM as trusted content | A compromised callee can inject instructions into the LLM's reasoning context | Treat `call-service` responses as untrusted data; validate schema before including in LLM context |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Activity feed shows no events between continuation steps | Operator cannot monitor agent progress; appears hung | Emit a `ContinuationStep` event to the activity feed at each step with step number and state summary |
| `OutOfFuel` on continuation step N with no context on which step | Developer cannot tell if the fuel limit is too low for all steps or just step N | Surface step number and fuel consumed per step in the engine error; log it in the existing tracing spans |
| Quorum stall from LLM non-determinism looks identical to network failure | Developer debugs the wrong thing | Surface "operators submitted different payloads" distinctly from "operators did not submit" |
| `CircularServiceCall` error message does not show the cycle | Developer must manually inspect logs to find the loop | Include the full call chain in the error: `A -> B -> A` |

## "Looks Done But Isn't" Checklist

- [ ] **Continuation state is KV-backed:** The `Continue` WIT return value carries a KV key, not the full state blob — verify by inspecting the serialized return value size is < 64 bytes
- [ ] **Step limit enforced at engine level:** Deploy an agent component that returns `Continue` unconditionally and confirm the engine terminates it at `MAX_CONTINUATION_STEPS` with an error event
- [ ] **Multi-operator consensus works at temperature=0:** Run a continuation agent on a 2-operator testnet with temp=0; confirm both operators reach the same `Done` payload and quorum is achieved
- [ ] **`call-service` uses async host function:** Confirm `func_wrap_async` (not `func_wrap`) in the linker setup for `call-service`; run 2 concurrent inter-service calls and verify neither deadlocks
- [ ] **Cycle detection rejects A→B→A:** Deploy services A and B with mutual `AllowedServiceCalls`; trigger A and confirm `CircularServiceCall` error before infinite loop
- [ ] **`AllowedCallers` enforcement rejects unauthorized caller:** Deploy service B with `AllowedCallers: [service_c]`; have service A attempt to call B; confirm rejection
- [ ] **KV namespace isolation confirmed:** Have service A call service B; have A attempt to read a key written by B; confirm `None` return (isolation working correctly)
- [ ] **LRU pin works across steps:** Run a 10-step agent while 19 other services are active (filling the LRU); confirm all steps complete at uniform latency without cache-miss spikes
- [ ] **WIT backward compatibility:** Load a component compiled against `wavs:operator@2.7.0` on the new engine; confirm it executes normally via the legacy world path

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Runaway continuation loop | LOW | Deactivate the service via the HTTP API; delete the continuation KV keys for the service namespace; fix the termination condition; redeploy |
| Continuation state poisoned (corrupted KV) | LOW | Delete the KV key for the affected event ID (`continuation:{event_id}`); next trigger starts a fresh chain |
| Deadlocked Tokio runtime from sync `call-service` | HIGH | Restart the WAVS node; fix host function to use `func_wrap_async` before redeploying; cannot recover without restart |
| Quorum stall from LLM non-determinism | MEDIUM | Switch to temperature=0 for the agent; redeploy service definition; existing stalled quorum queues will TTL out (default 48h) |
| Circular call graph deployed | LOW | Update `AllowedServiceCalls` to remove the circular permission on one service; redeploy service definition |
| WIT interface break for legacy components | HIGH | Maintain old world path in the engine linker; recompile affected components against new WIT when cycle is ready |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Re-instantiation model misunderstood | Phase 1 (WIT + engine loop) | Unit test: fresh store on each step; no variables persist |
| LLM non-determinism breaks multi-operator consensus | Phase 1 (engine loop design) | 2-operator integration test with temp=0; both operators agree on all steps |
| Sync `call-service` deadlocks Tokio | Phase 2 (host function) | `func_wrap_async` used; 2-concurrent-call test passes without deadlock |
| KV namespace isolation misunderstood | Phase 2 (call-service + docs) | Cross-service KV read returns `None`; documented in service design guide |
| Continuation state exceeds payload cap | Phase 1 (WIT interface design) | KV-backed state chosen; `Continue` payload verified < 64 bytes |
| No step limit — runaway agents | Phase 1 (engine loop) | Engine terminates unconditional-Continue agent at `MAX_CONTINUATION_STEPS` |
| LRU eviction between steps | Phase 1 (engine loop) | Pin component `Arc` per active chain; latency uniform across 10-step agent |
| Circular `call-service` graph | Phase 2 (host function) | Cycle detection rejects A→B→A; error includes call chain |
| WIT versioning breaks legacy components | Phase 1 (WIT interface) | Legacy component loads on new engine via fallback world |
| No callee-side permission enforcement | Phase 2 (permission enforcement) | `AllowedCallers` rejects unauthorized callers; confirmed in integration test |

## Sources

- Direct code inspection: `/workspace/WAVS/packages/wavs/src/subsystems/engine.rs` — `ExecuteOperator` dispatches as separate async tasks; each is independent
- Direct code inspection: `/workspace/WAVS/packages/engine/src/worlds/operator/execute.rs` — `call_run` is called on a fresh store per invocation; execution context is not preserved
- Direct code inspection: `/workspace/WAVS/packages/engine/src/worlds/instance.rs` — `configure_store` creates fuel + epoch from scratch per invocation; `configure_linker` creates a new `Linker<T>` per invocation
- Direct code inspection: `/workspace/WAVS/packages/engine/src/backend/wasi_keyvalue/context.rs` — `KeyValueCtx::new(db, service.id().to_string())` — per-service namespace enforced at construction time
- Direct code inspection: `/workspace/WAVS/packages/wavs/src/subsystems/engine/wasm_engine.rs` line 161 — `KeyValueCtx::new(self.engine.db.clone(), service.id().to_string())` — confirms KV isolation scope
- Direct code inspection: `/workspace/WAVS/packages/utils/src/storage/db.rs` — `WavsDb::kv_store` is a `DashMap` (in-memory, not persisted); `QuorumQueue` has TTL cleanup
- Direct code inspection: `/workspace/WAVS/packages/wavs/src/dispatcher.rs` — `DispatcherCommand` enum; no existing continuation or inter-service RPC variants
- Direct code inspection: `/workspace/WAVS/wit-definitions/operator/wit/*.wit` — `call-run` returns `result<list<wasm-response>, string>`; changing to a variant is a breaking WIT change
- Direct code inspection: `/workspace/WAVS/.planning/PROJECT.md` — v3.0 scope: `Continue`/`Done` variants, `call-service` host function, `AllowedServiceCalls`, engine re-invocation loop
- Wasmtime async host functions: `LinkerInstance::func_wrap_async` required when `Config::async_support(true)`; sync `func_wrap` with `block_on` inside deadlocks under load — [docs.wasmtime.dev/api/wasmtime/component/struct.LinkerInstance.html](https://docs.wasmtime.dev/api/wasmtime/component/struct.LinkerInstance.html)
- Wasmtime issue #9600: Reentrant WASM component calls — confirmed that component reentrancy requires careful store management, not stack suspension — [github.com/bytecodealliance/wasmtime/issues/9600](https://github.com/bytecodealliance/wasmtime/issues/9600)
- WAVS ASYNC_NOTES.md: WASI 0.2 has no native stack suspension; WASI 0.3 adds async in the ABI. WAVS is on p2. Continuation must be implemented at application level.
- Multi-operator quorum: `QuorumQueue` keys by `(EventId, SubmitAction)`; different LLM responses produce different `SubmitAction` payloads; quorum never reached — observed pattern from distributed compute systems with non-deterministic workers

---
*Pitfalls research for: Agent continuation mode and service-to-service RPC in WASI/Wasmtime runtime*
*Researched: 2026-04-20*
