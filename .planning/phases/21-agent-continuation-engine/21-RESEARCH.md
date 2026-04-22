# Phase 21: Agent Continuation Engine - Research

**Researched:** 2026-04-22
**Domain:** Wasmtime WASM execution engine — agent continuation loop, KV state persistence, LRU pinning
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal,
success criteria, and codebase conventions to guide decisions.

### Claude's Discretion
All implementation choices (loop placement, KV key format, detection mechanism, error type) are Claude's to decide.

### Deferred Ideas (OUT OF SCOPE)
None.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CONT-01 | Engine re-invocation loop in `run_trigger` — calls `execute_operator_step()`, checks Continue/Done, repeats until Done or max steps | `execute()` in `packages/engine/src/worlds/operator/execute.rs` must gain a loop that calls `call_run_agent()` and branches on `StepResult` |
| CONT-02 | Auto-persist agent state to KV between steps using `continuation:<service_id>:<correlation_id>:step:N` key pattern | Host writes directly to `db.kv_store.insert()` via `WavsDb` — no component bucket needed; correlation_id derived from `EventId::new()` hex |
| CONT-03 | Step limit enforcement — engine terminates agent with clear error when `max_continuation_steps` exceeded | Read `component.max_continuation_steps.unwrap_or(10)` from `Workflow.component`; add `EngineError::ContinuationLimit` variant |
| CONT-04 | Developer-defined multi-step workflows — named step sequences with explicit `continue("step_name")` handoffs | The `string` field in `StepResult::Continue(string)` IS the step name; agent reads it back from KV state on re-invocation |
| CONT-05 | Component LRU pinning between continuation steps — compiled module stays cached across re-invocations | Hold `WasmComponent` clone before loop (already Arc-backed); call `cache.get(&digest)` to promote-to-MRU before each step |
</phase_requirements>

---

## Summary

Phase 21 adds the agent continuation loop to the WAVS execution engine. When a WASM component exports the `run-agent` function (via the `agent` named interface added in Phase 20), the engine repeatedly re-invokes it until it returns `Done(responses)` or the step limit is exceeded. Between steps, the agent's continuation token (a step name string from the `Continue(string)` variant) is persisted to the KV store so the component can read it back on re-invocation and route to the correct handler.

The implementation is entirely inside `packages/engine/src/worlds/operator/execute.rs` (the `execute()` function) and `packages/engine/src/utils/error.rs` (new error variant). The high-level architecture is:
1. **Detect**: Check if the WASM component exports the `agent` interface before invoking
2. **Loop**: Call `world.wavs_operator_agent().call_run_agent()`, inspect `StepResult`
3. **Persist**: Write continuation token to `db.kv_store` under a key derived from `EventId`
4. **Limit**: Count steps, terminate with `ContinuationLimit` error if exceeded
5. **Pin**: Hold the `WasmComponent` clone (already Arc-backed) for the entire loop to prevent LRU eviction

The changes do not touch `packages/wavs/src/subsystems/engine.rs` or `wasm_engine.rs` because `execute()` returns `Vec<WasmResponse>` — the loop is internal.

**Primary recommendation:** Implement the loop inside `execute()` in `execute.rs`, using `component.component_type().exports(&engine)` to probe for the `agent` interface before instantiation, falling back to legacy `call_run` for non-agent components.

---

## Standard Stack

### Core (already present — no new dependencies needed)

| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| `wasmtime` | 42.0.1 | WASM execution, bindgen, `WavsWorld`, `call_run_agent` | Already in workspace |
| `lru` | 0.16.3 (workspace) | LRU cache for compiled components | `cache.get(&digest)` promotes to MRU |
| `utils::storage::db::WavsDb` | internal | KV persistence for continuation state | `db.kv_store.insert(key, bytes)` |
| `wavs_types::EventId` | internal | Deterministic correlation ID from trigger | `EventId::new(&service_id, &workflow_id, salt).to_string()` → 40-char hex |

**Installation:** No new dependencies required. All needed types are already in the workspace.

---

## Architecture Patterns

### Recommended Project Structure (changes)

```
packages/engine/src/worlds/operator/
├── execute.rs       ← MODIFIED: add agent detection + continuation loop
├── component.rs     ← unchanged
└── mod.rs           ← unchanged

packages/engine/src/utils/
└── error.rs         ← MODIFIED: add ContinuationLimit error variant
```

### Pattern 1: Agent Export Detection via component_type()

