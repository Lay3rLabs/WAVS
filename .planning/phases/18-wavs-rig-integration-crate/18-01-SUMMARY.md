---
phase: 18-wavs-rig-integration-crate
plan: 01
subsystem: wavs-rig
tags: [rust, wasm, rig, http, wasi, wavs-rig]
dependency_graph:
  requires:
    - packages/rig-wasi (phase 17 — rig-core fork with HttpClientExt trait)
    - packages/wasi-utils (wstd http helpers pattern)
  provides:
    - packages/wavs-rig (new crate — HTTP transport for rig agents in WASM sandbox)
  affects:
    - Cargo.toml (workspace membership + dependency)
tech_stack:
  added:
    - wavs-rig crate (new rlib for WASI sandbox)
    - wstd::http::Client (WASI outgoing HTTP transport)
  patterns:
    - Pre-convert body T→Bytes before async block to avoid 'static bound issues
    - anyhow::Error → StringError wrapper for rig's HttpError::Instance
    - http::Request (rig) directly usable with wstd (same underlying http crate types)
key_files:
  created:
    - packages/wavs-rig/Cargo.toml
    - packages/wavs-rig/src/lib.rs
    - packages/wavs-rig/src/http.rs
    - packages/wavs-rig/src/tools/mod.rs
    - packages/wavs-rig/src/memory.rs
    - packages/wavs-rig/src/agent.rs
    - packages/wavs-rig/src/permissions.rs
  modified:
    - Cargo.toml (added packages/wavs-rig to members + workspace.dependencies)
decisions:
  - "Pre-convert body T→Bytes before async block: avoids impl stricter bounds than trait (E0276)"
  - "StringError wrapper for anyhow::Error: wstd uses anyhow::Error which lacks std::error::Error impl"
  - "Reconstruct http::Request with builder(): preserves method/URI/all headers for wstd compatibility"
metrics:
  duration: "~15 minutes"
  completed: "2026-04-20T17:00:30Z"
  tasks_completed: 2
  tasks_total: 2
  files_created: 7
  files_modified: 1
---

# Phase 18 Plan 01: wavs-rig Crate Scaffold + WasiHttpClient Summary

**One-liner:** `packages/wavs-rig` rlib crate scaffolded with WasiHttpClient routing LLM HTTP calls through wasi:http/outgoing-handler via wstd::http::Client.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create wavs-rig crate scaffold and Cargo.toml | `adf7eeb14` | Cargo.toml, packages/wavs-rig/Cargo.toml, src/lib.rs, placeholder modules |
| 2 | Implement WasiHttpClient with HttpClientExt trait | `de8906fab` | packages/wavs-rig/src/http.rs |

## What Was Built

### packages/wavs-rig crate

New rlib crate that bridges `rig-wasi` (the rig-core WASM fork from Phase 17) to the WAVS WASI sandbox. This is the foundational crate for the v2.0 Agent Runtime milestone — every LLM API call from a rig agent flows through `WasiHttpClient`.

**Crate structure:**
- `src/lib.rs` — module declarations + key rig re-exports (Agent, ToolDefinition, Tool)
- `src/http.rs` — WasiHttpClient implementing HttpClientExt (this plan)
- `src/tools/mod.rs` — placeholder (Plan 02)
- `src/memory.rs` — placeholder (Plan 03)
- `src/agent.rs` — placeholder (Plan 03)
- `src/permissions.rs` — placeholder (Plan 03)

### WasiHttpClient

Implements all three `HttpClientExt` methods:
- **`send()`**: Routes requests through `wstd::http::Client` → `wasi:http/outgoing-handler`. Preserves method, URI, and all headers (critical for Authorization + Content-Type headers to LLM APIs).
- **`send_multipart()`**: Returns `NOT_IMPLEMENTED` — LLM completion APIs use JSON, not multipart.
- **`send_streaming()`**: Returns `StreamEnded` — streaming out of scope per REQUIREMENTS.md.

## Implementation Notes

### Key technical decisions

**1. Body pre-conversion pattern**

The `HttpClientExt::send` trait signature doesn't require `T: 'static`, but returning a `'static` future that captures `T` would require `T: 'static`. The solution: convert `T → Bytes` BEFORE the `async move` block, so only `'static` data enters the future.

