---
phase: 18-wavs-rig-integration-crate
plan: 02
subsystem: wavs-rig
tags: [rust, wasm, rig, tools, wasi, keyvalue, http, evm, logging]
dependency_graph:
  requires:
    - packages/wavs-rig (phase 18-01 — crate scaffold + WasiHttpClient)
    - packages/rig-wasi (phase 17 — rig-core fork with Tool trait)
    - wit-definitions/operator/wit/deps/wasi-keyvalue-0.2.0-draft2 (KV bindings)
    - packages/wasi-utils (HTTP helpers pattern reference)
  provides:
    - packages/wavs-rig/src/tools/kv.rs (KvGetTool, KvSetTool)
    - packages/wavs-rig/src/tools/http.rs (HttpFetchTool)
    - packages/wavs-rig/src/tools/evm.rs (EvmQueryTool)
    - packages/wavs-rig/src/tools/log.rs (LogTool)
    - packages/wavs-rig/src/tools/mod.rs (re-exports all five tools)
  affects:
    - packages/wavs-rig/Cargo.toml (added wit-bindgen dependency)
    - Cargo.lock
tech_stack:
  added:
    - wit_bindgen::generate! for wasi:keyvalue bindings inline in kv.rs
    - schemars::schema_for! for JSON Schema generation on all tool args
    - wstd::http::Client for HttpFetchTool and EvmQueryTool
  patterns:
    - Raw JSON-RPC over wstd HTTP for EvmQueryTool (avoids alloy WASM issues)
    - wit_bindgen generate! with "imports" world + keyvalue WIT path for rlib KV access
    - KV errors format!("{:?}") because wasip2 KV error types lack std::error::Error
    - LogTool writes to stderr (eprintln!) which WASI captures for logging
key_files:
  created:
    - packages/wavs-rig/src/tools/kv.rs
    - packages/wavs-rig/src/tools/http.rs
    - packages/wavs-rig/src/tools/evm.rs
    - packages/wavs-rig/src/tools/log.rs
  modified:
    - packages/wavs-rig/src/tools/mod.rs (replaced placeholder with full re-exports)
    - packages/wavs-rig/Cargo.toml (added wit-bindgen workspace dep)
decisions:
  - "wit_bindgen::generate! in kv.rs rlib: wasip2 1.0.2 does NOT provide wasi:keyvalue; generate! with the operator WIT path provides the correct bindings"
  - "Raw JSON-RPC for EvmQueryTool: avoids alloy-provider WASM compilation complexity; consistent with WasiEvmClient pattern in wasi-utils"
  - "eprintln! for LogTool: rlib cannot call host::log() directly (component-world specific); stderr is captured by WASI runtime"
  - "format!({:?}) for KV errors: wasi:keyvalue error variant types do not implement std::error::Error; Debug formatting is the safe fallback"
metrics:
  duration: "~20 minutes"
  completed: "2026-04-20T17:10:56Z"
  tasks_completed: 2
  tasks_total: 2
  files_created: 4
  files_modified: 2
---

# Phase 18 Plan 02: Built-in WAVS Tools Summary

**One-liner:** Five rig Tool trait impls (KvGetTool, KvSetTool, HttpFetchTool, EvmQueryTool, LogTool) with typed args/output and JSON Schema, all compiling to wasm32-wasip2.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Implement KvGetTool, KvSetTool, and LogTool | `0a4688679` | tools/mod.rs, tools/kv.rs, tools/log.rs, Cargo.toml |
| 2 | Implement HttpFetchTool and EvmQueryTool | `ca6e00ec1` | tools/http.rs, tools/evm.rs |

## What Was Built

### Five Tool Implementations

All five tools implement rig's `Tool` trait with:
- `const NAME: &'static str` — unique tool identifier for LLM function calling
- `type Args` — serde `Deserialize` + `schemars::JsonSchema` derive
- `type Output` — serde `Serialize`
- `type Error` — `thiserror::Error` derive
- `async fn definition()` — returns `ToolDefinition` with JSON Schema parameters
- `async fn call()` — executes the tool action using WASI host capabilities

### KvGetTool (`kv_get`)

Reads a UTF-8 value from the WAVS KV store:
- Args: `{ bucket: String, key: String }`
- Output: `Option<String>` (None if key missing)
- Uses `wit_bindgen::generate!` with the operator WIT's keyvalue package to bind `wasi:keyvalue/store`
- Calls `store::open(bucket).get(key)`, converts `Vec<u8>` to UTF-8 string

### KvSetTool (`kv_set`)

Writes a UTF-8 value to the WAVS KV store:
- Args: `{ bucket: String, key: String, value: String }`
- Output: `"ok"` confirmation string
- Same wasi:keyvalue bindings; calls `bucket.set(key, value.as_bytes())`

### HttpFetchTool (`http_fetch`)