**What:** Inspect component's exported type tree before instantiation to determine if it exports the `agent` named interface.

**When to use:** Before calling `WavsWorld::instantiate_async` — avoids paying instantiation cost for legacy-path check and avoids confusing failure modes.

**The export name:** When WIT has `export agent;` where `agent` is a named interface in `wavs:operator@2.7.0`, the fully qualified export name visible in `component.component_type().exports(engine)` is `"wavs:operator/agent@2.7.0"` (or similar qualified name). Alternatively, any export whose name contains `"agent"` or that is a `ComponentItem::ComponentInstance` with the right name.

**Example:**
```rust
// Source: packages/wit-schema/src/traverse.rs (existing pattern in codebase)
use wasmtime::component::types::ComponentItem;

fn has_agent_export(component: &wasmtime::component::Component, engine: &wasmtime::Engine) -> bool {
    let component_type = component.component_type();
    for (name, _item) in component_type.exports(engine) {
        // The exact name will be "wavs:operator/agent@2.7.0" for a named interface export
        if name.contains("agent") {
            return true;
        }
    }
    false
}
```

**Caution:** [ASSUMED] The exact export name string for a named interface `export agent;` in wasmtime 42 component model. Verify by printing `component_type.exports(engine)` for an agent component. The pattern from wit-schema traverse.rs confirms `component_type.exports()` returns `(name, item)` pairs where named interface exports are `ComponentItem::ComponentInstance`.

### Pattern 2: Continuation Loop Structure

**What:** The re-invocation loop wraps the existing single `call_run` call, replacing it with an agent-aware loop.

**When to use:** When `has_agent_export()` returns true.

**Example:**
```rust
// Source: packages/engine/src/worlds/operator/execute.rs (new logic)
// The component is held as a clone — this is the "LRU pin": holding the Arc prevents
// it from being evicted even if the cache capacity is exceeded by concurrent activity.
// Additionally call cache.get(&digest) before each step to promote it to MRU.

let max_steps = workflow.component.max_continuation_steps.unwrap_or(10) as usize;
let mut step = 0usize;
let mut final_responses: Vec<WasmResponse> = Vec::new();

loop {
    if step >= max_steps {
        return Err(EngineError::ContinuationLimit {
            service_id: service_id.clone(),
            workflow_id: workflow_id.clone(),
            steps: max_steps,
        });
    }

    let world = WavsWorld::instantiate_async(
        deps.store.as_operator_mut(),
        &deps.component,  // clone held since before loop
        deps.linker.as_operator_ref(),
    )
    .await
    .map_err(|e| EngineError::Instantiate(e.into()))?;

    let step_result = world
        .wavs_operator_agent()
        .call_run_agent(deps.store.as_operator_mut(), &input)
        .await
        .map_err(|e| EngineError::ComponentError(e.into()))?
        .map_err(EngineError::ExecResult)?;

    match step_result {
        StepResult::Done(responses) => {
            final_responses = responses.into_iter().map(|r| r.into()).collect();
            break;
        }
        StepResult::Continue(step_name) => {
            // Persist step_name to KV so component can read it on next invocation
            let kv_key = format!(
                "continuation/{}/{}:step:{}",
                service_id, correlation_id_hex, step
            );
            db.kv_store.insert(kv_key, step_name.into_bytes()).ok();
            step += 1;
            // Rebuild InstanceDeps for next iteration (new Store required per step)
            // deps = rebuild_deps(...)
        }
    }
}

Ok(final_responses)
```

### Pattern 3: KV Key Format

**What:** Deterministic key under which continuation token is stored between steps.

**The locked format (from STATE.md):** `wavs_agent_step:` prefix.

**The REQUIREMENTS.md format:** `continuation:<service_id>:<correlation_id>:step:N`

**Reconciliation:** STATE.md is the locked decision source. Use prefix `wavs_agent_step:`. The full key stored in `db.kv_store` (which uses flat string keys) should be:

```
wavs_agent_step:{service_id}:{correlation_id}:step:{N}
```

where `correlation_id` = `EventId::new(&service_id, &workflow_id, EventIdSalt::Trigger(&trigger_data)).to_string()` (40-char hex string).

