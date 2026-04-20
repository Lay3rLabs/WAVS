# Architecture Research

**Domain:** WAVS v2.0 — rig-core Agent Runtime Integration
**Researched:** 2026-04-20
**Confidence:** HIGH (all findings from direct source inspection of WAVS codebase + WAVS_AGENT_IMPROVEMENTS.md spec)

## System Overview

The integration adds a new library crate (`packages/wavs-rig/`) that bridges rig-core's agent abstractions into the WASI sandbox. No existing packages are modified during the MVP. The bridge sits entirely on the component side (inside WASM), not on the node side.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          WAVS Node (Tokio / Rust)                        │
│                                                                           │
│  ┌──────────────┐  Crossbeam  ┌──────────────┐  ┌────────────────────┐  │
│  │   Trigger    │────────────▶│  Dispatcher  │──▶│  Engine Manager   │  │
│  │   Manager    │             │  (loop)      │  │  (packages/engine) │  │
│  └──────────────┘             └──────────────┘  └────────┬───────────┘  │
│                                                           │              │
│                                         Wasmtime WASI sandbox per invoke │
│  ┌────────────────────────────────────────────────────────▼───────────┐  │
│  │   WavsWorld instance (operator)                                     │  │
│  │                                                                      │  │
│  │   Host functions exposed via WIT (operator.wit):                    │  │
│  │     wasi:http/outgoing-handler    (AllowedHostPermission enforced)  │  │
│  │     wasi:keyvalue/store|atomics|batch                               │  │
│  │     host::log, host::config-var                                     │  │
│  │     host::get-evm-chain-config, host::get-cosmos-chain-config       │  │
│  │                                                                      │  │
│  │   ┌─────────────────────────────────────────────────────────────┐   │  │
│  │   │   Agent Component (wasm32-wasip2 cdylib)                     │   │  │
│  │   │                                                               │   │  │
│  │   │   fn run(trigger: TriggerAction) -> Result<Vec<WasmResponse>> │   │  │
│  │   │     │                                                          │   │  │
│  │   │     ▼  wstd::runtime::block_on(async { ... })                 │   │  │
│  │   │                                                               │   │  │
│  │   │   ┌─────────────────────────────────────────────────────┐    │   │  │
│  │   │   │   wavs-rig (packages/wavs-rig/ — NEW CRATE)          │    │   │  │
│  │   │   │                                                       │    │   │  │
│  │   │   │   WasiHttpClient (impl HttpClientExt)                 │    │   │  │
│  │   │   │     └── wasi:http/outgoing-handler                    │    │   │  │
│  │   │   │                                                       │    │   │  │
│  │   │   │   rig-wasi fork (git dep or local path)               │    │   │  │
│  │   │   │     ├── Agent<M, T> loop (prompt → tools → response)  │    │   │  │
│  │   │   │     ├── CompletionModel trait (20+ providers)         │    │   │  │
│  │   │   │     └── Tool trait + ToolDefinition (JSON Schema)     │    │   │  │
│  │   │   │                                                       │    │   │  │
│  │   │   │   Built-in Tools:                                      │    │   │  │
│  │   │   │     KvGetTool / KvSetTool  → wasi:keyvalue            │    │   │  │
│  │   │   │     HttpFetchTool          → wasi:http                │    │   │  │
│  │   │   │     EvmQueryTool           → host::get-evm-chain-config│    │   │  │
│  │   │   │     CosmosQueryTool        → host::get-cosmos-chain-config│  │   │  │
│  │   │   │     LogTool                → host::log                │    │   │  │
│  │   │   │                                                       │    │   │  │
│  │   │   │   WavsMemory               → wasi:keyvalue            │    │   │  │
│  │   │   │   WavsAgent trait (developer-facing)                  │    │   │  │
│  │   │   │   run_agent() shim         → wstd::runtime::block_on  │    │   │  │
│  │   │   └─────────────────────────────────────────────────────┘    │   │  │
│  │   └─────────────────────────────────────────────────────────────┘   │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

## Component Boundaries

