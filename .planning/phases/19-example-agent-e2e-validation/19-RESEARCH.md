# Phase 19: Example Agent & E2E Validation - Research

**Researched:** 2026-04-20
**Domain:** WASI component authoring — composing wavs-rig into a runnable example agent with E2E deployment validation
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- Agent component lives in `examples/components/agent-example/` (follows existing example pattern)
- ~30 lines domain logic — receive trigger, call LLM with prompt, use at least one tool (e.g., KvSetTool to store reasoning), return structured JSON result
- Uses wavs-rig's `run_agent` shim, `WasiHttpClient`, built-in tools
- LLM provider: Anthropic (`api.anthropic.com`) — aligns with AllowedHostPermission::Only requirement
- API key passed via environment/config, not hardcoded
- `service.json` uses `AllowedHostPermission::Only(["api.anthropic.com"])`
- Component deployed as standard WAVS service
- Trigger: manual trigger (simplest for demo)
- Deploy via wavs-mcp or CLI
- Send trigger, observe structured result
- Verify non-listed hosts are blocked (negative test)

### Claude's Discretion

- Exact agent prompt and reasoning task
- Which tool(s) the agent uses in the demo
- Service name and trigger configuration details
- Test structure and validation approach

### Deferred Ideas (OUT OF SCOPE)

- Agent continuation mode (multi-step) — v3.0
- Template gallery for agent examples — future
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| E2E-01 | Example agent component (~30 lines domain logic) demonstrates full agent loop: trigger → LLM reasoning → tool use → structured result | Direct: echo-data + kv-store patterns verified; wavs-rig run_agent shim ready in packages/wavs-rig/src/agent.rs |
| E2E-02 | Agent deployed and executed end-to-end on a live WAVS node (trigger fires, agent reasons, result returned) | Direct: manual trigger pattern (TriggerData::Raw) verified in echo-data; service.json format confirmed from wavs-foundry-template; deploy via CLI or wavs-mcp |
| E2E-03 | service.json uses AllowedHostPermission::Only(["api.anthropic.com"]) demonstrating sandboxed LLM access | Direct: AllowedHostPermission::Only serde format confirmed as `{"only": ["api.anthropic.com"]}`; engine FIXME noted — Only acts as All at runtime (host enforcement incomplete); agent startup check via check_http_permission |
</phase_requirements>

---

## Summary

Phase 19 creates `examples/components/agent-example/`, a complete `cdylib` WASI component that ties together everything built in Phases 17 and 18. The component wires the `WavsAgent` trait, `run_agent` shim, `WasiHttpClient`, and at least one built-in tool into a ~30-line domain logic body demonstrating the full trigger → LLM reasoning → tool use → structured result loop.

All the building blocks are in place and verified. The `wavs-rig` crate (`packages/wavs-rig/`) compiles cleanly to `wasm32-wasip2`. The Anthropic provider in `rig-wasi` uses `ClientBuilder<AnthropicBuilder, AnthropicKey, H>` which accepts any `H: HttpClientExt + Default` — so `WasiHttpClient` plugs in via `.http_client(WasiHttpClient::default())`. The component structure exactly mirrors `examples/components/kv-store/` (imports `example-helpers`, exports the world macro, implements `Guest::run`, calls `block_on` exactly once via `run_agent`).

One critical engine limitation exists: `AllowedHostPermission::Only(hosts)` is declared in `service.json` but the WAVS engine currently only gates on None vs non-None (the `Only` host filter has a `// FIXME` comment in `packages/engine/src/worlds/instance.rs`). The `Only` variant correctly communicates intent and will pass through to the component via `host::get_service()`, but actual runtime host-blocking for non-listed hosts is not yet enforced by the Wasmtime linker. The agent startup call to `check_http_permission` validates permission is non-None. The negative test must acknowledge this gap.

