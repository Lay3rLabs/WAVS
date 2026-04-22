# Phase 22: Service-to-Service RPC - Research

**Researched:** 2026-04-22
**Domain:** Wasmtime async host functions, service permission enforcement, cycle detection
**Confidence:** HIGH

## Summary

Phase 22 implements the `call-service` host function that was stubbed in Phase 20. A WASM component calls `call_service(target_id, payload)` and receives response bytes synchronously — but the host side must execute another WASM component asynchronously, requiring fiber-based async host bindings.

The critical architectural challenge is making a single host import function async while keeping the rest of the `Host` trait synchronous. The solution is the wasmtime bindgen `imports: { "call-service": async }` selective async override, which generates an async method only for that one host function. This requires adding the `"async"` feature to the wasmtime workspace dependency (which enables `wasmtime-fiber`). Without this, `func_wrap_async` is behind `#[cfg(feature = "async")]` and will not compile.

Permission enforcement (RPC-02, RPC-03) and cycle detection (RPC-04) require two structural additions to `OperatorHostComponent`: an injected `rpc_caller` callback (via a trait object to avoid a circular crate dependency) and a `call_stack: Vec<String>` tracking the current call chain.

**Primary recommendation:** Add `"async"` to wasmtime features, add `imports: { "call-service": async }` to the bindgen macro in `world.rs`, inject an `Arc<dyn RpcCaller>` and `call_stack` into `OperatorHostComponent`, and implement the async `call_service` method to perform permission checks, cycle detection, and delegate to the engine.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

### Claude's Discretion

All implementation choices are Claude's discretion.

### Deferred Ideas (OUT OF SCOPE)

None — infrastructure phase.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| RPC-01 | `call-service` host function using `func_wrap_async` — re-entrant `Arc<WasmEngine>` calls `execute_operator_component` directly | Selective async bindgen + `RpcCaller` trait injection covers this |
| RPC-02 | `AllowedServiceCalls` permission enforcement — engine checks caller's permission before dispatching call | `Permissions.allowed_service_calls` already in types; check in async `call_service` impl |
| RPC-03 | `AllowedCallers` callee-side enforcement — engine checks callee accepts calls from the caller service | `Component.allowed_callers` already in types; check after resolving callee service |
| RPC-04 | Call depth limit (default 5) with cycle detection — prevents A→B→A deadlocks and unbounded nesting | `call_stack: Vec<String>` in `OperatorHostComponent`; check length and cycle before dispatch |
</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| wasmtime | 42.0.1 | WASM execution + async host functions | Already in workspace; needs `"async"` feature added |
| wasmtime-fiber | (pulled by `async` feature) | Stack-switching for fiber-based async | Required by `func_wrap_async` / async bindgen imports |
| tokio | 1.47.1 | Async runtime for host-side execution | Already `"full"` features in workspace |

### Supporting

No new library dependencies are needed. All required types (`AllowedServiceCalls`, `AllowedCallers`, `TriggerData::Raw`) are already in `wavs-types`.

### Required Feature Change

```toml
# Cargo.toml (workspace)  — BEFORE
wasmtime = { version = "42.0.1", features = ["cache", "component-model", "runtime", "std"] }

# AFTER
wasmtime = { version = "42.0.1", features = ["async", "cache", "component-model", "runtime", "std"] }
```

[VERIFIED: /home/node/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/wasmtime-42.0.1/src/runtime/component/linker.rs line 460] — `func_wrap_async` is behind `#[cfg(feature = "async")]`.

[VERIFIED: wasmtime-42.0.1/Cargo.toml] — `async` feature pulls `dep:wasmtime-fiber`, `wasmtime-component-macro?/async`, `runtime`.

## Architecture Patterns

### Recommended File Changes

```
Cargo.toml                                    # add "async" to wasmtime features
packages/engine/src/bindings/operator/
  world.rs                                    # add imports: { "call-service": async } to bindgen macro
  host.rs                                     # implement async call_service method
packages/engine/src/worlds/operator/
  component.rs                                # add call_stack + rpc_caller to OperatorHostComponent
packages/engine/src/worlds/instance.rs       # add rpc_caller field to InstanceDepsBuilder; thread it in
packages/engine/src/utils/error.rs           # add RpcPermissionDenied, RpcCycleDetected, RpcDepthExceeded variants
packages/engine/src/lib.rs or new file       # pub trait RpcCaller
packages/wavs/src/subsystems/engine/
  wasm_engine.rs                              # impl RpcCaller for WasmEngine, inject in execute_operator_component
packages/engine/tests/
  rpc.rs                                      # new test file for permission + cycle tests
```