Makes HTTP requests via wasi:http/outgoing-handler:
- Args: `{ url, method?, body?, headers? }`
- Output: `{ status: u16, body: String }`
- Supports GET/POST/PUT/DELETE/PATCH/HEAD
- Uses `wstd::http::Client` directly (same transport as WasiHttpClient)
- AllowedHostPermission enforced by WAVS host — tool cannot bypass it

### EvmQueryTool (`evm_query`)

Executes read-only eth_call via raw JSON-RPC:
- Args: `{ rpc_url: String, to: String, data: String }`
- Output: hex-encoded return data string (e.g., `"0x000...001"`)
- Builds JSON-RPC payload: `{"method":"eth_call","params":[{"to":...,"data":...},"latest"]}`
- Uses wstd::http::Client for the POST request (avoids alloy WASM complications)
- Parses JSON-RPC error and result fields; returns descriptive errors

### LogTool (`log`)

Logs structured messages via stderr:
- Args: `{ level: String, message: String }`
- Output: the logged message (echo)
- Maps level string → TRACE/DEBUG/INFO/WARN/ERROR labels
- Uses `eprintln!` which the WASI runtime captures for logging

## Implementation Notes

### Key technical decisions

**1. wit_bindgen::generate! in the rlib**

The research doc stated wasip2 provides `wasi:keyvalue` but wasip2 1.0.2 does NOT include keyvalue or logging. The actual source is the WIT definitions at `wit-definitions/operator/wit/deps/wasi-keyvalue-0.2.0-draft2/package.wit`. By adding `wit-bindgen` as a dependency and using `generate!` with `world: "imports"` and the WIT path, the rlib gets the correct bindings that will satisfy the component link. The path `../../wit-definitions/operator/wit/deps/wasi-keyvalue-0.2.0-draft2/package.wit` is relative to the `packages/wavs-rig/` directory.

**2. Raw JSON-RPC for EvmQueryTool**

Rather than using alloy-provider (which requires complex WASM-compatibility shims and optional features), EvmQueryTool sends raw JSON-RPC eth_call directly via wstd::http::Client. This matches the pattern used by `WasiEvmClient` in `packages/wasi-utils/src/evm/provider.rs` and is simpler for a tool that only needs read-only calls.

**3. LogTool via eprintln!**

An rlib cannot call `host::log()` (which is part of the component world export bindings, available only in cdylib crates). The correct approach is to write to stderr via `eprintln!`. The WAVS runtime captures stderr output from WASI components and routes it through its logging subsystem. This is semantically equivalent and doesn't require any special bindings.

**4. KV error formatting**

The `wasi:keyvalue::store::Error` type (generated by wit_bindgen) is a WIT `variant` that does NOT implement `std::error::Error`. The workaround is `format!("{:?}", e)` (Debug formatting) to convert to a string for the `KvToolError::KvError(String)` variant.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Functionality] Added wit-bindgen dep for KV bindings**
- **Found during:** Task 1
- **Issue:** wasip2 1.0.2 does not provide wasi:keyvalue or wasi:logging modules; plan said to use `wasip2::keyvalue::store` but those paths don't exist
- **Fix:** Added `wit-bindgen = { workspace = true }` to Cargo.toml; used `wit_bindgen::generate!` with the operator WIT path to bind the keyvalue interface
- **Files modified:** packages/wavs-rig/Cargo.toml, packages/wavs-rig/src/tools/kv.rs
- **Commit:** 0a4688679

**2. [Rule 1 - Bug] LogTool uses eprintln! instead of wasi:logging**
- **Found during:** Task 1
- **Issue:** Plan suggested using `wasip2::logging::logging::log()` or `host::log()`, but neither is available in an rlib. wasip2 lacks a logging module; host::log() is cdylib-only.
- **Fix:** Used `eprintln!` with level prefix — WASI routes stderr to the host's logging sink
- **Files modified:** packages/wavs-rig/src/tools/log.rs
- **Commit:** 0a4688679

## Threat Flags

None. All tools operate within the WAVS WASI sandbox. HttpFetchTool and EvmQueryTool make outbound HTTP calls that are controlled by AllowedHostPermission at the host level — no new attack surface beyond what the threat model already accounts for.

## Self-Check: PASSED

Files exist:
- packages/wavs-rig/src/tools/kv.rs — FOUND
- packages/wavs-rig/src/tools/http.rs — FOUND
- packages/wavs-rig/src/tools/evm.rs — FOUND
- packages/wavs-rig/src/tools/log.rs — FOUND
- packages/wavs-rig/src/tools/mod.rs — FOUND

Commits exist:
- 0a4688679 — FOUND (KvGetTool, KvSetTool, LogTool)
- ca6e00ec1 — FOUND (HttpFetchTool, EvmQueryTool)

cargo check -p wavs-rig --target wasm32-wasip2: PASSED (no errors, only pre-existing warnings in rig-wasi)