```rust
// BEFORE async block (no T captured):
let (parts, body_t) = req.into_parts();
let body_bytes: Bytes = body_t.into();  // T consumed here
let wstd_req_result = builder.body(WstdBody::from(body_bytes.to_vec()))...;

async move {
    let wstd_req = wstd_req_result?;  // only 'static data in future
    ...
}
```

**2. anyhow::Error conversion**

wstd uses `anyhow::Error` as its HTTP error type, which does NOT implement `std::error::Error`. On WASM targets, `rig::http_client::Error::Instance` requires `Box<dyn std::error::Error + 'static>`. Resolution: `StringError` wrapper that implements `std::error::Error`:

```rust
fn wstd_error_to_http(e: anyhow::Error) -> HttpError {
    struct StringError(String);
    impl std::error::Error for StringError {}
    HttpError::Instance(Box::new(StringError(format!("{e:#}"))))
}
```

**3. wstd/http type compatibility**

wstd re-exports `http::request::Request` directly (same underlying crate). This means `http::Request` from rig and wstd's `Request` are the same type. We can reconstruct a `Request<WstdBody>` using `Request::builder()` and pass it directly to `WstdClient::send()`.

## Verification Results

```
cargo check -p wavs-rig --target wasm32-wasip2
Finished `dev` profile [unoptimized + debuginfo]
```

- `impl HttpClientExt for WasiHttpClient` — present in http.rs
- `WstdClient::new()` — used for transport
- `parts.headers.iter()` — all headers copied (Authorization, Content-Type preserved)
- `reqwest` count in Cargo.toml — 0 (no reqwest dependency)

## Deviations from Plan

**[Rule 1 - Bug] Pre-convert body T→Bytes before async block**
- **Found during:** Task 2 implementation
- **Issue:** `async move` capturing `T` requires `T: 'static`, but the `HttpClientExt` trait doesn't have that bound — adding `T: 'static` to the impl would violate E0276.
- **Fix:** Extract `T → Bytes` conversion before the `async move` block so `T` is consumed before the future is constructed.
- **Files modified:** packages/wavs-rig/src/http.rs
- **Commit:** `de8906fab`

**[Rule 1 - Bug] StringError wrapper for anyhow::Error**
- **Found during:** Task 2 implementation
- **Issue:** `wstd::http::Error = anyhow::Error` does not implement `std::error::Error`, causing E0277 when wrapping in `Box<dyn std::error::Error>`.
- **Fix:** `StringError` struct wrapping error message string, implementing `std::error::Error`.
- **Files modified:** packages/wavs-rig/src/http.rs
- **Commit:** `de8906fab`

## Known Stubs

The following modules are empty placeholders for future plans:

| File | Purpose | Implementing Plan |
|------|---------|-------------------|
| packages/wavs-rig/src/tools/mod.rs | Built-in WAVS tools (KV, EVM, HTTP fetch, logging) | Plan 02 |
| packages/wavs-rig/src/memory.rs | WavsMemory — KV-backed conversation memory | Plan 03 |
| packages/wavs-rig/src/agent.rs | WavsAgent — agent entry-point shim | Plan 03 |
| packages/wavs-rig/src/permissions.rs | Permission check for AllowedHostPermission | Plan 03 |

These stubs do not prevent the plan's goal (WasiHttpClient HTTP transport) from being achieved. They are intentional scaffolding for subsequent plans.

## Threat Flags

None. No new network endpoints, auth paths, or schema changes beyond those described in the plan's threat model. T-18-01 (API key disclosure) is mitigated — headers are never logged in http.rs.

## Self-Check: PASSED

- [x] `packages/wavs-rig/Cargo.toml` exists
- [x] `packages/wavs-rig/src/lib.rs` exists
- [x] `packages/wavs-rig/src/http.rs` exists with `impl HttpClientExt for WasiHttpClient`
- [x] `packages/wavs-rig/src/tools/mod.rs` exists
- [x] `packages/wavs-rig/src/memory.rs` exists
- [x] `packages/wavs-rig/src/agent.rs` exists
- [x] `packages/wavs-rig/src/permissions.rs` exists
- [x] Cargo.toml includes `packages/wavs-rig` in workspace members
- [x] Cargo.toml includes `wavs-rig = { path = "packages/wavs-rig" }` in workspace.dependencies
- [x] Commits `adf7eeb14` and `de8906fab` exist
- [x] `cargo check -p wavs-rig --target wasm32-wasip2` passes with no errors
