---
phase: 18-wavs-rig-integration-crate
plan: 03
subsystem: wavs-rig
tags: [rust, wasm, rig, kv, memory, wasi, wavs-rig, wit-bindgen]
dependency_graph:
  requires:
    - packages/wavs-rig (phase 18-01 — crate scaffold with WasiHttpClient)
    - packages/rig-wasi (phase 17 — rig-core fork)
    - wit-definitions/operator/wit (wasi:keyvalue WIT for KV bindings)
  provides:
    - packages/wavs-rig/src/memory.rs (WavsMemory — KV-backed conversation history)
    - packages/wavs-rig/src/agent.rs (WavsAgent trait + run_agent shim)
    - packages/wavs-rig/src/permissions.rs (HttpPermission + check_http_permission)
    - packages/wavs-rig/src/kv_bindings.rs (wit_bindgen-generated wasi:keyvalue bindings)
    - packages/wavs-rig/wit/ (minimal kv-world WIT)
  affects:
    - packages/wavs-rig/src/lib.rs (re-exports all public types)
    - packages/wavs-rig/Cargo.toml (added wit-bindgen dependency)
tech_stack:
  added:
    - wit_bindgen::generate! for wasi:keyvalue in rlib context
    - Minimal kv-world WIT (packages/wavs-rig/wit/) with wasi:keyvalue/imports
  patterns:
    - KV bindings generated in separate kv_bindings.rs module (matches simple-aggregator pattern)
    - wasi::keyvalue::store accessed as crate::kv_bindings::wasi::keyvalue::store
    - Token estimation: (role.len() + content.len()) / 4 — no tokenizer dependency
    - run_agent as sole block_on boundary — prevents deadlock in async agent loop
    - HttpPermission as local enum mirroring AllowedHostPermission (rlib cannot use WIT host types)
key_files:
  created:
    - packages/wavs-rig/src/memory.rs
    - packages/wavs-rig/src/agent.rs
    - packages/wavs-rig/src/permissions.rs
    - packages/wavs-rig/src/kv_bindings.rs
    - packages/wavs-rig/wit/world.wit
    - packages/wavs-rig/wit/deps/wasi-keyvalue-0.2.0-draft2/package.wit
  modified:
    - packages/wavs-rig/src/lib.rs (re-exports + kv_bindings module)
    - packages/wavs-rig/Cargo.toml (wit-bindgen dep)
decisions:
  - "wit_bindgen kv_bindings module: wasip2 crate does not expose wasi:keyvalue; used wit_bindgen::generate! in a separate kv_bindings module (matches simple-aggregator/echo-block-interval pattern)"
  - "Minimal kv-world WIT: avoid pulling in full wavs-world; only import wasi:keyvalue/imports@0.2.0-draft2"
  - "generate_all: required by wit_bindgen 0.53.1 when no explicit with mappings given"
  - "HttpPermission as local enum: rlib cannot import WIT host types from component bindings; mirrors AllowedHostPermission semantics"
  - "run_agent takes &A not A: avoids ownership issues when agent constructed in Guest::run scope"
metrics:
  duration: "~20 minutes"
  completed: "2026-04-20T17:20:09Z"
  tasks_completed: 2
  tasks_total: 2
  files_created: 6
  files_modified: 2
---

# Phase 18 Plan 03: WavsMemory, WavsAgent, and Permission Check Summary

**One-liner:** WavsMemory with KV-backed conversation history (token budget truncation), WavsAgent trait with single-executor run_agent shim, and HttpPermission startup validation — completing the wavs-rig crate.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | WavsMemory with KV-backed conversation history | `432f74fc6` | memory.rs, kv_bindings.rs, wit/world.wit, wit/deps/wasi-keyvalue-0.2.0-draft2/package.wit, Cargo.toml |
| 2 | WavsAgent trait, run_agent shim, permission check | `de16762e4` | agent.rs, permissions.rs, lib.rs |

## What Was Built

### WavsMemory (packages/wavs-rig/src/memory.rs)

KV-backed conversation memory for rig agents:
- Stores full conversation as JSON-serialized `Vec<Message>` under a single KV key
- KV keys namespaced with `wavs_agent_memory:` prefix to avoid collision with application data
- Token estimation: `(role.len() + content.len()) / 4` per message — no tokenizer dependency
- Truncation removes oldest messages (keeps at least 1) when over budget
- `DEFAULT_TOKEN_BUDGET = 4000` (approximately 16K characters of conversation)
- `append()`, `retrieve()`, `clear()` public API

