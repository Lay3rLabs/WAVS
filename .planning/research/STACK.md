# Stack Research

**Domain:** WASI agent runtime — continuation mode + service-to-service RPC (v3.0 additions)
**Researched:** 2026-04-20
**Confidence:** HIGH — based on direct codebase inspection of all relevant packages

---

## Executive Summary

v3.0 adds agent continuation mode and synchronous service-to-service RPC. Neither feature
requires new external crates. Every mechanism is built from primitives already in the
workspace: `wasmtime 42.0.1` async host functions, `wasi:keyvalue` KV, `wit-bindgen 0.53.1`
variant types, `tokio` async tasks, and the existing `WasmEngine::execute_operator_component`
path. The total surface area is: one new WIT variant, two new host function registrations,
one new `Permissions` field, a step loop in `run_trigger`, and a widened `WavsAgent` trait.

---

## Recommended Stack

### No New External Crates

v3.0 adds zero new Cargo dependencies. All building blocks already exist:

| Existing Primitive | Version | Role in v3.0 |
|-------------------|---------|--------------|
| `wasmtime` | 42.0.1 (pinned) | Async host function for `call-service`; new WIT variant binding in `execute.rs` |
| `wasmtime-wasi` | 42.0.1 (pinned) | No change |
| `wit-bindgen` | 0.53.1 (pinned) | Re-run codegen after `operator.wit` variant change; no tooling version change |
| `wasi:keyvalue` (host-provided) | 0.2.0-draft2 | Continuation state persistence under `wavs_agent_step:` key prefix |
| `tokio` | workspace | Async host function body; step loop; `.await` on recursive component exec |
| `serde` / `serde_json` | workspace | Serialize `AgentContinuation` opaque state blob to KV |
| `thiserror` | workspace | Two new `EngineError` variants (`ContinuationLimit`, `ServiceCallDenied`) |
| `wavs_types::Permissions` | existing | Gains `allowed_service_calls: AllowedServiceCalls` field with serde default `None` |

---

### WIT Changes — `wit-definitions/operator/wit/operator.wit`

**1. New return variant for the `run` export.**

Current signature:
```wit
export run: func(trigger-action: trigger-action) -> result<list<wasm-response>, string>;
```

New signature:
```wit
variant agent-step-result {
    // Terminal — emit these responses and finish
    done(list<wasm-response>),
    // Non-terminal — re-invoke after persisting state
    continue(agent-continuation),
}

record agent-continuation {
    // Opaque bytes the agent wants restored on next invocation.
    // If absent, engine auto-restores KV-persisted conversation state.
    state: option<list<u8>>,
    // Human-readable step reason (logged, never submitted on-chain)
    reason: option<string>,
}

export run: func(trigger-action: trigger-action) -> result<agent-step-result, string>;
```

Backward compatibility: non-agent components (plain `wasm-response` semantics) wrap their
existing logic in `agent-step-result::done(responses)`. The call site in `execute.rs`
handles both by always expecting `agent-step-result`.

**2. New `call-service` import in the `host` interface block.**

The existing `host` interface already contains `get-evm-chain-config`, `config-var`, `log`,
`get-service`, `get-workflow`, `get-event-id`. Add:

```wit
import host: interface {
    // ... existing imports unchanged ...

    // Synchronous RPC to another deployed service on this node.
    // Returns the first WasmResponse payload bytes from the target.
    // Blocked if target service-id is not in AllowedServiceCalls.
    call-service: func(
        service-id: service-id,
        workflow-id: workflow-id,
        payload: list<u8>
    ) -> result<list<u8>, string>;
}
```

No new WIT packages. No new WIT worlds. Both changes go inside `operator.wit`.

---

### Rust Host Side — `packages/engine`

**`OperatorHostComponent` (`worlds/operator/component.rs`)** — add two fields:

```rust
pub struct OperatorHostComponent {
    // ... existing fields ...
    pub allowed_service_calls: AllowedServiceCalls,   // new
    pub services: Arc<RwLock<Services>>,               // new — for call-service dispatch
}
```

`Services` is the existing `crate::services::Services` struct already threaded through
`WasmEngine`. Pass a clone of the `Arc` at instance construction time.

