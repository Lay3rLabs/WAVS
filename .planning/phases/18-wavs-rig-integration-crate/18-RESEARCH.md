# Phase 18: wavs-rig Integration Crate - Research

**Researched:** 2026-04-20
**Domain:** Rust WASI component library bridging rig-wasi into the WAVS sandbox
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- WasiHttpClient wraps wstd::http::Client to implement rig's HttpClientExt trait
- Request/response mapping: convert rig's http types <-> wstd::http types
- Auth headers passed through from agent config, not hardcoded
- Each tool is a separate struct implementing rig's Tool trait
- KvGetTool/KvSetTool use wasi:keyvalue host bindings (already available in WAVS engine)
- HttpFetchTool uses WasiHttpClient for external HTTP calls
- EvmQueryTool uses existing wavs-wasi-utils EVM helpers
- LogTool writes to wasi:logging (via host::log)
- All tools have typed args/output with serde + JSON Schema via schemars
- WavsMemory stores messages as JSON in wasi:keyvalue under a conversation key prefix
- Append: push new message to list; Retrieve: load all messages; Truncation: drop oldest when estimated token count exceeds budget
- Token estimation: simple char-count / 4 heuristic (no tokenizer dep in WASM)
- WavsAgent trait with async fn run(trigger_data) -> Result<AgentOutput>
- run_agent shim wraps the trait call inside wstd::runtime::block_on
- Single block_on call — no nested async runtimes (prevents deadlock)
- Before agent execution, check if HTTP outgoing is available via wasi:http capability probe
- If AllowedHostPermission::None -> return clear error string, not silent WASI trap
- Error message: "WAVS agent requires HTTP access — set AllowedHostPermission to All or Only"

### Claude's Discretion

- Internal module organization within packages/wavs-rig
- Error types and error handling patterns
- Any additional utility functions needed for the bridge
- Token budget default value
- Whether to re-export rig types or require consumers to depend on rig-wasi directly

### Deferred Ideas (OUT OF SCOPE)

- Agent continuation mode (CONT-01) — v3.0
- Service-to-service calls (RPC-01) — v3.0
- Structured tool abstraction in WIT (TOOL-01) — v3.0
- Embedding index / fact store (MEM-01, MEM-02) — v3.0
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| RIG-01 | WasiHttpClient implements rig's HttpClientExt trait over wasi:http/outgoing-handler | HttpClientExt trait signature verified in packages/rig-wasi/src/http_client/mod.rs; wstd::http::Client usage verified in packages/wasi-utils/src/http.rs |
| RIG-02 | Built-in WAVS tools: KvGetTool, KvSetTool, HttpFetchTool, EvmQueryTool, LogTool — typed args/output, JSON Schema | rig's Tool trait signature verified in packages/rig-wasi/src/tool/mod.rs; schemars 1.0.4 confirmed in rig-wasi/Cargo.toml |
| RIG-03 | WavsMemory with KV-backed conversation history, append, retrieve, token budget truncation | KV API usage pattern verified in examples/components/kv-store/src/lib.rs |
| RIG-04 | WavsAgent trait + run_agent shim via wstd::runtime::block_on | block_on usage verified in 6 example components; single-invocation pattern confirmed |
| RIG-05 | Startup validation: AllowedHostPermission::None returns clear error | AllowedHostPermission enum verified in packages/types/src/service.rs; HTTP probe pattern understood |
</phase_requirements>

---

## Summary

Phase 18 creates `packages/wavs-rig`, a new `rlib` crate in the WAVS workspace that bridges the `rig-wasi` fork (Phase 17) into WASI component development. The crate has five distinct sub-problems: (1) an HTTP transport implementing `HttpClientExt` over `wstd::http::Client`, (2) five built-in tool implementations using existing WASI host capabilities, (3) a KV-backed conversation memory store with token budget enforcement, (4) a `WavsAgent` trait + `run_agent` async entry-point shim, and (5) startup permission validation.

All five of these problems have verified prior art in the WAVS codebase. `packages/wasi-utils/src/http.rs` already wraps `wstd::http::Client` in exactly the pattern needed for `WasiHttpClient`. The KV store example component demonstrates the full `wasi:keyvalue` read/write API. EVM query helpers exist in `packages/wasi-utils/src/evm/provider.rs`. `wstd::runtime::block_on` is used correctly by six example components already in production. The `AllowedHostPermission` enum lives in `packages/types/src/service.rs`.