**Note on KV namespacing:** The wasi-keyvalue component API has `namespace/bucket_id/key` layering, but that's only for component-accessed KV. The host-side `db.kv_store.insert()` uses the flat key directly. No namespace prefix is added automatically. The component can read it back by opening bucket `"wavs_agent_step"` and key `"{service_id}:{correlation_id}:step:{N}"` (since the component KV layer prepends `{namespace}/{bucket_id}/` which equals `{service_id}/wavs_agent_step/`). This means the host-written key must match the component-read key path.

**Recommended approach:** Write from the host at the fully namespaced path to be readable by component:
```rust
// Host writes:
let kv_key = format!("{service_id}/wavs_agent_step/{service_id}:{correlation_id}:step:{step}");
db.kv_store.insert(kv_key, step_name.into_bytes()).unwrap();

// Component reads via wasi:keyvalue:
// bucket = bucket.open("wavs_agent_step")
// bucket.get("{service_id}:{correlation_id}:step:{step_number}")
```

[ASSUMED] This interpretation of the KV namespacing. Verify by checking how `KeyValueCtx` computes the full key vs what `bucket.get(key)` returns. See `packages/engine/src/backend/wasi_keyvalue/bucket_keys.rs`.

### Pattern 4: Store Rebuild Between Steps

**What:** Each continuation step requires a fresh `wasmtime::Store` because a store cannot be reused after a WASM call completes (the WASM instance is consumed). The `InstanceDeps` must be rebuilt for each step.

**How:** The `InstanceDepsBuilder::build()` is the standard mechanism. Key: preserve the same `WasmComponent` clone (to avoid re-compilation and to "pin" the LRU cache entry) by passing the already-loaded component into the builder.

**Caution:** The current `execute.rs` function signature takes `&mut InstanceDeps` which holds a `Store`. After the first `call_run_agent`, the store is consumed-in-place. The loop needs to rebuild InstanceDeps each iteration OR the loop must be restructured to hold the component separately and rebuild the store. The component itself is cloneable (Arc-backed), so `deps.component.clone()` is cheap.

**Recommended:** Extract the component clone before the loop:
```rust
let component = deps.component.clone(); // Arc clone — cheap, prevents LRU eviction
// Then rebuild Store/InstanceDeps on each continuation step
```

### Pattern 5: LRU Pinning

**What:** The `BaseEngine.memory_cache` is a `Mutex<LruCache<ComponentDigest, WasmComponent>>`. `WasmComponent` is `Clone` (Arc-backed internally). Holding a clone of the component outside the LRU cache prevents the underlying compiled module from being dropped even if the LRU evicts the cache entry.

**Implementation:**
1. The `execute()` function already receives a `WasmComponent` via `InstanceDeps.component`
2. Before the continuation loop, clone it: `let _pin = deps.component.clone();`
3. The `_pin` variable holds an Arc reference to the compiled module for the loop's lifetime
4. Additionally, before each step, call `engine.memory_cache.lock().unwrap().get(&digest)` to promote it to MRU position (prevents eviction even if more components load)
5. For simplicity: just holding the clone is sufficient to prevent the compiled bytes from being dropped

**Note:** The `execute()` function in `packages/engine/src/worlds/operator/execute.rs` receives `InstanceDeps` which contains `.component: wasmtime::component::Component`. This is already the compiled component object — it's passed by the caller (`execute_operator_component`) which loads it from the cache. The current `execute()` does NOT have access to the `BaseEngine` cache mutex. The simplest LRU pin is to hold the `deps.component.clone()` — the cache eviction only removes the entry from the LRU map, but the Arc refcount keeps the compiled module alive.

### Anti-Patterns to Avoid

- **Inline state in Continue payload:** The `%continue(string)` carries ONLY a step name string, NOT the agent's full state. State must be stored in KV. The 4KB WIT string limit makes inline state fragile.
- **Reusing the Store across steps:** Each WASM invocation consumes the Store's state. Must rebuild `InstanceDeps` (new Store) for each step.
- **Skipping step limit on first step:** Step 0 still counts against `max_continuation_steps`. The limit is total calls, not continuation calls.
- **Using `call_run` on agent components:** Agent components implement `run-agent`, not `run`. The engine should detect and route correctly. Calling `call_run` on a component that only exports `run-agent` will fail at instantiation (WavsWorldIndices::new fails if `run` is missing too — since both are required by wavs-world).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Event correlation ID | UUID generator | `EventId::new(&svc, &wf, EventIdSalt::Trigger(&data))` | Deterministic from trigger — same across all operators for consensus |
| KV persistence | Custom DB | `db.kv_store.insert(key, value)` on `WavsDb` | Already wired into engine, WASI-accessible by component |
| Component export detection | String parsing of WIT | `component.component_type().exports(engine)` | Official wasmtime API, see `packages/wit-schema/src/traverse.rs` |
| Step fuel limit | Custom counter | Existing `max_wasm_fuel` in Store config | Wasmtime's fuel mechanism handles per-step execution cost |