**Primary recommendation:** Create the agent example as a `cdylib` crate in `examples/components/agent-example/`. Use `ClientBuilder::<AnthropicBuilder, _, WasiHttpClient>::default().api_key(api_key).build()` pattern. Implement `WavsAgent` for a struct. Call `run_agent` as the sole `block_on` boundary inside `Guest::run`. Set `AllowedHostPermission::Only(["api.anthropic.com"])` in `service.json` with `"env_keys": ["WAVS_ENV_ANTHROPIC_API_KEY"]`.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `wavs-rig` (local) | workspace | Agent integration shim: `run_agent`, `WavsAgent`, `WasiHttpClient`, tools, memory | Phase 18 output — fully verified against wasm32-wasip2 |
| `rig-wasi` (local) | workspace | Anthropic provider, `AgentBuilder`, `Tool` trait | Phase 17 output — patched fork of rig-core 0.35.0 for WASI |
| `example-helpers` (local) | workspace | `Guest` trait, `TriggerAction`, `WasmResponse`, `export_layer_trigger_world!`, `decode_trigger_event`, `encode_trigger_output` | All WAVS example components use this crate |
| `wstd` | 0.6.5 [VERIFIED: workspace Cargo.toml] | WASI async runtime (`block_on`) — called by `run_agent` | WAVS WASI components standard runtime |
| `serde` / `serde_json` | workspace | Structured result type serialization | Standard WAVS convention |
| `anyhow` | workspace | Error propagation | Standard WAVS convention |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `wavs-wasi-utils` | workspace | Optional EVM helpers if EvmQueryTool is demonstrated | Only if Evm tool use is chosen for demo |

**Installation:** No new dependencies — all are existing workspace crates. The only new artifact is `examples/components/agent-example/`.

**Crate-type:** `["cdylib"]` — matches all other example components (not `["rlib", "cdylib"]` like `permissions`).

---

## Architecture Patterns

### Recommended Project Structure
```
examples/components/agent-example/
├── Cargo.toml          # cdylib; deps: wavs-rig, rig-wasi, example-helpers, serde, serde_json, anyhow
└── src/
    └── lib.rs          # ~30 lines domain logic + boilerplate
```

### Pattern 1: Component Structure (follows echo-data / kv-store)
**What:** Every example component is a `cdylib` that implements `Guest::run`, exports the world macro, and calls `block_on` exactly once.
**When to use:** Always. This is the mandatory WAVS component entry-point pattern.
**Example:**
```rust
// Source: examples/components/kv-store/src/lib.rs (verified)
use example_helpers::prelude::*;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        // parse trigger, do work, return responses
        // block_on called exactly once (by run_agent)
    }
}

export_layer_trigger_world!(Component);
```

### Pattern 2: Agent Component (new pattern — Phase 19)
**What:** Implement `WavsAgent` on a struct, pass the struct to `wavs_rig::run_agent`. The `run_agent` function contains the sole `block_on` call.
**When to use:** All LLM agent components. Never nest `block_on` inside `WavsAgent::run`.
**Example:**
```rust
// Source: packages/wavs-rig/src/agent.rs (verified)
use wavs_rig::{WavsAgent, run_agent, WasiHttpClient, check_http_permission, HttpPermission};
use rig::providers::anthropic;

struct WeatherAgent { api_key: String }

impl WavsAgent for WeatherAgent {
    type Output = serde_json::Value;
    async fn run(&self, trigger_data: Vec<u8>) -> anyhow::Result<serde_json::Value> {
        let client = anthropic::ClientBuilder::<_, _, WasiHttpClient>::default()
            .api_key(&self.api_key)
            .build()?;
        let agent = client.agent("claude-3-5-haiku-latest")
            .preamble("You are a helpful assistant.")
            .tool(KvSetTool)
            .build();
        let prompt = String::from_utf8_lossy(&trigger_data).to_string();
        let response = agent.prompt(&prompt).await?;
        Ok(serde_json::json!({ "result": response }))
    }
}

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        // 1. Check HTTP permission at startup
        let workflow = host::get_service()... // get component permissions
        check_http_permission(&permission)?;
        // 2. Extract API key from env
        let api_key = std::env::var("WAVS_ENV_ANTHROPIC_API_KEY")
            .map_err(|_| "ANTHROPIC_API_KEY not set")?;
        // 3. Build agent and run
        let agent = WeatherAgent { api_key };
        let raw_data = match trigger_action.data {
            TriggerData::Raw(data) => data,
            _ => return Err("expected Raw trigger".into()),
        };
        let output_bytes = run_agent(&agent, raw_data)?;
        Ok(vec![WasmResponse { payload: output_bytes, ordering: None, event_id_salt: None }])
    }
}
```