The critical architectural constraint is that the entire rig agent loop — including all LLM API calls and tool executions — must run inside a **single** `wstd::runtime::block_on` call. WAVS components are single-threaded WASI guests; calling `block_on` inside an already-running `block_on` will deadlock. The `run_agent` shim must be the outermost and only executor boundary.

**Primary recommendation:** Create `packages/wavs-rig` as an `rlib` crate (not `cdylib`), added to the workspace. Structure around five modules: `http`, `tools`, `memory`, `agent`, `permissions`. The `WasiHttpClient` wraps `wstd::http::Client` and implements `HttpClientExt` using the same request-builder / `Body::from` pattern already established in `wasi-utils`.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rig-wasi` (local) | workspace | LLM agent framework (forked) | Phase 17 output; HttpClientExt + Tool traits defined here |
| `wstd` | 0.6.5 [VERIFIED: Cargo.toml] | WASI async runtime + HTTP client | Used by all WAVS components; provides block_on and Client |
| `wasip2` | 1.0.1 [VERIFIED: Cargo.toml] | WASI 0.2 host interface bindings | Standard WAVS binding crate; provides wasi:keyvalue, wasi:logging |
| `wavs-wasi-utils` (local) | workspace | HTTP/EVM helpers | EvmQueryTool reuses WasiEvmClient directly |
| `serde` | 1.0.228 [VERIFIED: Cargo.toml] | Serialization | All tool args/outputs must be (De)Serialize |
| `serde_json` | 1.0.145 [VERIFIED: Cargo.toml] | JSON for KV storage and tool schemas | |
| `schemars` | 1.0.4 [VERIFIED: rig-wasi/Cargo.toml] | JSON Schema for tool definitions | Already in rig-wasi; Tool::definition returns schemars-generated schemas |
| `anyhow` | workspace [VERIFIED: Cargo.toml] | Error propagation | WAVS convention |
| `thiserror` | 2.0.12 [VERIFIED: rig-wasi/Cargo.toml] | Structured error types | Used throughout rig-wasi |
| `bytes` | 1.10.1 [VERIFIED: rig-wasi/Cargo.toml] | Byte buffer type required by HttpClientExt | HttpClientExt::send bounds require T: Into<Bytes> |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `futures` | 0.3.32 [VERIFIED: rig-wasi/Cargo.toml] | WasmBoxedFuture, async combinators | Required by WasmCompatSend boxed future patterns |
| `alloy-primitives` | workspace | EVM address/uint types for EvmQueryTool | When EvmQueryTool encodes call data |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| char-count/4 token heuristic | tiktoken or tokenizer crate | Tokenizer crates are not wasm32-wasip2 compatible; locked decision |
| wstd::http::Client for WasiHttpClient | raw wasi:http WIT calls | wstd already wraps WIT calls; avoid reimplementing |

**Installation (new crate addition to workspace):**
```bash
# packages/wavs-rig/Cargo.toml — no npm; pure Rust workspace crate
# Add "packages/wavs-rig" to [workspace] members in root Cargo.toml
# Add wavs-rig = { path = "packages/wavs-rig" } to [workspace.dependencies]
```

---

## Architecture Patterns

### Recommended Project Structure
```
packages/wavs-rig/
├── Cargo.toml
└── src/
    ├── lib.rs           # Public API, re-exports, crate-level docs
    ├── http.rs          # WasiHttpClient — HttpClientExt impl over wstd::http::Client
    ├── tools/
    │   ├── mod.rs       # Tool registry re-exports
    │   ├── kv.rs        # KvGetTool, KvSetTool
    │   ├── http.rs      # HttpFetchTool
    │   ├── evm.rs       # EvmQueryTool
    │   └── log.rs       # LogTool
    ├── memory.rs        # WavsMemory — KV-backed conversation history
    ├── agent.rs         # WavsAgent trait + run_agent shim
    └── permissions.rs   # AllowedHostPermission startup probe