### Existing Components — Unchanged for MVP

| Component | Location | Role in Agent Flow |
|-----------|----------|--------------------|
| `packages/engine` | `packages/engine/` | Instantiates WASM, injects host functions, enforces fuel+time limits. No changes — the agent component is just another `run()` export. |
| `packages/types` | `packages/types/src/service.rs` | `Component.permissions.allowed_http_hosts` (`AllowedHostPermission::All/Only/None`) is how the operator constrains which LLM API the agent may call. No changes needed. |
| `packages/wasi-utils` | `packages/wasi-utils/` | Existing HTTP helpers (`wstd::http::Client`) are the underlying call path for `WasiHttpClient`. Not modified; `wavs-rig` wraps them. |
| `examples/components/_helpers` | `examples/components/_helpers/` | The WIT bindings generation pattern (`wit_bindgen::generate!`, `export_layer_trigger_world!`) is directly reused for the agent example component. Not modified. |
| `wit-definitions/operator/wit/operator.wit` | `wit-definitions/operator/wit/` | Defines the host surface the agent component uses. All required host functions already exist: `wasi:http`, `wasi:keyvalue`, `host::log`, `host::config-var`, chain configs. No WIT changes needed for MVP. |

### New Components

| Component | Location | Purpose |
|-----------|----------|---------|
| `packages/wavs-rig` | `packages/wavs-rig/` | Bridge library: `WasiHttpClient`, built-in tools, `WavsMemory`, `WavsAgent` trait, `run_agent()` shim. Compiled to `wasm32-wasip2`. |
| `rig-wasi` fork | Git dependency or `packages/rig-wasi/` | Thin fork of rig-core (~300-500 line patch). Makes `reqwest` optional, drops `tokio::rt`, unifies `cfg` detection for wasip2. |
| `examples/components/agent-defi-monitor` | `examples/components/agent-defi-monitor/` | Example agent component demonstrating the full loop. ~30 lines of domain logic using `WavsAgent` trait. |

### Workspace Registration

The two new Cargo workspace members must be added to `WAVS/Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "packages/wavs-rig",
    "examples/components/agent-defi-monitor",
]
```

`rig-wasi` is a dependency of `wavs-rig`, not a workspace member. It is referenced via a git path dep in `packages/wavs-rig/Cargo.toml`.

## Data Flow: Trigger to Agent Reasoning to Result

```
1. On-chain event fires (EVM log / cron / block interval)
       │
       ▼
2. TriggerManager captures TriggerData → sends DispatcherCommand::Trigger(TriggerAction)
       │
       ▼
3. Dispatcher → EngineCommand::ExecuteOperator { trigger_action, component, ... }
       │
       ▼
4. Engine creates InstanceDeps:
     - OperatorHostComponent { service, workflow_id, permissions, keyvalue_ctx, http_ctx }
     - Linker populated with wasi:http (if AllowedHostPermission != None), wasi:keyvalue, host functions
     │
       ▼
5. Engine calls component export: run(trigger_action) → tokio::time::timeout wraps the call
       │
       ▼ (inside WASM sandbox)
6. Agent component fn run():
     a. Deserialize trigger_action.data to extract prompt context
     b. Call wavs_rig::run_agent(agent, trigger) which calls wstd::runtime::block_on(async { ... })
     c. Inside block_on:
          i.   Build rig Agent<M, T> via WavsAgent::build_rig_agent()
          ii.  Load conversation history from WavsMemory (wasi:keyvalue bucket)
          iii. Convert trigger data to prompt string via WavsAgent::trigger_to_prompt()
          iv.  Call rig_agent.chat(prompt, history).await
               │
               ▼
               rig agent loop (multi-turn):
                 - send messages to LLM via WasiHttpClient.send_request()
                   → wasi:http/outgoing-handler → WAVS AllowedHostPermission check
                 - LLM responds with tool_use or text
                 - if tool_use: dispatch to registered rig Tool impl
                   → KvGetTool/KvSetTool: wasi:keyvalue host function
                   → HttpFetchTool: wasi:http host function
                   → EvmQueryTool: host::get-evm-chain-config + HTTP call
                   → LogTool: host::log host function
                 - append tool result to history
                 - loop continues until text response or max_turns reached
          v.   Append new messages to WavsMemory (persists for next invocation)
          vi.  Convert rig response to Vec<WasmResponse> via WavsAgent::response_to_wasm()
     d. Return Ok(Vec<WasmResponse>)
       │
       ▼ (back on host)
7. Engine validates response sizes (DEFAULT_MAX_PAYLOAD_SIZE = 50MB)
       │
       ▼
8. EngineResponse::Operator(SubmissionRequest { responses }) → Dispatcher
       │
       ▼
9. Dispatcher → Aggregator → on-chain submission (existing flow, unchanged)
```