**`call-service` host function (`worlds/operator/component.rs` or a new `host_fns.rs`):**

```rust
linker.func_wrap_async("host", "call-service", |mut ctx, (service_id, workflow_id, payload): (String, String, Vec<u8>)| {
    Box::new(async move {
        let (allowed, services, engine) = ctx.data_mut().call_service_deps();
        // 1. Permission check — AllowedServiceCalls::None returns error immediately
        if !allowed.is_permitted(&service_id) {
            return Ok((Err(format!("ServiceCallDenied: {}", service_id)),));
        }
        // 2. Resolve target service
        let service = services.get(&service_id)?;
        // 3. Execute synchronously (awaited inline — host functions can .await in async stores)
        let responses = engine.execute_operator_component(service, make_trigger_action(service_id, workflow_id, payload)).await?;
        Ok((Ok(responses.into_iter().next().map(|r| r.payload).unwrap_or_default()),))
    })
})?;
```

The host function runs inside the existing `wasmtime` async engine. Direct `.await` on
`execute_operator_component` is correct — async host functions in Wasmtime with
`Config::async_support(true)` (already configured) can freely `.await` Tokio futures.

**Engine step loop (`packages/wavs/src/subsystems/engine/wasm_engine.rs`):**

`run_trigger` gains a loop around `execute_operator_component`:

```rust
const MAX_CONTINUATION_STEPS: u32 = 10;  // config constant, operator-adjustable in service.json

let mut step = 0u32;
let responses = loop {
    let step_result = self.engine.execute_one_step(service.clone(), action.clone()).await?;
    match step_result {
        AgentStepResult::Done(responses) => break responses,
        AgentStepResult::Continue(cont) => {
            // Engine persists continuation state on behalf of the component
            kv_write_continuation_state(&kv, &action.correlation_id, step, &cont)?;
            step += 1;
            if step >= MAX_CONTINUATION_STEPS {
                return Err(EngineError::ContinuationLimit(service.id(), action.config.workflow_id));
            }
        }
    }
};
```

Fuel accumulation across steps: each step starts with the full per-component fuel limit.
Alternatively, share a single budget across steps (simpler for MVP — each step gets its own
full budget, which is the easier change and avoids tracking partial fuel).

---

### KV State Convention — Continuation Persistence

The engine (host side) writes and reads continuation state. Components never touch this
KV namespace directly.

| Key | Content | Who Writes | Who Reads |
|-----|---------|-----------|----------|
| `wavs_agent_step:<correlation_id>:<step>` | Serialized `AgentContinuation` bytes | Engine host (after `Continue` return) | Engine host (before next `call_run`) |

This uses the existing per-service `KeyValueCtx` namespace. No bucket/namespace conflicts:
`WavsMemory` uses `wavs_agent_memory:` prefix; continuation uses `wavs_agent_step:` prefix.

The `correlation_id: String` field already on `TriggerAction` serves as the unique
per-invocation key component.

---

### Rust Guest Side — `packages/wavs-rig`

**`WavsAgent` trait (`src/agent.rs`)** — widen return type:

```rust
pub enum AgentOutput<T: Serialize> {
    Done(T),
    Continue {
        // Agent-managed opaque state (optional — WavsMemory handles conversation automatically)
        state: Option<Vec<u8>>,
        // Human-readable step reason for logs
        reason: Option<String>,
    },
}

pub trait WavsAgent {
    type Output: Serialize;
    fn run(&self, trigger_data: Vec<u8>)
        -> impl Future<Output = anyhow::Result<AgentOutput<Self::Output>>> + '_;
}
```

`run_agent` maps `AgentOutput::Done(v)` → `agent-step-result::done(json_bytes)` and
`AgentOutput::Continue { .. }` → `agent-step-result::continue(agent-continuation)`.

**`call_service` binding (`src/tools/mod.rs` or new `src/rpc.rs`):**