---

## Common Pitfalls

### Pitfall 1: WavsWorld Instantiation Fails for Legacy Components

**What goes wrong:** Calling `WavsWorld::instantiate_async` on a legacy component (one that only exports `run`, not `agent`) causes `WavsWorldIndices::new` to fail internally because `export agent;` is unconditionally required in `wavs-world`.

**Why it happens:** The WIT world `wavs-world` declares both `export run` and `export agent` as required. Legacy components compiled against the old WIT don't have the `agent` export.

**How to avoid:** Detect agent capability BEFORE attempting WavsWorld instantiation. Use `component.component_type().exports(engine)` to scan for the agent interface export. If absent, fall back to a path that only calls `call_run` — which means using `WavsWorld::instantiate_async` still works IF the WIT world treats `export agent` as optional, OR creating a separate legacy path.

**Mitigation strategy:**
- Option A: Probe via `component_type().exports()` and use try-catch on `WavsWorldPre::new`
- Option B: Add a separate `wavs-world-legacy` WIT world that only has `export run` and attempt to use it as fallback
- Option C (simplest): Use `WavsWorldPre::new` in a `try` block — if it fails, the component is legacy; re-instantiate with only `call_run`

**Warning signs:** `EngineError::Instantiate` on existing (non-agent) components after Phase 21 ships.

**Critical question for planner:** Does `export agent;` being in the wavs-world mean existing components MUST implement it, or does wasmtime allow partial worlds? If full enforcement, Phase 21 must add a legacy fallback path or existing tests will break.

### Pitfall 2: Store Cannot Be Reused After WASM Call

**What goes wrong:** Attempting to call `call_run_agent` a second time on the same `WavsWorld` instance (or with the same Store) after the first call returns.

**Why it happens:** Wasmtime's component model: a `Store` is stateful. After a component call completes, you need a fresh Store (and re-instantiation) for the next call. The existing aggregator execute.rs already shows this — each function creates a new `AggregatorWorld::instantiate_async`.

**How to avoid:** Rebuild `InstanceDeps` (new Store) for each continuation step. Hold `WasmComponent` clone separately before the loop.

### Pitfall 3: KV Key Collision Between Services

**What goes wrong:** Two different services with the same trigger data type produce the same EventId → same KV key → state corruption.

**Why it happens:** `EventId` includes `service_id` in the hash, but if the hash has a collision (unlikely but possible with short 20-byte hash space) or if someone uses `TriggerData::Raw(vec![])` for both.

**How to avoid:** Include service_id explicitly in the key string, even though it's in EventId. Pattern: `wavs_agent_step:{service_id}:{event_id_hex}:step:{N}` has service_id twice, but that's intentional redundancy for readability and collision resistance.

### Pitfall 4: Missing Fuel Reset Between Steps

**What goes wrong:** Each continuation step re-instantiates with the same `fuel_limit`. If the fuel counter is not reset for each new Store, the second step starts with whatever fuel the first step left over.

**Why it happens:** `configure_store` sets fuel when building InstanceDeps. If InstanceDeps is rebuilt correctly, this is not an issue — each new Store starts fresh.

**How to avoid:** Rebuild full InstanceDeps (via `InstanceDepsBuilder::build()`) for each step, which calls `configure_store` with `fuel_limit` fresh.

### Pitfall 5: Timeout Does Not Reset Per Step

**What goes wrong:** The `tokio::time::timeout(Duration::from_secs(deps.time_limit_seconds), ...)` in execute.rs wraps the ENTIRE operation. With a continuation loop, each step consumes from this single timeout budget.

**How to handle:** Two valid designs:
- **Per-step timeout (recommended):** Wrap each step invocation separately — each step gets the full `time_limit_seconds`
- **Total timeout:** Wrap the entire loop — time is shared across all steps

The per-step timeout is more developer-friendly (consistent behavior per step). The planner should choose one and document it.

---

## Code Examples

### 1. Agent Export Detection