## The Four Bridges — Implementation Detail

### Bridge 1: HTTP Transport (WasiHttpClient)

rig-core's `HttpClientExt` trait is the single seam for swapping the HTTP backend. The implementation routes all LLM API traffic through WAVS's existing `wasi:http/outgoing-handler`:

```rust
// packages/wavs-rig/src/http.rs
struct WasiHttpClient;

impl HttpClientExt for WasiHttpClient {
    async fn send_request(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        // Uses wstd::http::Client under the hood — same as existing wavs-wasi-utils
        let wstd_req = convert_to_wstd_request(req)?;
        let mut resp = wstd::http::Client::new().send(wstd_req).await
            .map_err(|e| HttpError::Send(e.to_string()))?;
        convert_from_wstd_response(&mut resp).await
    }
}
```

The critical property: `AllowedHostPermission` is enforced at the Wasmtime linker level in `configure_linker()` (`packages/engine/src/worlds/instance.rs`). If `permissions.allowed_http_hosts == AllowedHostPermission::None`, `wasmtime_wasi_http::add_only_http_to_linker_async` is NOT called, and any attempt to call `wasi:http` traps. An agent component with `Only(["api.anthropic.com"])` can only reach Claude, enforced by the sandbox.

### Bridge 2: WAVS Host Functions as Rig Tools

Each tool wraps a WASI host function call. The rig `Tool` trait requires `NAME: &'static str`, associated `Args`/`Output`/`Error` types with JSON Schema, `definition()`, and `call()`. The JSON Schema is auto-derived via `schemars::JsonSchema` on the `Args` struct.

Location for all built-in tools: `packages/wavs-rig/src/tools/`

```
tools/
├── mod.rs         (re-exports all tools)
├── kv.rs          (KvGetTool, KvSetTool)
├── http.rs        (HttpFetchTool)
├── evm.rs         (EvmQueryTool)
├── cosmos.rs      (CosmosQueryTool)
└── log.rs         (LogTool)
```

### Bridge 3: Async Runtime Shim

WASM components use `wstd::runtime::block_on` as their async executor. The shim is the `run_agent()` function that wraps the entire agent loop inside one `block_on` call:

```rust
// packages/wavs-rig/src/agent.rs
pub fn run_agent<A: WavsAgent>(
    agent: A,
    trigger: TriggerAction,
) -> Result<Vec<WasmResponse>, String> {
    wstd::runtime::block_on(async {
        let config = agent.build(trigger.clone())?;
        let rig_agent = build_rig_agent_from_config(config)?;
        let prompt = agent.trigger_to_prompt(trigger)?;
        let response = rig_agent.prompt(&prompt).await
            .map_err(|e| e.to_string())?;
        agent.response_to_wasm(response)
    })
}
```

The rig agent loop itself is pure async with `futures::StreamExt` — it does not require the tokio runtime. The tokio `rt` feature is removed in the rig-wasi fork. Sequential tool execution (rig concurrency = 1) avoids `futures::stream::buffer_unordered` which would require a multi-task executor.

### Bridge 4: KV-Backed Conversation Memory

`WavsMemory` persists multi-turn conversation history across invocations using `wasi:keyvalue`:

```rust
// packages/wavs-rig/src/memory.rs
pub struct WavsMemory {
    bucket_id: String,
    max_tokens: usize,
}
```