### Pattern 1: Selective Async Bindgen Import

The wasmtime bindgen macro supports per-function async overrides via the `imports` config.

**What:** Make only `call-service` async in the generated `Host` trait; all other host functions stay sync.
**When to use:** When one host import needs to await (call another async function) while the rest are sync.

```rust
// packages/engine/src/bindings/operator/world.rs
// Source: wasmtime-42.0.1/src/runtime/component/bindgen_examples/_7_async.rs

bindgen!({
    world: "wavs-world",
    path: "../../wit-definitions/operator/wit",
    with: {
        "wasi:keyvalue/store.bucket": crate::backend::wasi_keyvalue::bucket_keys::KeyValueBucket,
        "wasi:keyvalue/atomics.cas": crate::backend::wasi_keyvalue::atomics::KeyValueCas,
    },
    exports: {
        default: async,
    },
    imports: {
        "call-service": async,   // ONLY this host function is async
    },
});
```

This generates an async method in the `Host` trait:
```rust
async fn call_service(&mut self, service_id: String, payload: Vec<u8>) -> Result<Vec<u8>, String>;
```

And registers it via `func_wrap_async` in the generated `add_to_linker`. No manual linker manipulation needed.

**Critical:** The same `imports: { "call-service": async }` must be added to the `wavs-legacy-world` bindgen block as well, since the legacy world also has `call-service` in its host interface.

### Pattern 2: RpcCaller Trait for Circular Dependency Avoidance

`wavs-engine` cannot import from `wavs` (circular dependency). The `WasmEngine` that executes components lives in `wavs`. The host function in `wavs-engine` needs to invoke it.

**What:** Define a trait in `wavs-engine`; implement it in `wavs` on `WasmEngine`; inject via `Arc<dyn RpcCaller>`.
**When to use:** Any time `wavs-engine` code needs a runtime capability provided by the top-level `wavs` crate.

```rust
// packages/engine/src/rpc.rs (new file in wavs-engine)
// Source: [ASSUMED] standard Rust trait object injection pattern

use std::{future::Future, pin::Pin};
use wavs_types::{ServiceId, Service, TriggerAction, WasmResponse};

pub type RpcResult = Result<Vec<u8>, String>;
pub type RpcFuture<'a> = Pin<Box<dyn Future<Output = RpcResult> + Send + 'a>>;

/// Injected into OperatorHostComponent so call_service can execute callee components
/// without creating a circular dependency on the `wavs` crate.
pub trait RpcCaller: Send + Sync {
    /// Execute a callee service and return the first response payload.
    /// `caller_id` and `call_stack` are used for permission enforcement and cycle detection
    /// by the caller; the implementation calls execute_operator_component directly.
    fn call(
        &self,
        callee_service_id: ServiceId,
        callee_service: Service,
        trigger_action: TriggerAction,
    ) -> RpcFuture<'_>;
}
```

Responsibility split:
- `wavs-engine` (`call_service` impl): permission checks, cycle detection, depth check, service lookup delegation
- `wavs` crate (`RpcCaller` impl): service lookup from `Services`, building `TriggerAction`, calling `execute_operator_component`

### Pattern 3: Call Stack in OperatorHostComponent

**What:** Track the call chain as a `Vec<String>` of service IDs in the store data.
**Why:** Fiber-based async means the store data is accessible during host function execution. The call stack is threaded through each nested execution via the injected `RpcCaller` (which creates a new `OperatorHostComponent` with an extended call stack for the callee).

```rust
// packages/engine/src/worlds/operator/component.rs

pub struct OperatorHostComponent {
    pub service: Service,
    pub workflow_id: WorkflowId,
    pub chain_configs: ChainConfigs,
    pub trigger_data: TriggerData,
    pub(crate) table: wasmtime::component::ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http_ctx: WasiHttpCtx,
    pub(crate) tls_ctx: WasiTlsCtx,
    pub(crate) keyvalue_ctx: KeyValueCtx,
    pub(crate) inner_log: OperatorHostComponentLogger,
    // Phase 22 additions:
    pub call_stack: Vec<String>,                      // service IDs in current call chain
    pub rpc_caller: Option<Arc<dyn RpcCaller>>,       // None disables RPC
}
```