```

### Pattern 1: WasiHttpClient implementing HttpClientExt

**What:** A `#[derive(Clone, Default)] struct WasiHttpClient` that wraps `wstd::http::Client` and satisfies rig's `HttpClientExt` trait. This lets rig's provider clients (Anthropic, OpenAI, etc.) dispatch all LLM API calls through the WASI host's outgoing HTTP handler.

**When to use:** Constructed once at agent startup, passed to the rig provider client builder as the `H` type parameter.

**Critical insight from codebase:** `HttpClientExt` is a generic trait parameter `H` in `Client<Ext, H>`. On WASM targets, `providers` module is entirely gated out (`#[cfg(not(target_family = "wasm"))]` in `lib.rs`). So agent components must construct the provider's completion model directly using the lower-level `Client<Ext, H>` struct with `WasiHttpClient` as `H`. [VERIFIED: packages/rig-wasi/src/lib.rs line 136]

```rust
// Source: packages/wasi-utils/src/http.rs + packages/rig-wasi/src/http_client/mod.rs
use bytes::Bytes;
use http::{Request, Response};
use rig::http_client::{HttpClientExt, LazyBody, MultipartForm, Result, StreamingResponse};
use rig::wasm_compat::WasmCompatSend;
use wstd::http::{Body, Client as WstdClient, Request as WstdRequest};

#[derive(Clone, Default)]
pub struct WasiHttpClient;

impl HttpClientExt for WasiHttpClient {
    fn send<T, U>(
        &self,
        req: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        T: Into<Bytes> + WasmCompatSend,
        U: From<Bytes> + WasmCompatSend + 'static,
    {
        async move {
            let (parts, body_t) = req.into_parts();
            let body_bytes: Bytes = body_t.into();
            let wstd_req = WstdRequest::builder()
                .method(parts.method.as_str())
                .uri(parts.uri.to_string().as_str())
                // headers need per-entry insertion
                .body(Body::from(body_bytes.to_vec()))
                .map_err(|e| rig::http_client::Error::Protocol(e))?;
            // ... send via WstdClient::new().send(wstd_req).await
            // ... convert WstdResponse -> rig Response<LazyBody<U>>
            todo!()
        }
    }
    // send_multipart: multipart not directly supported by wstd;
    //   can serialize as regular bytes or return Err::Protocol
    // send_streaming: SSE gated on non-WASM; return Err for streaming (not needed for LLMs)
}
```

**Key gap to verify at implementation time:** `wstd::http::Request` uses a builder with `&str` methods; header map iteration from `http::HeaderMap` needs explicit loop to copy headers into wstd request.

### Pattern 2: Tool trait implementation with schemars 1.0

**What:** Each tool is a `struct` implementing rig's `Tool` trait. The `definition()` async fn returns a `ToolDefinition` with `parameters` as a `serde_json::Value`. schemars 1.0 generates this via `schemars::schema_for!(Args)`.

**schemars 1.0 note:** schemars 1.0 renamed `JsonSchema` derive macro to the same name but the `schema_for!` macro now returns `schemars::Schema` not `schemars::schema::RootSchema`. [VERIFIED: schemars 1.0.4 in rig-wasi/Cargo.toml; `schemars::Schema` used in agent/completion.rs line 44]

```rust
// Source: packages/rig-wasi/src/tool/mod.rs (Tool trait definition)
use rig::{completion::ToolDefinition, tool::Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct KvGetArgs {
    pub bucket: String,
    pub key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KvGetArgs { ... } // derive JsonSchema for schema generation

pub struct KvGetTool;

impl Tool for KvGetTool {
    const NAME: &'static str = "kv_get";
    type Error = KvToolError;
    type Args = KvGetArgs;
    type Output = Option<Vec<u8>>;  // or String for JSON-serializable output

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Read a value from WAVS KV store".to_string(),
            parameters: serde_json::to_value(
                schemars::schema_for!(KvGetArgs)
            ).unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        use wasip2::keyvalue::store;
        let bucket = store::open(&args.bucket)...;
        Ok(bucket.get(&args.key)...)
    }
}
```

### Pattern 3: WavsMemory — KV-backed conversation history

**What:** Stores the full conversation as a JSON-serialized `Vec<Message>` under a single KV key. On append, deserialize existing list, push new `Message`, reserialize. On retrieve, deserialize and return all. After append, check token estimate and truncate from the front if over budget.