History is serialized as JSON into a single KV key per agent instance. `trim_to_budget()` keeps the system message and the N most recent messages fitting within `max_tokens`. This is stateless across the agent object — the entire persistence is in the KV store, which is per-service in the engine's `KeyValueCtx`.

## New Crate: packages/wavs-rig

### Cargo.toml Dependencies

```toml
[package]
name = "wavs-rig"
# ...workspace fields...

[dependencies]
# The rig-wasi fork — thin fork of rig-core for wasip2 compatibility
rig-core = { git = "https://github.com/[fork]/rig", rev = "[pin]", default-features = false }

# WASI runtime (no tokio)
wstd = { workspace = true }
wasip2 = { workspace = true }

# WIT bindings (same as other components)
wit-bindgen = { workspace = true }

# WAVS types for WasmResponse, TriggerAction
wavs-wasi-utils = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }
schemars = "0.8"   # for Tool JSON Schema derivation

[lib]
crate-type = ["rlib"]   # Library only — not a cdylib itself
```

Note: `wavs-rig` is compiled as `rlib`, not `cdylib`. The agent example component is the `cdylib` that depends on `wavs-rig`. This mirrors the existing `example-helpers` pattern.

### Module Structure

```
packages/wavs-rig/src/
├── lib.rs          (pub use, crate-level exports)
├── agent.rs        (WavsAgent trait, AgentConfig, run_agent())
├── http.rs         (WasiHttpClient: impl HttpClientExt)
├── memory.rs       (WavsMemory: KV-backed conversation history)
└── tools/
    ├── mod.rs
    ├── kv.rs
    ├── http.rs
    ├── evm.rs
    ├── cosmos.rs
    └── log.rs
```

## Example Agent Component: examples/components/agent-defi-monitor

This is the MVP example demonstrating the complete loop. It follows exactly the same structure as existing example components:

```
examples/components/agent-defi-monitor/
├── Cargo.toml           (depends on wavs-rig, example-helpers)
└── src/
    └── lib.rs           (~30 lines of domain logic)
```

`Cargo.toml` structure mirrors `kv-store`:

```toml
[lib]
crate-type = ["rlib", "cdylib"]

[package.metadata.component]
package = "wavs:agent-defi-monitor"
```

The component uses `export_layer_trigger_world!(Component)` from `example-helpers/src/bindings/world.rs`, exactly as all existing examples do. The WIT world (`wavs-world`) is unchanged — agents export the same `run(trigger-action) -> result<list<wasm-response>, string>` interface.

## rig-wasi Fork: Required Changes

The fork patches rig-core to compile on `wasm32-wasip2`. All changes are in the platform/compat layer:

| File | Change | Effort |
|------|--------|--------|
| `Cargo.toml` | `reqwest = { ..., optional = true }`, drop `tokio = { features = ["rt"] }` | Trivial |
| `http_client.rs` | Gate `reqwest::Client` default behind `#[cfg(feature = "reqwest")]` | Small |
| `client/mod.rs` | `ClientBuilderError` must not reference `reqwest::Error` unconditionally | Small |
| `streaming.rs` | Replace `tokio::sync::watch` in `PauseControl` with `futures::channel::watch` or remove | Small |
| `wasm_compat.rs` | Unify `WasmBoxedFuture` and `WasmCompatSend` cfg gates to use `target_family = "wasm"` | Small |
| `sse.rs` | Add wasip2 dead-zone cfg branch (neither `wasm32` nor `wasm feature` fires on wasip2) | Small |
| `Cargo.toml` | Add `getrandom` with `wasi` feature for `wasm32-wasip2` target | Trivial |

Total: ~300-500 lines across 6-7 files. No changes to the agent loop, tool dispatch, or provider implementations.

## Architectural Patterns

### Pattern 1: WIT World is the Sandbox Boundary

**What:** The `wavs-world` WIT definition (operator.wit) is the contract between the host and the component. Everything an agent can do is expressed as a WIT host import. The agent loop inside WASM calls these imports; the Wasmtime linker provides their implementations.