The `call_stack` contains `[root_service_id, caller_service_id]` for a depth-2 chain. For cycle detection: check if `callee_service_id` already appears in `call_stack`. For depth limit: check `call_stack.len() >= RPC_MAX_DEPTH` (default 5).

### Pattern 4: Synthetic TriggerAction for RPC Calls

`execute_operator_component` takes a full `TriggerAction`. For RPC calls the trigger is synthetic.

```rust
// In the RpcCaller impl (wavs crate)
// Source: [ASSUMED] consistent with TriggerData::Raw usage in existing tests

use wavs_types::{TriggerAction, TriggerConfig, TriggerData, Trigger};

fn build_rpc_trigger(callee_service: &Service, payload: Vec<u8>, caller_workflow_id: &WorkflowId) -> TriggerAction {
    // Pick the first (lexicographic) workflow of the callee as the RPC target
    let callee_workflow_id = callee_service.workflows.keys().next()
        .expect("callee service has at least one workflow")
        .clone();

    TriggerAction {
        config: TriggerConfig {
            service_id: callee_service.id(),
            workflow_id: callee_workflow_id,
            trigger: Trigger::Cron,  // or a new Trigger::Rpc variant; Cron works as placeholder
        },
        data: TriggerData::Raw(payload),
    }
}
```

Note: A `Trigger::Rpc` variant could be added to `wavs_types::Trigger` for clarity, but is not required for functionality. Using `Trigger::Cron` as a placeholder is acceptable for v3.0.

### Pattern 5: call_service Implementation

```rust
// packages/engine/src/bindings/operator/host.rs
// Source: [ASSUMED] based on codebase patterns + STATE.md design decision

impl super::world::host::Host for OperatorHostComponent {
    // ... other sync methods unchanged ...

    async fn call_service(
        &mut self,
        callee_id: String,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        const RPC_MAX_DEPTH: usize = 5;

        let caller_service_id = self.service.id().to_string();

        // RPC-02: Caller permission check
        let allowed = match &self.service.workflows
            .get(&self.workflow_id)
            .map(|w| &w.component.permissions.allowed_service_calls)
        {
            Some(AllowedServiceCalls::All) => true,
            Some(AllowedServiceCalls::Only(ids)) => ids.contains(&callee_id),
            Some(AllowedServiceCalls::None) | None => false,
        };
        if !allowed {
            return Err(format!(
                "call-service denied: caller '{}' does not have permission to call '{}'",
                caller_service_id, callee_id
            ));
        }

        // RPC-04: Cycle detection
        if self.call_stack.contains(&callee_id) {
            return Err(format!(
                "call-service cycle detected: '{}' is already in the call chain {:?}",
                callee_id, self.call_stack
            ));
        }

        // RPC-04: Depth limit
        if self.call_stack.len() >= RPC_MAX_DEPTH {
            return Err(format!(
                "call-service depth limit ({}) exceeded: call chain {:?}",
                RPC_MAX_DEPTH, self.call_stack
            ));
        }

        // Get the caller reference
        let rpc_caller = self.rpc_caller.clone()
            .ok_or_else(|| "call-service not configured: no RPC caller injected".to_string())?;

        // Thread the call stack
        let mut new_call_stack = self.call_stack.clone();
        new_call_stack.push(caller_service_id);

        // Delegate to the engine (resolves callee service, checks RPC-03, executes component)
        rpc_caller.call(callee_id, payload, new_call_stack).await
    }
}
```

### Pattern 6: RpcCaller impl in wasm_engine.rs

