# Stack Research

**Domain:** rig-core integration into WASI components — WAVS v2.0 Agent Runtime
**Researched:** 2026-04-20
**Confidence:** HIGH — verified via crates.io API (live versions), docs.rs source inspection, and direct codebase inspection of WAVS packages

---

## Executive Summary

Integrating rig-core into wasm32-wasip2 WASI components requires one new crate (`wavs-rig`), one forked crate (`rig-core` → `rig-wasi`), and zero changes to the host-side WAVS node. The existing `wavs-wasi-utils` and `wstd` crates already provide all the host-function primitives (HTTP, KV, EVM) needed by the four bridges. The fork is scoped to ~300-500 lines across five files; the blockers are well-understood and all fixable.

**Current rig-core version:** 0.35.0 (released 2026-04-13, crates.io verified)

---

## Recommended Stack

### New Crates to Create

| Crate | Location | Purpose | Why |
|-------|----------|---------|-----|
| `wavs-rig` | `packages/wavs-rig/` | Integration layer: 4 bridges between rig and WASI sandbox | Keeps rig-specific code isolated; developers import one crate |
| `rig-wasi` fork | git dependency | rig-core with WASI blockers patched | rig-core 0.35.0 has hard blockers on wasm32-wasip2; ~300-500 line fork is the only viable path |

### Core Dependencies for `wavs-rig`

| Library | Version | Purpose | Why |
|---------|---------|---------|-----|
| `rig-core` (forked) | 0.35.0 fork | Agent loop, Tool trait, CompletionModel, 20+ LLM providers | Rig provides the complete agent framework; building from scratch = months of work |
| `wstd` | 0.6.6 (latest) | Async executor (`block_on`), WASI HTTP client, WASI clock | Already in workspace; provides `wstd::runtime::block_on` used by all existing components |
| `wasip2` | 1.0.3+wasi-0.2.9 (latest) | WASIp2 raw bindings including `wasi:keyvalue` | Already in workspace as `wasip2 = "1.0.1"` — upgrade to 1.0.3 |
| `wavs-wasi-utils` | workspace | HTTP helpers, EVM provider helpers | Already provides `fetch_json`, `http_request_post_json` over `wasi:http` |
| `serde` + `serde_json` | workspace | JSON serialization for tool args/outputs, LLM request bodies | Required by rig Tool trait (JSON Schema) |
| `schemars` | ~0.8 or 1.0 | JSON Schema generation for `ToolDefinition` | rig-core uses this for typed tool parameters |
| `anyhow` | workspace | Error propagation across bridge layers | Consistent with rest of WAVS codebase |

### Fork: `rig-core` → `rig-wasi`

**Fork rig-core 0.35.0. Apply five patches. Use as git dependency in `wavs-rig` and the agent example component.**

**Do not attempt to use upstream rig-core 0.35.0 for wasm32-wasip2.** It fails to compile due to three hard blockers:

#### Patch 1: Make `reqwest` optional (Cargo.toml + http_client.rs + client/mod.rs)