```rust
// Source: packages/wit-schema/src/traverse.rs (existing pattern)
use wasmtime::component::types::ComponentItem;

fn has_agent_export(
    component: &wasmtime::component::Component,
    engine: &wasmtime::Engine,
) -> bool {
    let component_type = component.component_type();
    for (name, item) in component_type.exports(engine) {
        match item {
            ComponentItem::ComponentInstance(_) if name.contains("agent") => return true,
            _ => {}
        }
    }
    false
}
```

### 2. Accessing Agent After Instantiation

```rust
// Source: WavsWorld docs (method name confirmed: wavs_operator_agent)
// WavsWorld has: pub fn wavs_operator_agent(&self) -> &Guest
// Guest has: pub async fn call_run_agent<S: AsContextMut>(&self, store: S, arg0: &TriggerAction)
//            -> Result<Result<StepResult, String>>

let world = WavsWorld::instantiate_async(store, &component, linker).await?;
let step_result: Result<StepResult, String> = world
    .wavs_operator_agent()
    .call_run_agent(store, &input)
    .await?;
```

### 3. StepResult Variant Matching

```rust
// Source: operator.wit Phase 20 addition
// variant step-result { done(list<wasm-response>), %continue(string) }
// bindgen generates: enum StepResult { Done(Vec<WasmResponse>), Continue(String) }

use crate::bindings::operator::world::wavs::operator::output::StepResult;

match step_result {
    StepResult::Done(responses) => {
        // convert and return
    }
    StepResult::Continue(step_name) => {
        // persist step_name to KV, increment counter
    }
}
```

### 4. New EngineError Variant

```rust
// Source: packages/engine/src/utils/error.rs (to be added)
#[error("ContinuationLimit: exceeded {steps} steps for service: {service_id}, workflow: {workflow_id}")]
ContinuationLimit {
    service_id: ServiceId,
    workflow_id: WorkflowId,
    steps: usize,
},
```

### 5. Reading max_continuation_steps from Component Config

```rust
// Source: packages/types/src/service.rs Phase 20 addition
// Component has: pub max_continuation_steps: Option<u32>
// Convention: unwrap_or(10) — matches WIT-05 requirement

let workflow = service.workflows.get(&workflow_id)?;
let max_steps = workflow.component.max_continuation_steps.unwrap_or(10) as usize;
```

### 6. Computing Correlation ID