**KV binding source:** [VERIFIED: examples/components/kv-store/src/lib.rs] — `wasip2` crate re-exports `wasi:keyvalue` under `wasip2::keyvalue::store`. The `wit-bindgen`-generated `store::open(id)` returns `Bucket`; `bucket.get(key)` returns `Option<Vec<u8>>`, `bucket.set(key, value)` writes bytes.

```rust
// Source: examples/components/kv-store/src/lib.rs (verified KV API)
use wasip2::keyvalue::store;

pub struct WavsMemory {
    bucket: String,
    conversation_key: String,
    token_budget: usize,  // default e.g. 4000
}

impl WavsMemory {
    fn load(&self) -> anyhow::Result<Vec<Message>> {
        let b = store::open(&self.bucket)...;
        match b.get(&self.conversation_key)... {
            Some(bytes) => Ok(serde_json::from_slice(&bytes)?),
            None => Ok(vec![]),
        }
    }

    fn save(&self, messages: &[Message]) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(messages)?;
        let b = store::open(&self.bucket)...;
        b.set(&self.conversation_key, &bytes)...;
        Ok(())
    }

    fn estimate_tokens(messages: &[Message]) -> usize {
        messages.iter().map(|m| m_text_len(m) / 4).sum()
    }

    pub fn append(&self, message: Message) -> anyhow::Result<()> {
        let mut messages = self.load()?;
        messages.push(message);
        // Truncate oldest if over budget
        while Self::estimate_tokens(&messages) > self.token_budget && messages.len() > 1 {
            messages.remove(0);
        }
        self.save(&messages)
    }
}
```

**Important:** `wasi:keyvalue` errors use `wasip2::keyvalue::store::Error` which must be mapped to `anyhow::Error`. KV is synchronous (no async) in the WASI host binding. [VERIFIED: kv-store example uses it synchronously without block_on]

### Pattern 4: WavsAgent trait + run_agent shim

**What:** `WavsAgent` is a user-implemented trait. `run_agent` is a function that wraps the trait method in a single `wstd::runtime::block_on`. Components call this from their synchronous `Guest::run` method.

**Critical: Single executor boundary.** `block_on` from `wstd` is a cooperative single-threaded executor. Calling it from inside an existing `block_on` deadlocks because the inner call tries to poll futures on the same thread that's already parked. Rig's agent loop is async internally — it must all run inside the single outer `block_on`. [VERIFIED: block_on pattern in 6 example components; see examples/components/permissions/src/lib.rs:28]

```rust
// Source: examples/components/permissions/src/lib.rs (block_on pattern)
use wstd::runtime::block_on;

pub trait WavsAgent {
    type Output: serde::Serialize;
    async fn run(&self, trigger_data: Vec<u8>) -> anyhow::Result<Self::Output>;
}

pub fn run_agent<A: WavsAgent>(
    agent: A,
    trigger_data: Vec<u8>,
) -> Result<Vec<u8>, String> {
    block_on(async move {
        let output = agent.run(trigger_data).await.map_err(|e| e.to_string())?;
        serde_json::to_vec(&output).map_err(|e| e.to_string())
    })
}
```

**Rig tool concurrency setting:** Rig's agent builder has a `max_concurrent_tool_calls` parameter (or equivalent). Must be set to 1 (or sequential mode) since WASI is single-threaded. [ASSUMED] — Verify by reading rig agent builder source; the locked decision says "Sequential tool execution for WASI MVP".

### Pattern 5: AllowedHostPermission startup probe (RIG-05)

**What:** Before running the agent, attempt a probe HTTP request (or check the service config directly) to detect if HTTP outgoing is available. If it fails with a "not allowed" error, return the human-readable startup error immediately.

**Probe approach:** The cleanest approach is a "dry-run" probe: attempt to open an HTTP connection to a known endpoint and catch the WASI host error. `wstd::http::Client::new().send(...)` on an endpoint will fail fast with a WASI trap or error if `AllowedHostPermission::None` is in effect.

**Alternative:** Access `host::get_service()` to inspect the permissions configuration. [VERIFIED: permissions example uses `host::get_service()` at line 86 to inspect `service.workflows`] The service struct from `packages/types/src/service.rs` contains `Permissions.allowed_http_hosts: AllowedHostPermission`. Reading it before attempting HTTP is cleaner than a probe.