### kv_bindings module (packages/wavs-rig/src/kv_bindings.rs)

WIT-generated wasi:keyvalue bindings for rlib context:
- Uses `wit_bindgen::generate!` with minimal `kv-world` WIT
- `kv-world` includes only `wasi:keyvalue/imports@0.2.0-draft2`
- WIT files at `packages/wavs-rig/wit/` (deps copied from `wit-definitions/operator/wit/deps`)
- Access path: `crate::kv_bindings::wasi::keyvalue::store`

### WavsAgent + run_agent (packages/wavs-rig/src/agent.rs)

Single-executor async bridge for WASI components:
- `WavsAgent` trait with `type Output: Serialize` and `fn run(&self, trigger_data: Vec<u8>) -> impl Future`
- `run_agent<A: WavsAgent>(agent: &A, trigger_data: Vec<u8>) -> Result<Vec<u8>, String>`
- `block_on` called EXACTLY ONCE — documented prominently to prevent deadlock
- Output is JSON-serialized via serde for WAVS result submission

### HttpPermission (packages/wavs-rig/src/permissions.rs)

Startup validation for agent HTTP access:
- `HttpPermission` enum: `All`, `None`, `Only(Vec<String>)` — mirrors `AllowedHostPermission`
- `check_http_permission` returns exact error message: "WAVS agent requires HTTP access — set AllowedHostPermission to All or Only"
- `All` and `Only(_)` both return `Ok(())` — both allow HTTP outbound
- Implemented as a simple function (not a trait) for minimal complexity

### lib.rs re-exports

All public types re-exported for ergonomic usage:
```rust
pub use http::WasiHttpClient;
pub use memory::{WavsMemory, Message};
pub use agent::{WavsAgent, run_agent};
pub use permissions::{HttpPermission, check_http_permission};
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `wasip2` crate does not include `wasi:keyvalue`**
- **Found during:** Task 1 implementation
- **Issue:** Plan specified `use wasip2::keyvalue::store` but the `wasip2` crate (1.0.2+wasi-0.2.9) only provides WASI CLI and HTTP worlds — no keyvalue module
- **Fix:** Added `wit-bindgen = { workspace = true }` to `packages/wavs-rig/Cargo.toml`. Created a minimal `kv-world` WIT (`packages/wavs-rig/wit/world.wit`) that includes only `wasi:keyvalue/imports@0.2.0-draft2`. Generated bindings in a separate `kv_bindings.rs` module using `wit_bindgen::generate!({..., generate_all})`. This matches the pattern used by `examples/components/simple-aggregator` and `examples/components/echo-block-interval`.
- **Files modified:** packages/wavs-rig/Cargo.toml, packages/wavs-rig/src/kv_bindings.rs (new), packages/wavs-rig/wit/ (new)
- **Commit:** 432f74fc6

**2. [Rule 1 - Bug] `generate_all` required by wit_bindgen 0.53.1**
- **Found during:** Task 1, first compile attempt
- **Issue:** `wit_bindgen::generate!` without `generate_all` option rejected with "missing `with` mapping for `wasi:keyvalue/store@0.2.0-draft2`"
- **Fix:** Added `generate_all` to the generate! invocation in `kv_bindings.rs`
- **Files modified:** packages/wavs-rig/src/kv_bindings.rs
- **Commit:** 432f74fc6

## Threat Flags

No new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries were introduced. Conversation history is stored in component-scoped KV only (T-18-09, T-18-10 accepted as per plan). Token budget truncation (T-18-08) is implemented via the `while estimate_tokens > budget && len > 1` loop.

## Self-Check: PASSED

- packages/wavs-rig/src/memory.rs: FOUND
- packages/wavs-rig/src/agent.rs: FOUND
- packages/wavs-rig/src/permissions.rs: FOUND
- packages/wavs-rig/src/kv_bindings.rs: FOUND
- packages/wavs-rig/wit/world.wit: FOUND
- Commit 432f74fc6: FOUND
- Commit de16762e4: FOUND
- cargo check -p wavs-rig --target wasm32-wasip2: PASSED (no errors)