```rust
// Source: packages/types/src/signing.rs (existing EventId)
use wavs_types::{EventId, EventIdSalt};

let correlation_id = EventId::new(
    &service_id,
    &workflow_id,
    EventIdSalt::Trigger(&trigger_action.data),
)
.map(|id| id.to_string())   // 40-char hex
.unwrap_or_else(|_| "unknown".to_string());
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single-step `call_run` | Multi-step `call_run_agent` loop | Phase 21 | Agent components can persist state across multiple LLM calls |
| No step limit | `max_continuation_steps` config field | Phase 20+21 | Runaway agents terminate with clear error |
| No KV agent state | Host writes `wavs_agent_step:` KV entries | Phase 21 | Agent conversation history is persistent and inspectable |

**Deprecated/outdated:**
- `call_run` is NOT deprecated — it remains the path for non-agent components. Both paths coexist.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The agent export name from `component.component_type().exports(engine)` contains the substring `"agent"` (e.g. `"wavs:operator/agent@2.7.0"`) | Architecture Pattern 1 | If wrong: `has_agent_export()` never returns true; all components take legacy path |
| A2 | Holding a `WasmComponent` clone outside the LRU cache prevents the compiled module from being freed (Arc semantics) | Pattern 5, CONT-05 | If wrong: need to explicitly prevent cache eviction via a separate pin map |
| A3 | KV host-side write at key `"{svc_id}/wavs_agent_step/{step_key}"` is readable by component via `bucket.open("wavs_agent_step").get("{step_key}")` | Pattern 3 | If wrong: step name is written but unreadable by component — need a different key format |
| A4 | `WavsWorld::instantiate_async` fails for legacy components that don't export `agent` (since `export agent;` is unconditional in wavs-world) | Pitfall 1 | If wrong: no detection issue — but if wasmtime silently allows missing optional exports, then detection approach can be simplified |
| A5 | `StepResult::Done` and `StepResult::Continue` are the exact enum variant names generated by wasmtime bindgen for `done(...)` and `%continue(...)` | Pattern 3 | If wrong: compilation error when pattern-matching; check `wavs_engine::bindings::operator::world::wavs::operator::output::StepResult` |

---

## Open Questions

1. **Does `export agent;` in wavs-world break existing (legacy) component loading?**
   - What we know: `WavsWorldIndices::new` "may fail if the component does not have the required exports" — and `export agent;` is unconditional in the current WIT
   - What's unclear: Whether wasmtime's component model enforces this strictly or allows partial worlds
   - Recommendation: The plan MUST include a test that loads a legacy component (e.g., `echo` example) after Phase 21 changes to verify it still works. If it breaks, add a fallback path.

2. **Should the continuation loop rebuild InstanceDeps OR reset the Store in-place?**
   - What we know: Wasmtime Stores are stateful; the existing pattern creates a new Store per invocation via `InstanceDepsBuilder::build()`
   - What's unclear: Whether `Store` can be reset or if full `InstanceDeps` rebuild is required
   - Recommendation: Rebuild full `InstanceDeps` for each step — matches existing patterns, no risk of state leakage

3. **Per-step timeout or total timeout for continuation loop?**
   - What we know: Current `execute()` wraps single call in `tokio::time::timeout(time_limit_seconds)`
   - What's unclear: Whether `time_limit_seconds` should apply per-step or to the whole chain
   - Recommendation: Per-step timeout is more predictable and consistent with non-agent behavior

4. **Exact KV key path for host-written state**
   - What we know: `db.kv_store.insert(key, value)` writes a flat string key; component reads via `namespace/bucket_id/key` namespacing
   - What's unclear: The exact key format needed so that a component calling `bucket.open("wavs_agent_step").get("...")` reads the host-written value
   - Recommendation: Write a unit test that writes a key from host then reads it via the component KV API path to confirm format

---

## Environment Availability

Step 2.6: SKIPPED (no external dependencies — pure Rust code changes within existing engine crate)

---

## Sources

### Primary (HIGH confidence — verified from codebase)

- `packages/engine/src/worlds/operator/execute.rs` — current execute() function, `call_run` path
- `packages/engine/src/utils/error.rs` — existing `EngineError` enum
- `packages/engine/src/common/base_engine.rs` — `LruCache<ComponentDigest, WasmComponent>` implementation
- `packages/engine/src/backend/wasi_keyvalue/context.rs` — `KeyValueCtx`, `KeyValueCtxProvider`
- `packages/engine/src/backend/wasi_keyvalue/bucket_keys.rs` — KV key format: `{namespace}/{bucket_id}/{key}`
- `packages/engine/src/backend/wasi_keyvalue/store.rs` — `db.kv_store.insert()` host-side KV write
- `packages/engine/src/worlds/instance.rs` — `InstanceDepsBuilder`, `InstanceDeps`, `configure_store`
- `packages/engine/src/bindings/operator/host.rs` — `call_service` stub; host implementation pattern
- `packages/wit-schema/src/traverse.rs` — `component.component_type().exports(engine)` pattern
- `wit-definitions/operator/wit/operator.wit` — WIT definitions including `step-result` variant, `agent` interface
- `packages/types/src/service.rs` — `Component.max_continuation_steps: Option<u32>` (Phase 20)
- `packages/types/src/signing.rs` — `EventId`, `to_string()` = 40-char hex
- `target/doc/wavs_engine/bindings/operator/world/struct.WavsWorld.html` — `wavs_operator_agent()` method, `call_run` method
- `target/doc/wavs_engine/bindings/operator/world/exports/wavs/operator/agent/struct.Guest.html` — `call_run_agent` method
- `target/doc/wavs_engine/bindings/operator/world/struct.WavsWorldIndices.html` — "may fail if required exports missing"
- `.planning/STATE.md` — Locked decision: KV prefix `wavs_agent_step:`

### Secondary (MEDIUM confidence)

- `target/doc/wavs_engine/bindings/operator/world/sidebar-items.js` — confirms `StepResult` type alias at world level [VERIFIED: generated docs]
- `target/doc/wavs_engine/bindings/operator/world/exports/wavs/operator/agent/sidebar-items.js` — confirms `Guest` and `GuestIndices` structs in agent exports module [VERIFIED: generated docs]

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all codebase-verified, no new deps
- Architecture: HIGH (core patterns) / MEDIUM (KV key namespace path, agent export string name)
- Pitfalls: HIGH — confirmed from wasmtime bindgen behavior in docs

**Research date:** 2026-04-22
**Valid until:** Stable — all findings based on current codebase state, not external ecosystem