### Pattern 3: Anthropic ClientBuilder for WASI
**What:** WASI components cannot use `Client::new()` (gated on `#[cfg(feature = "reqwest")]`). Use `ClientBuilder::default()` with explicit `.http_client(WasiHttpClient::default())`.
**When to use:** Always in WASI agent components. The `with_http_client` method is `fn http_client<U>(self, http_client: U) -> ClientBuilder<Ext, ApiKey, U>` — it switches the H type parameter.
**Example:**
```rust
// Source: packages/rig-wasi/src/client/mod.rs (verified — http_client method at line 585)
// and packages/wavs-rig/src/http.rs (WasiHttpClient verified)
use rig::providers::anthropic;
use wavs_rig::WasiHttpClient;

let client = anthropic::ClientBuilder::default()
    .api_key(api_key)            // -> ClientBuilder<AnthropicBuilder, AnthropicKey, ()>
    .http_client(WasiHttpClient::default())  // -> ClientBuilder<AnthropicBuilder, AnthropicKey, WasiHttpClient>
    .build()?;
```

### Pattern 4: Reading Permissions from Host
**What:** The component reads its own `AllowedHostPermission` from the WIT host, maps it to `HttpPermission`, and passes it to `check_http_permission`.
**When to use:** At agent startup, before any LLM calls.
**Example:**
```rust
// Source: packages/engine/src/bindings/types/wavs_to_component.rs (verified — same variant names)
// and packages/wavs-rig/src/permissions.rs (check_http_permission verified)
use example_helpers::bindings::world::{host, wavs::types::service::AllowedHostPermission};
use wavs_rig::{HttpPermission, check_http_permission};

let service_info = host::get_service();
let workflow = service_info.service.workflows.into_iter()
    .find(|(id, _)| *id == service_info.workflow_id)
    .map(|(_, w)| w)
    .ok_or("workflow not found")?;

let http_perm = match workflow.component.permissions.allowed_http_hosts {
    AllowedHostPermission::All => HttpPermission::All,
    AllowedHostPermission::None => HttpPermission::None,
    AllowedHostPermission::Only(hosts) => HttpPermission::Only(hosts),
};
check_http_permission(&http_perm).map_err(|e| e)?;
```

### Pattern 5: Manual Trigger Data Handling
**What:** For manual triggers (simplest demo), `TriggerData::Raw(Vec<u8>)` carries the prompt bytes directly. No `decode_trigger_event` needed.
**When to use:** Manual triggers only. EVM/Cosmos triggers need `decode_trigger_event`.
**Example:**
```rust
// Source: examples/components/echo-data/src/lib.rs (verified — TriggerData::Raw branch)
let prompt_bytes = match trigger_action.data {
    TriggerData::Raw(data) => data,
    _ => return Err("agent-example expects Raw trigger data with prompt text".into()),
};
```

### Pattern 6: service.json AllowedHostPermission::Only Format
**What:** `AllowedHostPermission` uses `serde(rename_all = "snake_case")`. Unit variants serialize as strings, tuple variants as objects.
**Example:**
```json
"permissions": {
    "allowed_http_hosts": { "only": ["api.anthropic.com"] },
    "file_system": false,
    "raw_sockets": false,
    "dns_resolution": false
}
```
[VERIFIED: packages/types/src/service.rs — `#[serde(rename_all = "snake_case")]` on `AllowedHostPermission` enum with `Only(Vec<String>)` variant]