**When to use:** Any new capability for agents (e.g., `call-service` for inter-component RPC in post-MVP) must be added as a WIT host import, not as Rust code in the component crate.

**Trade-offs:** Discipline required — no "reaching out" from the component outside the WIT surface. This is the guarantee that makes agents trustworthy.

### Pattern 2: rlib + cdylib Split

**What:** Shared logic (`wavs-rig`, `example-helpers`) is compiled as `rlib`. Only the final component that exports the WIT world is `cdylib`. The `wavs-rig` crate is an rlib — the agent component crate is the cdylib.

**When to use:** Always. Matches the existing pattern for all example components.

**Trade-offs:** Requires two crates per agent (the library and the component). The convention is clear and already established.

### Pattern 3: Component Config for API Keys

**What:** LLM provider API keys are injected via `Component.env_keys` (system env vars prefixed `WAVS_ENV_`) and `Component.config` (key-value pairs in service.json). Inside WASM, retrieved via `host::config-var(key)`. The agent reads `WAVS_ENV_ANTHROPIC_API_KEY` at runtime.

**When to use:** All secrets and provider configuration. Never hardcode API keys in WASM bytes.

**Trade-offs:** Requires operator to set env vars. Future work (post-MVP P2): first-class API key management in the Tauri app UI.

### Pattern 4: Sequential Tool Execution for MVP

**What:** Configure rig's concurrency to 1 (sequential tool calls). Rig supports `buffer_unordered` for parallel tool calls, but this requires multi-task executor semantics. WASI is single-threaded; `wstd::runtime::block_on` drives a single-task async executor.

**When to use:** MVP. Sequential is correct for the common case (most agent tool chains are sequential by nature).

**Trade-offs:** Parallel tool execution is deferred. For post-MVP, the engine-level `Continue/Checkpoint` variant (from `WAVS_AGENT_IMPROVEMENTS.md` post-MVP section) can provide external parallelism.

## Anti-Patterns

### Anti-Pattern 1: Modifying the Engine for Agent Support

**What:** Adding agent-specific logic to `packages/engine`, `packages/wavs`, or the WAVS node to "support" agents as a special execution mode.

**Why it's wrong:** Agents are just WASM components that export `run()`. The engine doesn't know or care that the component inside is running an LLM loop. The entire rig integration lives inside the WASM boundary. Adding engine changes for MVP breaks this clean separation.

**Do this instead:** Put all agent logic in `packages/wavs-rig` (inside the WASM side). The engine sees the agent component identically to any other component.

### Anti-Pattern 2: Rebuilding the Rig Agent Loop

**What:** Writing a custom LLM dispatch loop inside `wavs-rig` instead of using rig's existing `Agent<M, T>`.

**Why it's wrong:** Rig already has multi-turn loops, tool dispatch, provider implementations for 20+ LLMs, structured output, and prompt hooks. Option C (minimal extraction) from the spec estimates this as "higher upfront effort" with "no fork maintenance burden" — but months of work to reach parity.

**Do this instead:** Fork rig-core (Option B). ~300-500 lines of platform patches. Upstream later if accepted.

### Anti-Pattern 3: Using tokio::rt in the WASI Component

**What:** Including `tokio = { features = ["rt"] }` in the agent component or `wavs-rig` dependencies, then using `tokio::runtime::Runtime::new().block_on(...)`.

**Why it's wrong:** The tokio `rt` feature requires `std::thread` primitives that don't exist on `wasm32-wasip2`. This is one of the hard blockers documented in the rig investigation.

**Do this instead:** Use `wstd::runtime::block_on` exclusively. The rig agent loop is pure async and does not require tokio's runtime internals.

### Anti-Pattern 4: Separate Crate for Each Agent

**What:** Creating a new workspace crate per agent type (e.g., `packages/wavs-rig-defi`, `packages/wavs-rig-oracle`).

**Why it's wrong:** The library (`wavs-rig`) provides all the infrastructure. Each agent is a thin `cdylib` component in `examples/components/`. Adding them as workspace members inflates the workspace root.