```rust
// packages/wavs/src/subsystems/engine/wasm_engine.rs — new impl block
// Source: [ASSUMED] based on existing execute_operator_component signature

impl<S: CAStorage + Send + Sync + 'static> RpcCaller for WasmEngine<S> {
    fn call(&self, callee_id: String, payload: Vec<u8>, call_stack: Vec<String>) -> RpcFuture<'_> {
        Box::pin(async move {
            // Resolve callee service from Services registry
            let callee_service_id = /* parse callee_id as ServiceId */;
            let callee_service = self.services.get(&callee_service_id)
                .map_err(|e| format!("call-service: callee service not found: {}", e))?;

            // RPC-03: Callee-side AllowedCallers check
            let caller_id = call_stack.last()
                .ok_or_else(|| "call-service: empty call stack".to_string())?;
            let callee_workflow = callee_service.workflows.values().next()
                .ok_or_else(|| "call-service: callee has no workflows".to_string())?;
            let callee_allowed = match &callee_workflow.component.allowed_callers {
                Some(AllowedCallers::All) => true,
                Some(AllowedCallers::Only(ids)) => ids.contains(caller_id),
                Some(AllowedCallers::None) | None => false,
            };
            if !callee_allowed {
                return Err(format!(
                    "call-service denied: callee '{}' does not accept calls from '{}'",
                    callee_id, caller_id
                ));
            }

            // Build synthetic trigger action
            let trigger_action = build_rpc_trigger(&callee_service, payload, &call_stack);

            // Execute with extended call stack (injected into callee's OperatorHostComponent)
            let responses = self.execute_operator_component_with_call_stack(
                callee_service, trigger_action, call_stack
            ).await.map_err(|e| e.to_string())?;

            // Return first response payload
            responses.into_iter().next()
                .map(|r| r.payload)
                .ok_or_else(|| "call-service: callee returned no responses".to_string())
        })
    }
}
```

This requires `WasmEngine` to hold a reference to `Services`. Currently `WasmEngine` in `wavs/src/subsystems/engine/wasm_engine.rs` does NOT hold `Services` — it's stored in `EngineManager`. Options:
- Add `Services` to `WasmEngine` (simplest)
- Pass `Services` into `WasmEngine::execute_operator_component_with_call_stack`
- Wrap both in the `RpcCaller` implementation via a newtype: `struct RpcCallerImpl { engine: Arc<WasmEngine<S>>, services: Services }`

The `RpcCallerImpl` newtype approach is cleanest: it keeps `WasmEngine` unchanged and places the service-lookup logic alongside the engine reference injection.

### Anti-Patterns to Avoid

- **Routing through Dispatcher channel**: STATE.md explicitly prohibits this. Dispatcher is async-channel-based and creates deadlock risk when called from within a WASM execution fiber.
- **`block_in_place` for async execution**: Works but is not `func_wrap_async` and misuses the thread pool. STATE.md specifies `func_wrap_async`.
- **Re-registering call-service after `add_to_linker`**: Wasmtime returns an error if a name is already registered — cannot override.
- **Using `tokio::runtime::Handle::current().block_on()`** directly in sync context: Will panic inside an async context.
- **Making ALL host imports async**: Unnecessary overhead; only `call-service` needs async.
- **Using `Service::id()` which may differ from the string ServiceId representation**: Verify `ServiceId` parsing from `String` before using in lookups.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async host function with fiber suspension | Manual store state machine | `imports: { "call-service": async }` in bindgen | Generates `func_wrap_async` registration automatically |
| Service lookup | Custom registry | `Services::get(service_id)` in `wavs` crate | Already handles all lookup/error cases |
| Callee workflow selection | Custom routing logic | `.workflows.values().next()` (first workflow) | Simple and consistent; callee can always expose one RPC workflow |
| RPC payload serialization | Custom codec | `TriggerData::Raw(payload)` | Already established pattern in tests |

## Common Pitfalls

### Pitfall 1: wasmtime `async` Feature Not Added

**What goes wrong:** `func_wrap_async` is `#[cfg(feature = "async")]` — code compiles but the async bindgen generates a compile error about unavailable `func_wrap_async`.
**Why it happens:** The workspace `Cargo.toml` has wasmtime without the `"async"` feature. The `imports: { "call-service": async }` in the bindgen macro tries to use `func_wrap_async` internally.
**How to avoid:** First task must be adding `"async"` to the wasmtime features list in `Cargo.toml`, then run `cargo check -p wavs-engine` to verify compilation.
**Warning signs:** Compile error mentioning `func_wrap_async` or `#[cfg(feature = "async")]`.

### Pitfall 2: Legacy World Bindgen Not Updated