### Pattern 7: Environment Variable for API Key
**What:** WAVS exposes environment variables to components via `env_keys`. Keys must be prefixed with `WAVS_ENV_`. The component reads them via `std::env::var("WAVS_ENV_<KEY>")`.
**Example:**
```json
// service.json
"env_keys": ["WAVS_ENV_ANTHROPIC_API_KEY"]
```
```rust
// In component
let api_key = std::env::var("WAVS_ENV_ANTHROPIC_API_KEY")
    .map_err(|_| "WAVS_ENV_ANTHROPIC_API_KEY not set".to_string())?;
```
[VERIFIED: packages/types/src/lib.rs — `WAVS_ENV_PREFIX = "WAVS_ENV"`; packages/layer-tests/src/e2e/helpers.rs — env_keys format confirmed]

### Anti-Patterns to Avoid
- **Nested block_on:** Never call `wstd::runtime::block_on` inside `WavsAgent::run`. The `run_agent` function is the sole executor boundary. Deadlock guaranteed.
- **Client::new() on WASI:** `anthropic::Client::new()` is `#[cfg(feature = "reqwest")]` gated. Use `ClientBuilder::default().api_key(...).http_client(WasiHttpClient::default()).build()`.
- **Using crate-type = ["rlib"]:** The agent-example is a component binary (`cdylib`), not a library. wavs-rig is the rlib; agent-example is the cdylib.
- **Hardcoding API key:** Never embed API key in source. Use `env_keys` + `std::env::var`.
- **Using ProviderClient::from_env():** `from_env()` calls `Client::new()` which is reqwest-gated. Not available on WASI.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async executor boundary | Custom block_on wrapper | `wavs_rig::run_agent` | Single executor constraint; nested block_on deadlocks in WASI |
| HTTP transport for LLM | Custom wstd HTTP call | `WasiHttpClient` + rig Anthropic provider | Type-safe, handles auth headers, request/response mapping done |
| API permission check | Custom permission logic | `wavs_rig::check_http_permission` | Clear error message; pattern established in Phase 18 |
| Tool schema generation | Manual JSON Schema | `rig::tool::Tool` trait with `schemars` | Tool dispatch, argument deserialization, schema all handled |
| KV storage from tool | Direct wasi:keyvalue call | `wavs_rig::tools::KvSetTool` / `KvGetTool` | Typed args, error handling, namespaced keys done |
| Conversation memory | Manual KV JSON encoding | `wavs_rig::WavsMemory` | Token budget, append/retrieve/truncate already implemented |

---

## Common Pitfalls

### Pitfall 1: `AllowedHostPermission::Only` Does Not Actively Block Non-Listed Hosts at Runtime
**What goes wrong:** Developer expects the WAVS engine to actively reject HTTP calls to `google.com` when `Only(["api.anthropic.com"])` is configured. Negative test for "blocks non-listed hosts" fails because the engine allows all HTTP if permission is non-None.
**Why it happens:** `packages/engine/src/worlds/instance.rs` has `// FIXME: we need to apply Only(host) checks as well`. The engine only tests `!= AllowedHostPermission::None` before adding HTTP to the linker. [VERIFIED: packages/engine/src/worlds/instance.rs — FIXME comment with "involves some wat magic"]
**How to avoid:** Design the negative test to verify what IS enforced: (1) `AllowedHostPermission::None` returns clear error from `check_http_permission`, (2) the service.json `only` field is correctly parsed and passed through to the component. Note in test documentation that active host filtering is an engine-level TODO, not a Phase 19 deliverable.
**Warning signs:** Test expectation like "HTTP call to non-listed host traps/errors" will fail.