**Do this instead:** Follow the established pattern — one `rlib` library crate, many lightweight `cdylib` example components that depend on it.

## Integration Points with Existing Code

### What wavs-rig Calls (Direct WASI host imports)

| Host Function | WIT Location | wavs-rig Uses It For |
|--------------|-------------|----------------------|
| `wasi:http/outgoing-handler` | operator.wit | `WasiHttpClient` (LLM API calls, `HttpFetchTool`) |
| `wasi:keyvalue/store` | operator.wit (via include wasi:keyvalue) | `KvGetTool`, `KvSetTool`, `WavsMemory` |
| `wasi:keyvalue/atomics` | operator.wit | Optional atomic ops in tools |
| `host::log` | operator.wit (inline interface) | `LogTool` |
| `host::config-var` | operator.wit (inline interface) | Reading API keys, model names from service config |
| `host::get-evm-chain-config` | operator.wit (inline interface) | `EvmQueryTool` — gets RPC URL for EVM chain |
| `host::get-cosmos-chain-config` | operator.wit (inline interface) | `CosmosQueryTool` |

All of these host functions are already implemented in `packages/engine/src/bindings/operator/host.rs` and `packages/engine/src/backend/wasi_keyvalue/`. No engine changes needed.

### What the Agent Component Exports (Unchanged WIT Contract)

```wit
export run: func(trigger-action: trigger-action) -> result<list<wasm-response>, string>;
```

This is identical to all existing WAVS components. The engine calls `call_run()` on the instantiated component. The agent is just a component.

### Permission Model: AllowedHostPermission Controls LLM Access

`Permissions.allowed_http_hosts` in `service.json` governs what the agent component can call over HTTP:

```json
{
  "permissions": {
    "allowed_http_hosts": { "only": ["api.anthropic.com"] }
  }
}
```

This is enforced in `configure_linker()` at `packages/engine/src/worlds/instance.rs:350-355`. If `AllowedHostPermission::None`, `wasi:http` is not linked into the component's sandbox — HTTP calls trap. If `Only(["api.anthropic.com"])`, the check is currently a coarse gate (the FIXME on line 352 notes that per-host allowlisting requires WAT-level inspection). For MVP, `All` or `Only` enables HTTP; `None` disables it entirely.

### Component Fuel and Time Limits

An agent making 10 LLM API calls (each 5-10 seconds) may need 50-100 seconds of wall time. The default `Workflow::DEFAULT_TIME_LIMIT_SECONDS` is `u64::MAX` — no limit unless explicitly set. The `fuel_limit` is `Workflow::DEFAULT_FUEL_LIMIT = u64::MAX`. For agent components, the service.json `time_limit_seconds` should be explicitly set (e.g., 120-300 seconds) to prevent runaway loops.

## Recommended Build Order

The correct build order follows the dependency chain:

1. **rig-wasi fork** — The foundational dependency. Apply the ~300-500 line patches (reqwest optional, tokio::rt dropped, cfg unification). Verify it compiles for `wasm32-wasip2`. No WAVS changes needed.

2. **packages/wavs-rig** — The bridge library. Implement the four bridges (WasiHttpClient, built-in tools, run_agent shim, WavsMemory). Compile target: `wasm32-wasip2`. Depends on: rig-wasi fork.

3. **examples/components/agent-defi-monitor** — The example component. Demonstrates the full loop: trigger → WavsAgent::build → prompt → rig agent loop → tool calls → WasmResponse. Depends on: wavs-rig, example-helpers (existing). Build with `just wasi-build-native agent-defi-monitor`.

4. **Workspace Cargo.toml** — Add the two new workspace members after both crates exist and compile.

5. **End-to-end test** — Deploy the agent component via existing `dev-tool deploy-service`, send a trigger, observe LLM reasoning in the activity feed logs.

### Dependencies Between Steps