**What goes wrong:** `wavs-legacy-world` in `world.rs` also has `call-service` in its host interface. If it is not given `imports: { "call-service": async }`, the legacy world's `Host` trait has a sync `call_service` but the struct implements the async version — type mismatch compile error.
**Why it happens:** Two bindgen macro invocations in `world.rs` — main and legacy. Both must be updated.
**How to avoid:** Update both `bindgen!` blocks in `world.rs`.

### Pitfall 3: WasmEngine Does Not Hold Services

**What goes wrong:** The `RpcCaller` impl needs to look up callee services by `ServiceId` from the `Services` registry. `WasmEngine` currently does not hold a `Services` reference — only `EngineManager` does.
**Why it happens:** The separation of concerns in the engine architecture.
**How to avoid:** Use a `RpcCallerImpl { engine: Arc<WasmEngine<S>>, services: Services }` newtype, or add `Services` to `WasmEngine`. The newtype avoids touching `WasmEngine::new()` callers. The `RpcCallerImpl` is constructed in `EngineManager` where both `engine` and `services` are available.

### Pitfall 4: ServiceId String Roundtrip

**What goes wrong:** `call_service("target-id", payload)` passes a `String`. `ServiceId` is a hash type. Parsing it back may use a different representation than `service.id().to_string()`.
**Why it happens:** `ServiceId` derives from a hash of `ServiceManager` — not a human-readable string. The `to_string()` returns hex.
**How to avoid:** Verify `ServiceId` roundtrip (from `String` hex → `ServiceId` → lookup). Check how `Services::get` takes its key. Use the same string format produced by `ServiceId::to_string()` in all places. [VERIFIED: codebase uses hex representation via `ServiceId::hash()`].

### Pitfall 5: call_stack Not Threaded Into Callee's OperatorHostComponent

**What goes wrong:** The callee executes with an empty `call_stack`, so cycle detection in nested calls fails.
**Why it happens:** The `execute_operator_component` path builds a fresh `OperatorHostComponent` without a call stack. A separate `execute_operator_component_with_call_stack` method (or an extra parameter) is needed.
**How to avoid:** Add `call_stack: Vec<String>` as a parameter to the execution path used by `RpcCaller`, or add it to `InstanceDepsBuilder`.

### Pitfall 6: Callee Has No Workflows

**What goes wrong:** `callee_service.workflows.values().next()` returns `None` if the service has no workflows — unexpected, but defensible.
**Why it happens:** In theory, a deployed service always has at least one workflow. But an empty `workflows` map is representable.
**How to avoid:** Return a clear `Err("call-service: callee service '{id}' has no workflows")` rather than panicking.

### Pitfall 7: RpcCaller Arc is None When RPC Disabled

**What goes wrong:** If `rpc_caller: None`, calling `call_service` returns a generic "not configured" error instead of a permission error for `AllowedServiceCalls::None`.
**Why it happens:** The `AllowedServiceCalls::None` check happens before the `rpc_caller.is_some()` check in the impl above. This is intentional — the error message is clearer for the permission case.
**How to avoid:** Check permissions first (returns permission-denied), then check `rpc_caller`. A component with `AllowedServiceCalls::None` gets a permission error, not a "not configured" error. Only fall through to the `rpc_caller.is_none()` case if permissions somehow passed without an injected caller.

## Code Examples

### Selective Async Import Bindgen Syntax

```rust
// Source: wasmtime-42.0.1 bindgen_examples/_7_async.rs (pattern)
// [VERIFIED: config.rs FunctionConfig supports per-function async override]

bindgen!({
    world: "wavs-world",
    path: "../../wit-definitions/operator/wit",
    with: {
        "wasi:keyvalue/store.bucket": crate::backend::wasi_keyvalue::bucket_keys::KeyValueBucket,
        "wasi:keyvalue/atomics.cas": crate::backend::wasi_keyvalue::atomics::KeyValueCas,
    },
    exports: { default: async },
    imports: { "call-service": async },
});
```

### InstanceDepsBuilder Extension