### Pitfall 2: `Client::new()` vs `ClientBuilder` on WASI
**What goes wrong:** `anthropic::Client::new("key")` compiles locally but fails to compile against `wasm32-wasip2` because the impl is behind `#[cfg(feature = "reqwest")]`.
**Why it happens:** rig-wasi's P1 patch gates the reqwest-dependent `Client::new` behind `#[cfg(feature = "reqwest")]`. On WASI, `reqwest` is not enabled.
**How to avoid:** Always use `ClientBuilder::default().api_key(api_key).http_client(WasiHttpClient::default()).build()`. [VERIFIED: packages/rig-wasi/src/client/mod.rs — reqwest gate at line 282]
**Warning signs:** Compile error mentioning `reqwest` or `DefaultHttpClient` type mismatch.

### Pitfall 3: `fuel_limit` Budget for Agent Components
**What goes wrong:** Agent with multiple tool calls and LLM roundtrips runs out of fuel mid-execution. Silent failure or error from engine.
**Why it happens:** Each `wasi:http` call is computationally expensive in Wasmtime fuel units. Simple components (echo, kv-store) use the default `u64::MAX`. Agent components do more work but service.json should also set high fuel limit.
**How to avoid:** Set `"fuel_limit": null` (or omit it) in service.json to use `Workflow::DEFAULT_FUEL_LIMIT = u64::MAX`. [VERIFIED: packages/types/src/service.rs — `DEFAULT_FUEL_LIMIT = u64::MAX`]. STATE.md mentioned calibration needed but u64::MAX is safe for demo.
**Warning signs:** Component returns without result; WAVS logs show fuel exhaustion.

### Pitfall 4: `with_http_client` Type Inference Complexity
**What goes wrong:** Rust cannot infer the H type parameter when calling `.http_client()` if the surrounding context is ambiguous. Compiler errors about "cannot infer type" for `ClientBuilder<..., WasiHttpClient>`.
**Why it happens:** `http_client()` method switches the `H` type parameter: `fn http_client<U>(self, http_client: U) -> ClientBuilder<Ext, ApiKey, U>`. When building the completion model after `.build()`, Rust needs to know `H = WasiHttpClient`.
**How to avoid:** Add explicit type annotation or call `.build()?` immediately after `.http_client(WasiHttpClient::default())` before storing. Let the compiler resolve the chain fully.
**Warning signs:** Type inference errors about `DefaultHttpClient` or `()` not implementing `HttpClientExt`.