```
rig-wasi fork
    │
    ▼
packages/wavs-rig (rlib, wasm32-wasip2)
    │
    ▼
examples/components/agent-defi-monitor (cdylib, wasm32-wasip2)
    │
    ▼
service.json (AllowedHostPermission: Only([LLM provider URL]))
    │
    ▼
WAVS node execution (no node changes needed)
```

Steps 1-3 have strict ordering. Step 5 can only run after all prior steps succeed.

## Modified vs New Components Summary

### New Files

| File | Type | Purpose |
|------|------|---------|
| `packages/wavs-rig/Cargo.toml` | New | Crate manifest |
| `packages/wavs-rig/src/lib.rs` | New | Public API surface |
| `packages/wavs-rig/src/agent.rs` | New | `WavsAgent` trait, `AgentConfig`, `run_agent()` |
| `packages/wavs-rig/src/http.rs` | New | `WasiHttpClient: impl HttpClientExt` |
| `packages/wavs-rig/src/memory.rs` | New | `WavsMemory` KV-backed history |
| `packages/wavs-rig/src/tools/mod.rs` | New | Tool module root |
| `packages/wavs-rig/src/tools/kv.rs` | New | `KvGetTool`, `KvSetTool` |
| `packages/wavs-rig/src/tools/http.rs` | New | `HttpFetchTool` |
| `packages/wavs-rig/src/tools/evm.rs` | New | `EvmQueryTool` |
| `packages/wavs-rig/src/tools/cosmos.rs` | New | `CosmosQueryTool` |
| `packages/wavs-rig/src/tools/log.rs` | New | `LogTool` |
| `examples/components/agent-defi-monitor/Cargo.toml` | New | Example component manifest |
| `examples/components/agent-defi-monitor/src/lib.rs` | New | ~30 lines of agent domain logic |
| `rig-wasi/` (fork) | New repo or git dep | Patched rig-core for wasip2 |

### Modified Files

| File | Change | Why |
|------|--------|-----|
| `WAVS/Cargo.toml` | Add `packages/wavs-rig` and `examples/components/agent-defi-monitor` to `[workspace.members]` | Register new crates |

**All other existing files are unchanged for MVP.** The engine, dispatcher, aggregator, submission, trigger manager, WIT definitions, and Tauri app require no modifications.

---

## Sources

All findings from direct source inspection (HIGH confidence):

- `/workspace/WAVS/packages/engine/src/worlds/instance.rs` — `configure_linker()`, `InstanceDepsBuilder::build()`, permission enforcement
- `/workspace/WAVS/packages/engine/src/bindings/operator/host.rs` — all host function implementations
- `/workspace/WAVS/packages/engine/src/worlds/operator/execute.rs` — component invocation, timeout handling
- `/workspace/WAVS/packages/engine/src/worlds/operator/component.rs` — `OperatorHostComponent` with all WASI contexts
- `/workspace/WAVS/packages/engine/src/common/base_engine.rs` — component loading, fuel/time limit defaults
- `/workspace/WAVS/packages/types/src/service.rs` — `AllowedHostPermission`, `Component`, `Workflow::DEFAULT_FUEL_LIMIT`
- `/workspace/WAVS/wit-definitions/operator/wit/operator.wit` — WIT world surface, all host imports
- `/workspace/WAVS/packages/wasi-utils/src/http.rs` — existing HTTP helper pattern (`wstd::http::Client`)
- `/workspace/WAVS/examples/components/_helpers/src/bindings/world.rs` — WIT bindings generation pattern
- `/workspace/WAVS/examples/components/_helpers/src/trigger.rs` — trigger decode/encode pattern
- `/workspace/WAVS/examples/components/kv-store/src/lib.rs` — reference component structure
- `/workspace/WAVS/examples/components/kv-store/Cargo.toml` — reference component manifest
- `/workspace/WAVS/Cargo.toml` — workspace structure, existing deps
- `/workspace/WAVS_AGENT_IMPROVEMENTS.md` — detailed architecture spec for all four bridges, rig fork analysis, sequencing

---
*Architecture research for: WAVS v2.0 — rig-core Agent Runtime Integration*
*Researched: 2026-04-20*