```rust
// packages/engine/src/worlds/instance.rs — InstanceDepsBuilder struct
// Source: [ASSUMED]

pub struct InstanceDepsBuilder<'a, P> {
    pub component: wasmtime::component::Component,
    pub service: Service,
    pub workflow_id: WorkflowId,
    pub data: InstanceData,
    pub engine: &'a WTEngine,
    pub data_dir: P,
    pub chain_configs: &'a ChainConfigs,
    pub log: HostComponentLogger,
    pub keyvalue_ctx: KeyValueCtx,
    // Phase 22 additions:
    pub rpc_caller: Option<Arc<dyn RpcCaller>>,   // None for aggregator/legacy
    pub call_stack: Vec<String>,                   // empty at root, extended for RPC calls
}
```

Default the new fields to `None`/empty in the `.build()` construction of `OperatorHostComponent`. All existing `InstanceDepsBuilder` construction sites add `rpc_caller: None, call_stack: vec![]` — no behavior change for existing code.

### New EngineError Variants

```rust
// packages/engine/src/utils/error.rs

#[error("call-service permission denied for service {caller_id} calling {callee_id}: {reason}")]
RpcPermissionDenied {
    caller_id: String,
    callee_id: String,
    reason: String,
},

#[error("call-service cycle detected in chain {call_chain:?}")]
RpcCycleDetected { call_chain: Vec<String> },

#[error("call-service depth limit {limit} exceeded in chain {call_chain:?}")]
RpcDepthExceeded { limit: usize, call_chain: Vec<String> },
```

These are returned from `RpcCaller::call()` as `String` errors (since the WIT function returns `result<list<u8>, string>`) and propagated as `Err(String)` to the component.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| All host imports sync via `Host` trait | Selective async import via bindgen `imports: { fn: async }` | wasmtime >=38 | Enables mixed sync/async host functions without manual linker registration |
| `block_in_place` for sync-to-async bridging | `func_wrap_async` via fiber suspension | wasmtime >=29 | No thread pool starving; WASM fiber suspends cleanly |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `imports: { "call-service": async }` syntax works in wasmtime 42.0.1 bindgen to make only one host function async | Standard Stack, Architecture | Must fall back to `imports: { default: async }` (all imports async) or manual linker registration — more invasive |
| A2 | `TriggerData::Raw(payload)` passed to callee is the correct mechanism for RPC payload forwarding | Pattern 4 | Callee must be designed to read `TriggerData::Raw`; may need a `TriggerData::Rpc` variant for clarity |
| A3 | The first (lexicographic) workflow of the callee service is the correct RPC dispatch target | Pattern 4 | Multi-workflow services called via RPC will always route to the first workflow — may need a convention or separate RPC workflow designation |
| A4 | `wasmtime-fiber` compile overhead is acceptable on this build target | Standard Stack | Platform-specific compile issues on linux/x86_64 are unlikely but not verified |

**If A1 is wrong:** Use `imports: { default: async }` with all host functions becoming async (adds `async fn` to sync operations like `log`, `get_service` — minor overhead). OR manually register `call-service` via `func_wrap_async` by not using `add_to_linker` for that specific function.

## Open Questions

1. **ServiceId string roundtrip format**
   - What we know: `ServiceId` is a hash type; `to_string()` produces a hex string
   - What's unclear: What exact string format should callers pass to `call_service("target-id", ...)`? Is it the hex of the service manager hash? Or a human-readable name?
   - Recommendation: Use `ServiceId::to_string()` (hex) as the canonical identifier. Document this in the implementation. Phase 23 E2E tests will surface any mismatch.

2. **Callee workflow selection**
   - What we know: Services have a `BTreeMap<WorkflowId, Workflow>` — multiple workflows possible
   - What's unclear: Should RPC always target the first workflow, or should callers specify a workflow?
   - Recommendation: For v3.0, use first workflow (lexicographic). The WIT signature has no `workflow_id` parameter, and adding it would be a WIT change. This is sufficient for E2E-05 success criteria.

3. **Trigger variant for RPC**
   - What we know: `TriggerAction::config.trigger` needs a valid `Trigger` variant; `TriggerData::Raw` is appropriate for payload
   - What's unclear: Should a `Trigger::Rpc` variant exist for callee components to detect they are being called via RPC?
   - Recommendation: Add `Trigger::Rpc { caller_service_id: String }` to `wavs_types::Trigger` — it's a clean semantic addition that lets callee components behave differently when called via RPC vs triggered by chain events. Low-risk change.

## Environment Availability