```rust
/// Call another deployed WAVS service synchronously.
/// Requires AllowedServiceCalls to permit the target service-id.
pub fn call_service(service_id: &str, workflow_id: &str, payload: &[u8]) -> anyhow::Result<Vec<u8>> {
    use crate::bindings::wavs::operator::host;
    host::call_service(service_id, workflow_id, payload)
        .map_err(|e| anyhow::anyhow!("call-service failed: {}", e))
}
```

This is usable as a rig tool (implement `Tool` trait wrapping `call_service`) or as a
direct call within the agent's async loop.

---

### New `wavs_types` Fields (`packages/types/src/service.rs`)

**`Permissions` struct** — one additive field:

```rust
pub struct Permissions {
    pub allowed_http_hosts: AllowedHostPermission,  // existing
    pub file_system: bool,                           // existing
    pub raw_sockets: bool,                           // existing
    pub dns_resolution: bool,                        // existing
    #[serde(default, skip_serializing_if = "AllowedServiceCalls::is_none")]
    pub allowed_service_calls: AllowedServiceCalls,  // NEW
}
```

Serde default `None` means all existing `service.json` files deserialize without change.

**`AllowedServiceCalls` enum** — mirrors `AllowedHostPermission` exactly:

```rust
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AllowedServiceCalls {
    All,
    Only(Vec<ServiceId>),
    #[default]
    None,
}
```

`service.json` usage:
```json
{
  "permissions": {
    "allowed_service_calls": { "only": ["<target-service-id>"] }
  }
}
```

---

## Supporting Libraries (No New Adds)

| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| `serde` / `serde_json` | workspace | Serialize `AgentContinuation` state blob to KV bytes | Already used throughout |
| `thiserror` | workspace | `EngineError::ContinuationLimit`, `EngineError::ServiceCallDenied` | Two new variants in existing error enum |
| `tokio` | workspace | Async host function body; `.await` inside `func_wrap_async` | `Config::async_support(true)` already set in `BaseEngine` |
| `wasmtime 42.0.1` | pinned | `func_wrap_async` for `call-service` host function | Already used for all other host functions |

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| KV-persisted continuation state (engine-managed) | Guest-managed KV writes via `wasi:keyvalue` | Engine management is invisible to the guest, enabling step budget enforcement and atomicity guarantees. Guest-managed requires convention compliance with no enforcement. |
| New WIT `variant` return type on `run` | Separate `run-continuation` WIT export | Separate export breaks the single-entrypoint model and complicates aggregator routing. Variant keeps one export. |
| Direct `.await` on `execute_operator_component` inside `call-service` host function | New crossbeam channel RPC path | Channel path adds latency and complexity and risks deadlock on the dispatcher thread. Direct async `.await` in the host function body is idiomatic Wasmtime async and already safe in this codebase. |
| `AllowedServiceCalls` in `Permissions` struct | Separate top-level `service.json` field | `Permissions` is the established pattern; co-location with `allowed_http_hosts` is consistent. |
| `MAX_CONTINUATION_STEPS = 10` constant | No limit | Without a limit, a buggy agent loops indefinitely and monopolizes the engine. |
| Per-step full fuel budget | Shared fuel budget across steps | Per-step is simpler for MVP; shared budget is a future refinement if operators need tighter compute metering across multi-step agents. |

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| New async runtime or executor for guest continuation | WASM is single-threaded; second `block_on` deadlocks | Existing `wstd::runtime::block_on` boundary; continuation is host-driven, not guest-driven |
| WASI 0.3 async component model | Not stable in Wasmtime 42.0.1; async I/O landed in 2025 roadmap but component model async export is not production-ready | Synchronous host function returning `result<list<u8>, string>` |
| `tokio::sync::oneshot` channel for `call-service` response | Adds latency and indirection vs. direct `await` | Direct `.await` on `execute_operator_component` inside `func_wrap_async` |
| Separate "continuation engine" crate | Over-engineering for what is ~200 lines of changes | Extend existing `wavs-engine` and `wavs-rig` packages in-place |
| Cross-operator `call-service` (calling a service on a different operator node via network) | Network hop, consensus complexity, far out of scope | Single-node synchronous only; cross-node is a v4+ concern |
| Fuel sharing / accounting across continuation steps for v3.0 MVP | Adds accounting complexity with marginal benefit for initial release | Per-step full fuel budget; add shared accounting later if needed |