reqwest 0.12.x does not support wasm32-wasip2 (GitHub issue #2979, opened March 2026, no merged PR as of April 2026). In rig-core 0.35.0, `reqwest = { features = ["json", "stream", "multipart"] }` is an unconditional dependency — not behind a feature gate.

**Required changes:**
```toml
# rig-wasi Cargo.toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "stream", "multipart"], optional = true }

[features]
default = ["rustls"]  # remove "reqwest" from default
reqwest = ["dep:reqwest"]
```

In `client/mod.rs`: make `H` default type conditional on `reqwest` feature. In `http_client.rs`: gate the reqwest `Client` impl behind `#[cfg(feature = "reqwest")]`.

#### Patch 2: Remove `tokio` `rt` feature, replace `watch` channel in streaming.rs

`tokio = { features = ["rt", "sync"] }` — the `rt` feature requires `std::thread`, unavailable on wasip2. The agent loop itself does not use tokio's runtime (it is pure async with `futures::StreamExt`), but `streaming.rs` imports `tokio::sync::watch` for `PauseControl`.

**Required changes:**
```toml
# rig-wasi Cargo.toml
tokio = { version = "1", features = ["sync"], default-features = false }  # drop "rt"
```

In `streaming.rs`: replace `tokio::sync::watch` with `futures::channel::oneshot` or a simple `std::sync::atomic::AtomicBool` (PauseControl is streaming-only infrastructure; for WASI MVP with sequential execution, stub it as a no-op pause controller).

#### Patch 3: Unify cfg detection in wasm_compat.rs

Current inconsistency: `WasmBoxedFuture` uses `#[cfg(target_family = "wasm")]` (fires on wasip2), but `WasmCompatSend`/`WasmCompatSync` use `#[cfg(all(feature = "wasm", target_arch = "wasm32"))]` (does NOT fire on wasip2 without the `wasm` cargo feature). This creates a type mismatch: futures drop `Send` but traits still require it.

**Required change:** Unify to `#[cfg(target_family = "wasm")]` everywhere in `wasm_compat.rs`. This fires correctly on both wasm32-unknown-unknown and wasm32-wasip2 without needing a separate cargo feature flag for the wasip2 case.

#### Patch 4: Fix SSE dead zones for wasip2 (sse.rs)

The SSE module has branches for `#[cfg(not(target_arch = "wasm32"))]` (native) and `#[cfg(all(feature = "wasm", target_arch = "wasm32"))]` (browser). On wasip2 without `feature = "wasm"`, neither branch compiles. Either add a third branch for `#[cfg(all(not(feature = "wasm"), target_arch = "wasm32"))]` or gate the entire SSE module behind `#[cfg(not(target_family = "wasm"))]` (streaming responses via SSE are not used in the WASI bridge; rig's agent loop uses the non-streaming completion path).

#### Patch 5: Handle `futures-timer` (if transitively required)

`futures-timer` internally uses `std::thread::sleep` on non-WASM platforms. For wasip2, it needs clock-based delay via `wstd::time` or `wasi::monotonic_clock`. If `futures-timer` is in the dependency tree of the fork, either patch it to use `wstd` timers or replace its usage with `wstd::time::Duration` + `wstd::runtime` primitives directly.

#### Patch 6: Verify `getrandom` feature (trivial)

rig-core uses `getrandom = { features = ["js"] }` for randomness (used in nanoid generation). On wasm32-wasip2, getrandom natively supports wasip2 via `wasi:random/random.get-random-u64` — no feature flag required. The `wasm_js` feature (for browser) should be removed in the fork since it bloats Cargo.lock and can cause build issues on non-browser WASM.

```toml
# rig-wasi Cargo.toml — getrandom does NOT need wasm_js for wasip2
getrandom = { version = "0.3", default-features = true }
# wasip2 target gets random natively via wasi:random
```

### Cargo Configuration

**In `wavs-rig/Cargo.toml` (new crate in workspace):**
```toml
[package]
name = "wavs-rig"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
rig-core = { git = "https://github.com/[org]/rig-wasi.git", rev = "[commit]", default-features = false }
wstd = { workspace = true }
wavs-wasi-utils = { path = "../wasi-utils" }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
```

**In workspace `Cargo.toml` — add `wavs-rig` to members:**
```toml
[workspace]
members = [
    # ... existing members ...
    "packages/wavs-rig",
]
```

**In workspace `Cargo.toml` — patch entry for rig-core (overrides transitive deps too):**
```toml
[patch.crates-io]
rig-core = { git = "https://github.com/[org]/rig-wasi.git", rev = "[commit]" }
```

### WASI Component Example: `examples/components/rig-agent`

New example following the existing pattern (like `kv-store`, `cosmos-query`):

```toml
# examples/components/rig-agent/Cargo.toml
[dependencies]
wavs-rig = { path = "../../../packages/wavs-rig" }
example-helpers = { workspace = true }
wstd = { workspace = true }
serde_json = { workspace = true }

[lib]
crate-type = ["cdylib"]
```

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Upstream `rig-core = "0.35.0"` from crates.io | reqwest + tokio rt + cfg inconsistencies = compile failure on wasm32-wasip2 | Fork with 5 targeted patches |
| `reqwest` in `wavs-rig` directly | reqwest 0.12 does not support wasm32-wasip2 (issue #2979 open, no merge) | `wstd::http::Client` via `wavs-wasi-utils::http` |
| `tokio` in `wavs-rig` or agent components | tokio `rt` feature requires `std::thread`; wasip2 is single-threaded, single-process | `wstd::runtime::block_on` is the correct async executor |
| `tokio-wasm` or `tokio_with_wasm` crates | These target wasm32-unknown-unknown (browser); wasip2 is a different target with different primitives | `wstd` 0.6.6 by Bytecode Alliance, designed specifically for wasip2 |
| `wasm-bindgen` or `js-sys` in agent components | These are browser-WASM primitives; wasip2 components run in Wasmtime, not a browser | `wasip2` crate bindings for WASI APIs |
| `getrandom` with `wasm_js` feature | Breaks non-browser WASM builds; wasip2 has native getrandom support | Default getrandom — wasip2 support is built-in |
| New host-side (node) changes for v2.0 MVP | All four bridges operate inside the WASM sandbox using existing host functions | Existing `wasi:http`, `wasi:keyvalue`, `host::log` — zero node changes needed |
| Streaming/SSE responses for agent loop | SSE adds complexity with no benefit in WASI; sequential completion calls are sufficient | Use rig's non-streaming `prompt()` path |
| Concurrent tool execution (`buffer_unordered`) | wasip2 is single-threaded; concurrent futures require a multi-task executor that doesn't exist | Set rig's tool concurrency to 1; sequential tool calls work fine |

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| Fork rig-core (Option B) | Build agent SDK from scratch (Option C) | Rig has 20+ LLM providers, typed Tool trait, WASM-compat traits. Reimplementing = months of work. Fork is ~300-500 lines. |
| Fork rig-core (Option B) | Upstream PR to rig-core (Option A) | Option A is correct long-term but review/merge timeline is unknown. Fork moves fast; Option A pursued in parallel. |
| `wstd::runtime::block_on` | Custom async executor | wstd is maintained by Bytecode Alliance specifically for wasip2. It already works in existing WAVS components. |
| `wstd::http::Client` for HTTP bridge | `wasi-http-client` crate | wavs-wasi-utils already wraps wstd HTTP; `fetch_json` / `http_request_post_json` are the established patterns in this codebase. |
| Simple `AtomicBool` for PauseControl stub | Full `futures::channel` watch replacement | PauseControl is streaming infrastructure; WASI MVP uses non-streaming completions. Stub is sufficient and avoids pulling in channels with thread requirements. |
| `[patch.crates-io]` for rig-core | Separate fork workspace | `[patch.crates-io]` in workspace root automatically patches all transitive dependencies. Clean, no duplication. |

---

## Async Runtime: Why `wstd::runtime::block_on`

All existing WAVS WASI components use this pattern:

```rust
impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        wstd::runtime::block_on(async {
            // async code here
        })
    }
}
```

wasip2 is single-threaded. `block_on` drives the future to completion cooperatively with the WASI reactor (which handles I/O events like HTTP responses and KV operations). Rig's agent loop — `agent.prompt(&prompt).await` — is a standard async chain with no thread-spawning. It works inside `block_on` as long as:

1. HTTP calls use `wstd::http::Client` (not reqwest)
2. No `tokio::spawn` is called (rig's agent loop doesn't spawn — it is sequential)
3. Tool calls are sequential (set concurrency = 1 in rig config)

---

## Version Compatibility

| Package | Version | Status | Notes |
|---------|---------|--------|-------|
| `rig-core` | 0.35.0 | Fork required | Latest as of 2026-04-13; fork at this version |
| `wstd` | 0.6.6 | Upgrade from 0.6.5 | Workspace currently has 0.6.5; 0.6.6 (2026-03-12) is latest |
| `wasip2` | 1.0.3+wasi-0.2.9 | Upgrade from 1.0.1 | Workspace currently has 1.0.1; 1.0.3 (2026-04-17) is latest |
| `reqwest` | 0.12.x | Do NOT use in agent components | No wasm32-wasip2 support; issue open, no PR merged |
| `tokio` (in fork) | 1.x sync-only | Drop `rt` feature in fork | `sync` feature (for `Mutex`, `RwLock`) may still be needed; `rt` must be removed |
| `getrandom` | 0.3.x | No `wasm_js` flag | wasip2 has native random support; `wasm_js` is browser-only |
| `schemars` | workspace-compatible | Verify in fork | rig uses schemars for ToolDefinition JSON Schema generation; wasip2-compatible |

---

## Integration Points with Existing WAVS Structure

| `wavs-rig` Feature | Bridges To | Existing Code |
|--------------------|------------|---------------|
| HTTP transport (LLM API calls) | `wstd::http::Client` | `packages/wasi-utils/src/http.rs` — `fetch_json`, `http_request_post_json` |
| KV memory (conversation history) | `wasi:keyvalue::store` | `examples/components/kv-store/src/lib.rs` — `store::open`, `bucket.get/set` |
| EVM query tool | `wavs-wasi-utils::evm` | `packages/wasi-utils/src/evm/` — `get_evm_chain_config`, provider helpers |
| Logging | `host::log` | `example_helpers::bindings::world::host::log` |
| Entry point | `wstd::runtime::block_on` | All existing components use this pattern |
| Component type | `cdylib` + WIT bindings | Same as all examples; `export_layer_trigger_world!(Component)` |

---

## Sources

- `crates.io/api/v1/crates/rig-core` — verified latest version 0.35.0 (released 2026-04-13) (HIGH confidence)
- `docs.rs/crate/rig-core/latest/source/Cargo.toml.orig` — reqwest features, tokio features, optional deps (HIGH confidence)
- `docs.rs/rig-core/latest/src/rig/streaming.rs.html` — `use tokio::sync::watch` at line 31 confirmed (HIGH confidence)
- `docs.rs/rig-core/latest/src/rig/wasm_compat.rs.html` — cfg inconsistency between `WasmCompatSend` and `WasmBoxedFuture` confirmed (HIGH confidence)
- `github.com/seanmonstar/reqwest/issues/2979` — wasip2 support open issue, no merged PR as of April 2026 (HIGH confidence)
- `crates.io/api/v1/crates/wstd` — version 0.6.6 (2026-03-12), maintained by Bytecode Alliance (HIGH confidence)
- `crates.io/api/v1/crates/wasip2` — version 1.0.3+wasi-0.2.9 (2026-04-17) (HIGH confidence)
- Direct inspection of `/workspace/WAVS/packages/wasi-utils/src/http.rs` — `wstd::http::Client` used for all HTTP in WASI components (HIGH confidence)
- Direct inspection of `/workspace/WAVS/packages/wasi-utils/Cargo.toml` — no reqwest, uses wstd (HIGH confidence)
- Direct inspection of `/workspace/WAVS/Cargo.toml` — current workspace versions for wstd (0.6.5), wasip2 (1.0.1), getrandom config (HIGH confidence)
- `/workspace/WAVS_AGENT_IMPROVEMENTS.md` — April 2026 investigation: confirmed hard blockers, fork strategy, file-level change breakdown (HIGH confidence)

---
*Stack research for: WAVS v2.0 — rig-core WASI integration*
*Researched: 2026-04-20*