Step 2.6: SKIPPED — Phase 22 is a pure code change within the existing WAVS mono-repo. No new external tools, services, or runtimes required. The `wasmtime-fiber` library is a Rust crate that compiles from source.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | N/A |
| V3 Session Management | no | N/A |
| V4 Access Control | yes | `AllowedServiceCalls` + `AllowedCallers` two-sided permission model |
| V5 Input Validation | yes | Payload bytes passed through; callee validates its own inputs |
| V6 Cryptography | no | N/A |

### Known Threat Patterns for this Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Service impersonation (caller lies about its ID) | Spoofing | Caller ID read from `self.service.id()` in host — cannot be forged by WASM component |
| Unbounded recursion DoS | Denial of Service | Depth limit (5) + cycle detection (A→B→A) in call stack |
| Callee called without consent | Elevation of Privilege | `AllowedCallers` callee-side check before dispatching |
| Unauthorized outbound service calls | Elevation of Privilege | `AllowedServiceCalls` caller-side check; default is `None` (deny-all) |
| Payload size amplification | DoS | Existing `max_payload_size` check on callee responses; same limits apply |

**Security invariant:** Both caller AND callee must opt in for a call to succeed. Neither side alone can authorize cross-service calls.

## Sources

### Primary (HIGH confidence)

- wasmtime-42.0.1 source: `/home/node/.cargo/registry/src/.../wasmtime-42.0.1/src/runtime/component/linker.rs:460` — `func_wrap_async` confirmed behind `#[cfg(feature = "async")]`
- wasmtime-42.0.1 Cargo.toml: `async` feature pulls `dep:wasmtime-fiber`
- wasmtime-internal-wit-bindgen-42.0.1 source: `config.rs` — `FunctionFlags::ASYNC` per-function config confirmed
- wasmtime-42.0.1 bindgen_examples/_7_async.rs — `imports: { default: async | trappable }` pattern
- `/workspace/WAVS/packages/engine/src/worlds/operator/execute.rs` — continuation loop, agent/legacy routing
- `/workspace/WAVS/packages/engine/src/worlds/operator/component.rs` — `OperatorHostComponent` fields
- `/workspace/WAVS/packages/engine/src/bindings/operator/host.rs` — existing `call_service` stub
- `/workspace/WAVS/packages/types/src/service.rs:712-736` — `AllowedServiceCalls` and `AllowedCallers` enum definitions
- `/workspace/WAVS/packages/types/src/service.rs:609-621` — `Permissions` struct with `allowed_service_calls`
- `/workspace/WAVS/packages/types/src/service.rs:209` — `Component.allowed_callers` field
- `/workspace/WAVS/.planning/STATE.md` — `call-service must use func_wrap_async; re-entrant Arc<WasmEngine>` locked decision
- `/workspace/WAVS/packages/wavs/src/subsystems/engine.rs:53` — `Arc<WasmEngine<S>>` in `EngineManager`
- `/workspace/WAVS/Cargo.toml` — wasmtime 42.0.1 without `async` feature (confirmed need to add)

### Secondary (MEDIUM confidence)

- Phase 20 SUMMARY (20-01): WIT `call-service` import confirmed in both `wavs-world` and `wavs-legacy-world`; stub returns `Err("call-service not yet implemented (Phase 22)")`
- Phase 21 SUMMARY (21-01, 21-02): `OperatorHostComponent` + `InstanceDepsBuilder` patterns; field propagation across 12 callers when adding new fields

### Tertiary (LOW confidence)

- A1 (assumed): `imports: { "call-service": async }` exact bindgen syntax — verified pattern exists but specific host import scoping not tested in this codebase

## Metadata

**Confidence breakdown:**
- Standard Stack (wasmtime async feature): HIGH — verified in source
- Architecture (selective async bindgen): MEDIUM-HIGH — pattern confirmed in bindgen examples; exact host interface scoping [ASSUMED]
- Architecture (RpcCaller trait): HIGH — standard Rust pattern, no codebase obstacles
- Architecture (call_stack threading): HIGH — well-established pattern, matches existing continuation step counting
- Pitfalls: HIGH — all derived from direct source code inspection

**Research date:** 2026-04-22
**Valid until:** 2026-05-22 (stable crates; wasmtime bindgen API is stable at 42.x)