```rust
// Source: packages/types/src/service.rs (AllowedHostPermission enum)
// Source: examples/components/permissions/src/lib.rs (host::get_service() usage)
pub fn check_http_permission() -> Result<(), String> {
    use AllowedHostPermission::*;
    let svc = host::get_service();  // from wit-bindgen generated bindings
    // find the workflow config to read permissions
    match svc.service.permissions.allowed_http_hosts {
        All | Only(_) => Ok(()),
        None => Err(
            "WAVS agent requires HTTP access — set AllowedHostPermission to All or Only"
                .to_string()
        ),
    }
}
```

**Note on accessing `host::get_service()`:** This requires the `wavs-world` WIT bindings, which are generated by `wit-bindgen` from `wit-definitions/operator/wit`. The `wavs-rig` crate itself is an `rlib` (not `cdylib`) — it cannot directly call WIT host functions. The permission check must either accept the service config as a parameter or be called from the component that wraps `wavs-rig`. [VERIFIED: `_helpers` crate shows that `wit-bindgen::generate!` must be called at the cdylib component level, not in a lib crate]. **Recommendation:** Accept `AllowedHostPermission` as a parameter to the check function, so the component can pass it from `host::get_service()`.

### Anti-Patterns to Avoid

- **Nested block_on:** Never call `wstd::runtime::block_on` inside async code running under `block_on`. Use a single outer `block_on` in `run_agent` and make everything inside async.
- **reqwest feature in wavs-rig:** `wavs-rig` must never enable the `reqwest` feature on `rig-wasi`. Only `WasiHttpClient` is the HTTP backend.
- **tokio::spawn inside tools:** No thread-spawning or task-spawning in WASI. Tool `call()` implementations must be synchronous or purely `async` without spawning.
- **Direct `providers::` usage on WASM:** The entire `rig::providers` module is `#[cfg(not(target_family = "wasm"))]`. Components must use `rig::client::Client<Ext, WasiHttpClient>` pattern directly.
- **wasi:keyvalue bucket name collision:** Use a namespaced prefix for WavsMemory keys (e.g., `wavs_agent_memory:{conversation_id}`) to avoid collision with application KV data.
- **KV as async operation:** `wasi:keyvalue` host bindings are synchronous. Do not wrap them in `block_on` (already in async context if called from within a `block_on` future — just call them directly as sync).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON Schema for tool args | Manual schema objects | `schemars::JsonSchema` derive + `schema_for!()` | schemars 1.0 already in rig-wasi dep tree; handles nested types correctly |
| HTTP request construction | Custom wstd request builder | `packages/wasi-utils/src/http.rs` helpers (or same pattern) | Already handles method/header/body for wstd |
| EVM JSON-RPC calls | Custom RPC client | `wavs-wasi-utils` `WasiEvmClient` (packages/wasi-utils/src/evm/provider.rs) | Already wasm32-wasip2 compatible; battle-tested |
| Async executor | Any executor other than wstd | `wstd::runtime::block_on` | Only executor compatible with WAVS WASI sandbox |
| Conversation serialization | Binary/custom format | `serde_json::to_vec` / `from_slice` | JSON is readable in KV inspection and debuggable |
| Token counting | Full tokenizer | char-count / 4 heuristic | Locked decision; no tokenizer crate compiles to wasm32-wasip2 |

**Key insight:** The WAVS package ecosystem already provides solutions for every non-trivial problem this phase faces. The crate's job is wiring, not invention.

---

## Common Pitfalls

### Pitfall 1: providers module gated on WASM

**What goes wrong:** Code that uses `rig::providers::anthropic::Client::new(api_key)` compiles on native but fails on `wasm32-wasip2` with "no module named `providers`".

**Why it happens:** `pub mod providers` is `#[cfg(not(target_family = "wasm"))]` in `lib.rs`. [VERIFIED: packages/rig-wasi/src/lib.rs line 136]

**How to avoid:** Use `rig::client::Client::<AnthropicExt, WasiHttpClient>::builder().api_key(key).build()` or expose a WASM-specific constructor in `wavs-rig` that wraps the lower-level client API.

**Warning signs:** `error[E0433]: failed to resolve: use of undeclared module or unresolved import` mentioning `providers`.