### Pitfall 5: Workspace Member Registration for New Example
**What goes wrong:** New `examples/components/agent-example/` crate is not in `Cargo.toml` workspace members list. Causes "not a member of workspace" error.
**Why it happens:** Root `Cargo.toml` explicitly lists all workspace members — new crates must be added.
**How to avoid:** Add `"examples/components/agent-example"` to the workspace `members` array in `Cargo.toml`. [VERIFIED: Cargo.toml — all examples/components/* listed explicitly]
**Warning signs:** `cargo build -p agent-example` reports crate not found.

### Pitfall 6: Negative Test for `AllowedHostPermission::None` Requires Live Node
**What goes wrong:** The check_http_permission test cannot be unit-tested offline — it requires deploying a component that calls `check_http_permission(&HttpPermission::None)` against a real WAVS node with `allowed_http_hosts: "none"` in service.json.
**Why it happens:** `HttpPermission` is a local enum in wavs-rig (an rlib) — the mapping from WIT `AllowedHostPermission` to `HttpPermission` happens in the cdylib component. The rlib can't call WIT host functions.
**How to avoid:** The negative test is a human verification test (per Phase 18 VERIFICATION.md). Design it as a manual step: deploy agent-example with service.json `allowed_http_hosts: "none"`, trigger it, confirm the error string "WAVS agent requires HTTP access — set AllowedHostPermission to All or Only" appears in the response.

---

## Code Examples

### Complete Agent Component skeleton (~30 lines domain logic)
```rust
// Source: Synthesized from packages/wavs-rig/src/agent.rs + examples/components/kv-store/src/lib.rs
// File: examples/components/agent-example/src/lib.rs

use anyhow::Result;
use example_helpers::prelude::*;
use rig::providers::anthropic;
use serde::Serialize;
use wavs_rig::{
    HttpPermission, WasiHttpClient, WavsAgent,
    check_http_permission, run_agent,
    tools::{KvSetTool},
};

// Structured result type
#[derive(Serialize)]
struct AgentResult {
    prompt: String,
    answer: String,
}

// Agent struct carries config
struct ExampleAgent {
    api_key: String,
}

impl WavsAgent for ExampleAgent {
    type Output = AgentResult;
    async fn run(&self, trigger_data: Vec<u8>) -> Result<AgentResult> {
        let prompt = String::from_utf8(trigger_data)?;
        let client = anthropic::ClientBuilder::default()
            .api_key(&self.api_key)
            .http_client(WasiHttpClient::default())
            .build()?;
        let agent = client.agent("claude-3-5-haiku-latest")
            .preamble("Answer the question concisely. Use kv_set to store the answer.")
            .tool(KvSetTool)
            .build();
        let answer = agent.prompt(&prompt).await?;
        Ok(AgentResult { prompt, answer })
    }
}

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<WasmResponse>, String> {
        // Startup: check HTTP permission
        let sw = host::get_service();
        let workflow = sw.service.workflows.into_iter()
            .find(|(id, _)| *id == sw.workflow_id)
            .map(|(_, w)| w)
            .ok_or_else(|| "workflow not found".to_string())?;
        let perm = match workflow.component.permissions.allowed_http_hosts {
            example_helpers::bindings::world::wavs::types::service::AllowedHostPermission::All
                => HttpPermission::All,
            example_helpers::bindings::world::wavs::types::service::AllowedHostPermission::None
                => HttpPermission::None,
            example_helpers::bindings::world::wavs::types::service::AllowedHostPermission::Only(hosts)
                => HttpPermission::Only(hosts),
        };
        check_http_permission(&perm).map_err(|e| e)?;
        // Get API key from env
        let api_key = std::env::var("WAVS_ENV_ANTHROPIC_API_KEY")
            .map_err(|_| "WAVS_ENV_ANTHROPIC_API_KEY not set".to_string())?;
        // Extract prompt from trigger
        let prompt = match trigger_action.data {
            TriggerData::Raw(data) => data,
            _ => return Err("expected Raw trigger data".into()),
        };
        // Run agent (single block_on boundary)
        let output = run_agent(&ExampleAgent { api_key }, prompt)?;
        Ok(vec![WasmResponse { payload: output, ordering: None, event_id_salt: None }])
    }
}

export_layer_trigger_world!(Component);
```

### Cargo.toml for agent-example
```toml
# Source: Synthesized from examples/components/kv-store/Cargo.toml + packages/wavs-rig/Cargo.toml
[package]
name = "agent-example"
edition.workspace = true
version.workspace = true
authors.workspace = true
rust-version.workspace = true
repository.workspace = true

[dependencies]
wavs-rig = { workspace = true }
rig-wasi = { path = "../../packages/rig-wasi" }  # or workspace dep if added
example-helpers = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
anyhow = { workspace = true }

[lib]
crate-type = ["cdylib"]

[package.metadata.component]
package = "wavs-examples:agent-example"
```

### service.json for agent-example
```json
{
  "name": "agent-example",
  "workflows": {
    "agent-workflow-01": {
      "trigger": "manual",
      "component": {
        "source": {
          "digest": "<sha256-of-agent_example.wasm>"
        },
        "permissions": {
          "allowed_http_hosts": { "only": ["api.anthropic.com"] },
          "file_system": false,
          "raw_sockets": false,
          "dns_resolution": false
        },
        "fuel_limit": null,
        "time_limit_seconds": 60,
        "config": {},
        "env_keys": ["WAVS_ENV_ANTHROPIC_API_KEY"]
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

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No LLM in WASI | wavs-rig + rig-wasi fork | Phase 17/18 (2026-04-20) | Anthropic and other LLM providers usable in WASM sandbox |
| AllowedHostPermission::All required | AllowedHostPermission::Only declares intent | Phase 19 | Communicates LLM provider constraint; active filtering is future engine work |
| Client::new(api_key) | ClientBuilder::default().api_key(...).http_client(WasiHttpClient) | Phase 17 P1 patch | reqwest removed from WASI build path |

**Deprecated/outdated:**
- `ProviderClient::from_env()`: Panics on WASI if ANTHROPIC_API_KEY not set AND uses `Client::new()` internally (reqwest path). Use `std::env::var("WAVS_ENV_ANTHROPIC_API_KEY")` + ClientBuilder manually.
- `wstd::runtime::block_on` directly in `Guest::run`: Still works but for agent components, use `run_agent` to ensure single executor boundary.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `rig-wasi` is accessible as a workspace dependency in `agent-example/Cargo.toml` via `{ path = "../../packages/rig-wasi" }` | Standard Stack | If workspace dependency not set up, Cargo.toml needs `rig-wasi = { workspace = true }` added to workspace.dependencies in root Cargo.toml |
| A2 | `AllowedHostPermission::Only(["api.anthropic.com"])` serializes as `{"only": ["api.anthropic.com"]}` in JSON | Code Examples | If serde adds `"content"` wrapper, service.json format would be `{"only": {"content": [...]}}` — needs verification by actually serializing the type |
| A3 | Manual trigger in service.json is just the string `"manual"` (not an object) | Code Examples | Looking at WIT: `variant trigger { ..., manual }` — unit variant should serialize as `"manual"` [PARTIALLY VERIFIED from echo-data which handles TriggerData::Raw but no JSON service.json example found with manual trigger] |
| A4 | The agent example works correctly with `KvSetTool` without a prior `KvGetTool` setup | Architecture Patterns | If wasi:keyvalue bucket must be pre-created, KvSetTool's `store::open` might fail on first invocation |

**Note on A2:** The serde derive for `AllowedHostPermission` is `#[serde(rename_all = "snake_case")]` on an enum with `Only(Vec<String>)`. For externally tagged enums in serde (the default), a tuple variant `Only(T)` serializes as `{"only": [...]}`. This is standard serde behavior — high confidence.

**Note on A3:** The `Trigger::Manual` variant in `wavs_types` with `serde(rename_all = "snake_case")` would serialize as `"manual"`. [ASSUMED — not found a literal service.json with manual trigger, but consistent with serde rules and WIT "manual" unit variant]

---

## Open Questions

1. **Does `rig-wasi` need to be added to `workspace.dependencies` in root `Cargo.toml`?**
   - What we know: `wavs-rig` is already in workspace.dependencies. `rig-wasi` is in workspace members.
   - What's unclear: Whether `agent-example` can reference `rig-wasi` via `{ path = "../../packages/rig-wasi" }` or needs `{ workspace = true }` entry.
   - Recommendation: Add `rig-wasi = { path = "packages/rig-wasi" }` to workspace.dependencies in root Cargo.toml for consistency, then use `{ workspace = true }` in agent-example.

2. **Should the example use `Submit::None` or a real aggregator?**
   - What we know: Simplest is `submit: "none"`. Aggregator pattern adds complexity.
   - What's unclear: Whether wavs-mcp or CLI can handle `submit: "none"` and still return the output.
   - Recommendation: Use `Submit::None` for E2E demo — raw `WasmResponse.payload` visible in logs/API response. The demo's goal is agent reasoning, not on-chain submission.

3. **How to package `agent-example.wasm` for service.json `source.digest`?**
   - What we know: Other examples use `"registry": { "digest": "...", "domain": "localhost:8090" }` or `"digest": "..."` for local builds.
   - What's unclear: Whether Phase 19 will use a local WASM file path, digest, or registry.
   - Recommendation: Use `"source": { "digest": "<sha256>" }` with the computed digest from `just generate-checksums`, same as dev-tool's `ComponentSource::Digest` pattern.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` (wasm32-wasip2 target) | Build agent-example.wasm | [ASSUMED] | — | `just wasi-build-docker agent-example` uses Docker |
| WAVS node (`just start-wavs-dev`) | E2E validation (E2E-02) | [ASSUMED] | — | Start per WAVS/CLAUDE.md instructions |
| Anthropic API key (`WAVS_ENV_ANTHROPIC_API_KEY`) | Agent LLM calls | Requires env setup | — | No fallback — required for E2E-02 |
| `wavs-cli` or `wavs-mcp` | Service deployment | [ASSUMED] | — | Use HTTP API directly |

**Missing dependencies with no fallback:**
- `WAVS_ENV_ANTHROPIC_API_KEY` environment variable — must be set before running E2E validation.

**Missing dependencies with fallback:**
- `cargo` WASI target: if native build fails, `just wasi-build-docker agent-example` uses Docker image.

---

## Validation Architecture

> `nyquist_validation` is `false` in `.planning/config.json` — this section is skipped.

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | partial | `check_http_permission` enforces no-HTTP-for-None at agent startup |
| V5 Input Validation | yes | Agent processes `trigger_data` as UTF-8 string; `String::from_utf8` returns `Err` on invalid bytes |
| V6 Cryptography | no | API key is read from env var, passed in auth header by `WasiHttpClient` — never logged (threat T-18-01 in Phase 18 verified) |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| API key leakage via logging | Information Disclosure | `WasiHttpClient.send()` never logs headers; verified no `eprintln!` of auth headers in wavs-rig |
| Prompt injection via trigger data | Tampering | Inherent to LLM; out of scope for demo — prompt is controlled by trigger sender |
| AllowedHostPermission::Only bypass | Elevation of Privilege | Engine FIXME — active blocking not implemented. Mitigation: document limitation; agent startup check still validates non-None |

---

## Sources

### Primary (HIGH confidence)
- `packages/wavs-rig/src/` — all 6 source files verified in Phase 18 verification
- `packages/rig-wasi/src/client/mod.rs` — `http_client()` method and `ClientBuilder` structure
- `packages/rig-wasi/src/providers/anthropic/client.rs` — `ClientBuilder<H>` API
- `packages/engine/src/worlds/instance.rs` — AllowedHostPermission::Only FIXME confirmed
- `packages/types/src/service.rs` — `AllowedHostPermission` serde behavior
- `examples/components/kv-store/src/lib.rs` — component structure pattern
- `examples/components/echo-data/src/lib.rs` — TriggerData::Raw handling
- `examples/components/permissions/src/lib.rs` — service permissions access pattern
- `wavs-foundry-template/.docker/service.json` — service.json format confirmed
- `Cargo.toml` (workspace root) — workspace members list
- `packages/types/src/lib.rs` — `WAVS_ENV_PREFIX` constant

### Secondary (MEDIUM confidence)
- `.planning/phases/18-wavs-rig-integration-crate/18-VERIFICATION.md` — Phase 18 all 5 requirements satisfied; 3 human tests pending (runtime)
- `.planning/STATE.md` — Phase 19 risk: fuel calibration needed
- `wit-definitions/operator/wit/operator.wit` — `get-service` host function returns `service-and-workflow-id`

### Tertiary (LOW confidence)
- A3 (Manual trigger JSON format as `"manual"`) — inferred from serde behavior, not confirmed via a service.json with manual trigger

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crates verified to exist and compile
- Architecture patterns: HIGH — derived from verified source files, not assumptions
- Pitfalls: HIGH — engine FIXME is confirmed; ClientBuilder reqwest gate is confirmed
- E2E deployment: MEDIUM — service.json format confirmed from template; manual trigger JSON format is ASSUMED

**Research date:** 2026-04-20
**Valid until:** 2026-05-20 (30 days — rig-wasi fork is pinned, WAVS engine stable)