---

## Integration Points With Existing Infrastructure

| Existing Mechanism | How v3.0 Integrates |
|-------------------|---------------------|
| `operator.wit` `run` export | Return type changes from `result<list<wasm-response>, string>` to `result<agent-step-result, string>`; `execute.rs` unwraps `done` case; non-agent components wrap responses in `done` |
| `OperatorHostComponent` | Gains `allowed_service_calls` and `Arc<RwLock<Services>>` fields; `call-service` host function registered via existing linker pattern |
| `run_trigger` in `wasm_engine.rs` | Gains step loop; engine drives re-invocation; component sees each step as a fresh `run` call |
| `KeyValueCtx` + `WavsDb` | Continuation state written under `wavs_agent_step:` prefix by engine host side; same bucket/namespace used by `WavsMemory`, no conflict due to distinct key prefixes |
| `WavsAgent` trait in `wavs-rig` | Return type widens to `AgentOutput<T>` enum; `run_agent` maps to new WIT `agent-step-result` variant |
| `Permissions` struct + `service.json` | Gains `allowed_service_calls` with serde default `None`; all existing `service.json` files deserialize without modification |
| `correlation_id: String` on `TriggerAction` | Used as continuation KV key component to isolate state per trigger invocation |
| `AllowedHostPermission` pattern | `AllowedServiceCalls` is structurally identical — `All` / `Only(Vec<ServiceId>)` / `None`; same enforcement pattern in linker |
| Existing `wasmtime` `Config::async_support(true)` | Already set in `BaseEngine::new`; enables `func_wrap_async` for `call-service` host function without config change |

---

## Version Compatibility

| Package | Version | Status | Notes |
|---------|---------|--------|-------|
| `wasmtime` | 42.0.1 | No change | `func_wrap_async` API available; async component model supported |
| `wit-bindgen` | 0.53.1 | No change | Re-run codegen after WIT change; no version bump needed |
| `wasi:keyvalue` | 0.2.0-draft2 | No change | New key prefix only; no API change |
| `wstd` | 0.6.5 | No change | `block_on` unchanged; continuation is host-driven |
| `wasip2` | 1.0.1 | No change | No new WASI APIs needed |

---

## Sources

- Direct inspection of `/workspace/WAVS/packages/engine/src/worlds/operator/execute.rs` — confirmed current `run` call site and `WasmResponse` handling
- Direct inspection of `/workspace/WAVS/packages/engine/src/worlds/instance.rs` — `OperatorHostComponent` struct, `configure_linker`, existing async store setup
- Direct inspection of `/workspace/WAVS/packages/engine/src/worlds/operator/component.rs` — existing host component struct; straightforward to extend
- Direct inspection of `/workspace/WAVS/packages/wavs/src/subsystems/engine/wasm_engine.rs` — `execute_operator_component` async signature; `run_trigger` structure
- Direct inspection of `/workspace/WAVS/packages/wavs-rig/src/agent.rs` — `WavsAgent` trait and `run_agent` — minimal change needed
- Direct inspection of `/workspace/WAVS/packages/wavs-rig/src/memory.rs` — `wavs_agent_memory:` key prefix; confirms no collision with `wavs_agent_step:` prefix
- Direct inspection of `/workspace/WAVS/packages/types/src/service.rs` — `Permissions` struct, `AllowedHostPermission` pattern; `correlation_id` on `TriggerAction`
- Direct inspection of `/workspace/WAVS/wit-definitions/operator/wit/operator.wit` — confirmed existing `host` interface and `run` export; where changes go
- [Wasmtime async host functions](https://docs.wasmtime.dev/examples-async.html) — `func_wrap_async` confirmed working with `Config::async_support(true)` — HIGH confidence
- [WIT reference — variants](https://component-model.bytecodealliance.org/design/wit.html) — variant return types supported in current wit-bindgen 0.53.1 — HIGH confidence
- `.planning/PROJECT.md` — v3.0 target feature list and existing architectural decisions

---
*Stack research for: WAVS v3.0 — agent continuation + synchronous service-to-service RPC*
*Researched: 2026-04-20*