### Pitfall 2: Nested block_on deadlock

**What goes wrong:** The agent loop produces no output, component appears to hang, WAVS eventually times it out.

**Why it happens:** `wstd::runtime::block_on` is cooperative and single-threaded. A nested call parks the thread waiting for itself.

**How to avoid:** `run_agent` must be the sole `block_on` call. All async code inside (including rig's completion loop, tool calls, memory ops) must be `.await`-ed, not wrapped in another `block_on`.

**Warning signs:** Component invocations that never return a result but don't error.

### Pitfall 3: send_streaming not available for WASI

**What goes wrong:** Calling `HttpClientExt::send_streaming` on `WasiHttpClient` with an SSE endpoint; rig's streaming path won't function.

**Why it happens:** SSE consumer code in rig-wasi is `#[cfg(not(target_family = "wasm"))]`. Even if implemented, WASI has no persistent connection model for streaming.

**How to avoid:** `WasiHttpClient::send_streaming` can return a single-chunk stream or `Err` since no LLM API call needed for basic non-streaming completion. Document clearly that streaming is not supported (per REQUIREMENTS.md out-of-scope table).

**Warning signs:** LLM responses hanging or truncated.

### Pitfall 4: wasi:keyvalue error mapping

**What goes wrong:** `store::open(bucket)` returns a `Result<_, wasip2::keyvalue::store::Error>` which is a WIT-generated type that doesn't implement `std::error::Error`. Direct `?` propagation fails.

**Why it happens:** WIT-generated types are structs/enums, not standard error types.

**How to avoid:** Map errors explicitly: `.map_err(|e| anyhow::anyhow!("KV error: {:?}", e))` or wrap in a `thiserror` enum variant.

**Warning signs:** `error[E0277]: the trait bound ... is not satisfied` for `?` operator on KV results.

### Pitfall 5: wstd Request builder API differences from http crate

**What goes wrong:** `http::Request` (from the `http` crate) and `wstd::http::Request` (from `wstd`) have different builder APIs. Attempting to map one to the other naively loses headers.

**Why it happens:** `http::HeaderMap` is a multi-value map. `wstd::http::Request::builder()` methods add single headers. The `.method()` method on `wstd` takes `&str`, not `http::Method`.

**How to avoid:** When converting `http::Request<T>` to `wstd::http::Request<Body>` in `WasiHttpClient::send`, iterate over `HeaderMap` entries and add each individually. Convert `Method` via `.as_str()`.

**Warning signs:** LLM API returning 400/401 because headers (Authorization, Content-Type) are lost.

### Pitfall 6: AllowedHostPermission check requires WIT bindings

**What goes wrong:** `wavs-rig` (an rlib) tries to call `host::get_service()` directly, fails to compile because the `Guest`/`host` bindings are only available in `cdylib` components via `wit-bindgen::generate!`.

**Why it happens:** WIT host function imports are only available in components that generate bindings. A pure `rlib` does not import WIT interfaces.

**How to avoid:** The `check_http_permission` function in `wavs-rig` should accept `AllowedHostPermission` as a parameter. The component's `run` function calls `host::get_service()`, extracts the permission, and passes it to `wavs-rig::permissions::check`. [VERIFIED: permissions example line 86-100 shows service access pattern]

---

## Code Examples

Verified patterns from existing WAVS code:

### wstd::runtime::block_on entry point (WAVS component pattern)
```rust
// Source: examples/components/permissions/src/lib.rs:28-49
impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        block_on(async move {
            let (trigger_id, req) = decode_trigger_event(trigger_action.data)
                .map_err(|e| e.to_string())?;
            let resp = inner_run_task(req).await.map_err(|e| e.to_string())?;
            let resp = serde_json::to_vec(&resp).map_err(|e| e.to_string())?;
            Ok(vec![encode_trigger_output(trigger_id, resp, host::get_service().service.manager)])
        })
    }
}
```

### wasi:keyvalue read/write (KvGetTool/KvSetTool pattern)
```rust
// Source: examples/components/kv-store/src/lib.rs:93-115
fn open_bucket(id: &str) -> Result<store::Bucket, anyhow::Error> {
    store::open(id).map_err(|e| anyhow::anyhow!("KV bucket open error: {:?}", e))
}

fn read_value(bucket_id: &str, key: &str) -> Result<Option<Vec<u8>>, anyhow::Error> {
    let bucket = open_bucket(bucket_id)?;
    bucket.get(key).map_err(|e| anyhow::anyhow!("KV read error: {:?}", e))
}

fn write_value(bucket_id: &str, key: &str, value: &[u8]) -> Result<(), anyhow::Error> {
    let bucket = open_bucket(bucket_id)?;
    bucket.set(key, value).map_err(|e| anyhow::anyhow!("KV write error: {:?}", e))
}
```

### rig Tool trait minimum implementation
```rust
// Source: packages/rig-wasi/src/tool/mod.rs:57-141
impl Tool for LogTool {
    const NAME: &'static str = "log";
    type Error = LogToolError;
    type Args = LogArgs;
    type Output = ();  // serde_json::Value::Null

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Log a message to WAVS host logging".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(LogArgs))
                .unwrap_or_default(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        host::log(LogLevel::Info, &args.message);
        Ok(())
    }
}
```

### EVM query helper (EvmQueryTool pattern)
```rust
// Source: packages/wasi-utils/src/evm/provider.rs (WasiEvmClient)
// EvmQueryTool args must include rpc_url and ABI-encoded call data
use wavs_wasi_utils::evm::new_evm_provider;
use alloy_provider::Provider;

// Inside EvmQueryTool::call():
let provider = new_evm_provider::<alloy_network::Ethereum>(args.rpc_url);
let result = provider.call(&call_request).await?;
```

### HttpClientExt trait to implement (rig-wasi)
```rust
// Source: packages/rig-wasi/src/http_client/mod.rs:111-139
pub trait HttpClientExt: WasmCompatSend + WasmCompatSync {
    fn send<T, U>(
        &self,
        req: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where T: Into<Bytes> + WasmCompatSend, U: From<Bytes> + WasmCompatSend + 'static;

    fn send_multipart<U>(
        &self,
        req: Request<MultipartForm>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where U: From<Bytes> + WasmCompatSend + 'static;

    fn send_streaming<T>(
        &self,
        req: Request<T>,
    ) -> impl Future<Output = Result<StreamingResponse>> + WasmCompatSend
    where T: Into<Bytes>;
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| schemars 0.8.x `RootSchema` | schemars 1.0.x `Schema` | 2024 | `schema_for!` returns `schemars::Schema`; `serde_json::to_value` on it works the same |
| reqwest as HTTP client | wstd::http::Client for WASI | Phase 17 | `reqwest` feature is now opt-in on rig-wasi; WasiHttpClient is the only WASI-compatible impl |

**Deprecated/outdated:**
- `rig::providers::*`: Gated out on WASM targets. Do not use directly in components.
- `tokio::runtime::block_on`: Not available in WASI. Use `wstd::runtime::block_on` only.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | rig's AgentBuilder has a max_concurrent_tool_calls or sequential execution configuration | Architecture Pattern 4 (run_agent) | If not, tool parallelism may attempt thread-spawning and deadlock; need to verify in rig-wasi/src/agent/builder.rs |
| A2 | `serde_json::to_value(schemars::schema_for!(T))` produces a valid JSON Schema Value for ToolDefinition parameters | Tools pattern | If schemars 1.0 Schema serialization differs, tool definitions may be malformed; test at compile-probe time |
| A3 | `wasip2` crate at version 1.0.1 re-exports `wasi:keyvalue::store` at the module path `wasip2::keyvalue::store` | Pitfall 4, KV code examples | If path differs, all KV code needs path adjustment; verify by checking wasip2 crate structure |
| A4 | `wstd::http::Request::builder()` supports per-header insertion analogous to `http::HeaderMap` iteration | WasiHttpClient implementation | If wstd builder API is significantly different, request conversion is more complex |

**If this table is empty:** All claims in this research were verified or cited — no user confirmation needed.

---

## Open Questions

1. **rig AgentBuilder concurrent tool execution configuration**
   - What we know: The locked decision says "Sequential tool execution for WASI MVP (single-threaded sandbox; configure rig concurrency to 1)"
   - What's unclear: The exact AgentBuilder API for controlling this — is it `max_concurrent_tools(1)`, a feature flag, or handled by rig automatically when there's no Tokio runtime?
   - Recommendation: Read `packages/rig-wasi/src/agent/builder.rs` during Wave 0 implementation to find the correct API before writing `run_agent`.

2. **WasiHttpClient multipart support**
   - What we know: `HttpClientExt::send_multipart` must be implemented; `wstd::http::Client` has no built-in multipart support
   - What's unclear: Whether any rig LLM provider (Anthropic, OpenAI) uses multipart for basic text completion (unlikely) or only for file uploads (audio, image)
   - Recommendation: Return `Err(http_client::Error::Protocol(...))` for `send_multipart` with a "not supported in WASI" message; no LLM text completion uses multipart.

3. **Re-export strategy for rig types**
   - What we know: This is Claude's Discretion. Consumers will need `rig::completion::Message`, `rig::tool::Tool`, `rig::agent::AgentBuilder` etc.
   - Recommendation: Re-export key rig types from `wavs_rig::prelude` (e.g., `pub use rig::{tool::Tool, completion::Message, agent::AgentBuilder}`). Consumers should also add `rig-wasi` as a direct dependency for types not re-exported. This avoids version mismatch.

---

## Environment Availability

Step 2.6: This phase is code/config-only (creating a new Rust library crate). The only external dependency is the Rust toolchain with `wasm32-wasip2` target.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All | ✓ | 1.91.0 (workspace rust-version) [VERIFIED: Cargo.toml] | — |
| wasm32-wasip2 target | Compile verification | [ASSUMED: available if Phase 17 passed] | — | Install via `rustup target add wasm32-wasip2` |
| packages/rig-wasi | WasiHttpClient, Tool trait | ✓ | workspace [VERIFIED: packages/rig-wasi/ exists] | — |
| packages/wasi-utils | EvmQueryTool | ✓ | workspace path dep [VERIFIED: Cargo.toml line 186] | — |

**Missing dependencies with no fallback:** None.

---

## Validation Architecture

`nyquist_validation: false` — section omitted per config.

---

## Security Domain

This phase is internal infrastructure (a library crate with no network-exposed endpoints). The primary security concern is that `AllowedHostPermission` enforcement is correctly detected (RIG-05) — the crate does NOT bypass or weaken WAVS's sandbox model, it only validates that the required permission is present.

No ASVS categories directly apply to a library crate that delegates all access control to the WAVS engine sandbox.

---

## Sources

### Primary (HIGH confidence)
- `packages/rig-wasi/src/http_client/mod.rs` — `HttpClientExt` trait signature, all three methods, bounds
- `packages/rig-wasi/src/tool/mod.rs` — `Tool` trait definition, `ToolDefinition` structure, `ToolDyn` wrapping
- `packages/rig-wasi/src/lib.rs` — providers module gating (`#[cfg(not(target_family = "wasm"))]` line 136)
- `packages/rig-wasi/src/wasm_compat.rs` — `WasmCompatSend`, `WasmCompatSync`, `WasmBoxedFuture` definitions
- `packages/rig-wasi/Cargo.toml` — schemars 1.0.4, bytes 1.10.1, thiserror 2.0.12
- `packages/wasi-utils/src/http.rs` — wstd::http::Client usage pattern
- `packages/wasi-utils/src/evm/provider.rs` — WasiEvmClient / new_evm_provider for EvmQueryTool
- `examples/components/kv-store/src/lib.rs` — store::open, bucket.get, bucket.set pattern
- `examples/components/permissions/src/lib.rs` — block_on entry point, host::get_service()
- `packages/types/src/service.rs` — AllowedHostPermission enum (lines 650-655)
- `Cargo.toml` (workspace) — wstd 0.6.5, wasip2 1.0.1, serde 1.0.228

### Secondary (MEDIUM confidence)
- schemars 1.0 API change (`Schema` vs `RootSchema`) — inferred from `agent/completion.rs` line 44 using `schemars::Schema`

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependency versions verified from Cargo.toml files
- Architecture: HIGH — HttpClientExt, Tool, block_on all verified from codebase
- Pitfalls: HIGH — WASM cfg gate, block_on deadlock, KV API verified directly

**Research date:** 2026-04-20
**Valid until:** 2026-05-20 (stable workspace; rig-wasi is a local fork pinned to a git rev)
