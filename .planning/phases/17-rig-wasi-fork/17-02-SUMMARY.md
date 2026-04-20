---
phase: 17-rig-wasi-fork
plan: "02"
subsystem: rig-wasi
tags: [rust, wasi, fork, rig, wasm32-wasip2, cfg, reqwest, tokio, sse]
dependency_graph:
  requires: [packages/rig-wasi (from plan 17-01)]
  provides: [wasm32-wasip2 compilable rig-wasi fork, compile probe passing]
  affects: [Cargo.toml (compile-probe member), packages/rig-wasi/src/ (11 files patched)]
tech_stack:
  added: [wstd 0.6.6 (compile probe), wasip2 1.0.2+wasi-0.2.9 (transitive)]
  patterns: [target_family = "wasm" cfg gating, AtomicBool stub for streaming, optional reqwest feature]
key_files:
  created:
    - packages/rig-wasi/tests/compile-probe/Cargo.toml
    - packages/rig-wasi/tests/compile-probe/src/lib.rs
  modified:
    - packages/rig-wasi/Cargo.toml (edition = "2024" override)
    - packages/rig-wasi/src/wasm_compat.rs (P3: cfg unified)
    - packages/rig-wasi/src/streaming.rs (P2: AtomicBool PauseControl stub)
    - packages/rig-wasi/src/http_client/mod.rs (P1+P4: reqwest gating, SSE module gate)
    - packages/rig-wasi/src/http_client/multipart.rs (P1: From<MultipartForm> gated)
    - packages/rig-wasi/src/http_client/sse.rs (P4: #![cfg(not(target_family = "wasm"))])
    - packages/rig-wasi/src/client/mod.rs (P1: DefaultHttpClient type, gated impls)
    - packages/rig-wasi/src/client/model_listing.rs (P1: DefaultHttpClient default)
    - packages/rig-wasi/src/client/builder.rs (P4: gated non-WASM)
    - packages/rig-wasi/src/agent/prompt_request/streaming.rs (P3: StreamingResult cfg)
    - packages/rig-wasi/src/vector_store/mod.rs (P1: StatusCode import gated)
    - packages/rig-wasi/src/lib.rs (P4: providers tree gated non-WASM)
    - packages/rig-wasi/FORK_BASIS.md (finalized with actual line counts)
    - Cargo.toml (compile-probe added to workspace members)
decisions:
  - "Edition override to 2024 in rig-wasi/Cargo.toml — rig-core uses let-chains (stabilized in Rust 2024 edition; workspace uses 2021)"
  - "BoxedStream type alias moved from sse.rs to http_client/mod.rs — needed on all targets; sse module itself gated out on WASM"
  - "providers tree gated entirely behind cfg(not(target_family = 'wasm')) — all providers use sse::GenericEventSource; Phase 18 adds WASI-specific provider impls"
  - "futures-timer v3.0.3 remains in dep tree — compiles on wasip2 (SSE that uses Delay is gated out by P4); no source removal needed"
  - "crate lib name is 'rig' not 'rig_wasi' — compile probe uses rig::wasm_compat::WasmCompatSend"
metrics:
  duration: "~30 minutes"
  completed_date: "2026-04-20"
  tasks_completed: 2
  tasks_total: 2
  files_created: 2
  files_modified: 14
---

# Phase 17 Plan 02: WASI Patches and Compile Probe Summary

**One-liner:** All six WASI compatibility patches applied to rig-wasi fork; wasm32-wasip2 compile probe passes with reqwest absent, tokio rt absent, and wasm-tools validation successful.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Apply Patches P1-P4 — reqwest, tokio, cfg, SSE source fixes | 33ed637e4 | 11 source files in packages/rig-wasi/src/ |
| 2 | Create compile probe and verify wasm32-wasip2 compilation (FORK-05) | b9e219cf5 | packages/rig-wasi/tests/compile-probe/, FORK_BASIS.md, Cargo.toml |

## What Was Built

### Source Patches Applied

**P1 — reqwest optional (FORK-01):**
- `http_client/mod.rs`: Removed `use reqwest::Body`, gated `impl HttpClientExt for reqwest::Client` behind `#[cfg(feature = "reqwest")]`, gated `pub use reqwest::Client as ReqwestClient`, gated `From<NoBody> for reqwest::Body`
- `client/mod.rs`: Introduced `DefaultHttpClient` type alias (`reqwest::Client` when reqwest feature, `()` otherwise); gated `impl Client<Ext, reqwest::Client>` blocks; gated test; gated `client/builder.rs` module
- `http_client/multipart.rs`: Gated `From<MultipartForm> for reqwest::multipart::Form` behind reqwest feature
- `client/model_listing.rs`: Changed `ModelLister<H = reqwest::Client>` to `ModelLister<H = DefaultHttpClient>`
- `vector_store/mod.rs`: Changed `use reqwest::StatusCode` to use `http::StatusCode` on non-reqwest targets

**P2 — tokio rt removal, PauseControl stub (FORK-02):**
- `streaming.rs`: Removed `use tokio::sync::watch;`, replaced `PauseControl` struct (with watch tx/rx) with `AtomicBool` stub implementing same interface (pause/resume/is_paused); added `#[derive(Clone)]`; fixed `StreamingResult` cfg to `target_family = "wasm"`

**P3 — cfg unification (FORK-03):**
- `wasm_compat.rs`: All 8 `#[cfg(all(feature = "wasm", target_arch = "wasm32"))]` occurrences → `#[cfg(target_family = "wasm")]`; `if_wasm!` and `if_not_wasm!` macros updated
- `agent/prompt_request/streaming.rs`: `StreamingResult<R>` type cfg updated to `target_family = "wasm"`

**P4 — SSE dead zone fix (FORK-04):**
- `http_client/sse.rs`: Added `#![cfg(not(target_family = "wasm"))]` inner attribute at file top
- `http_client/mod.rs`: Gated `pub mod sse;` behind non-WASM; moved `BoxedStream` type alias here (accessible on all targets)
- `lib.rs`: Gated `pub mod providers;` behind `#[cfg(not(target_family = "wasm"))]` — all providers use `sse::GenericEventSource`

**P5 — futures-timer (already handled):**
- futures-timer v3.0.3 is in wasip2 dep tree but compiles cleanly (no wasm-bindgen feature; SSE that uses `Delay` is gated out by P4). No source changes needed.

**P6 — getrandom (Cargo.toml, done in Plan 01):**
- Verified no `wasm_js` or `js` feature activated for getrandom.

**P-edition (deviation):**
- `packages/rig-wasi/Cargo.toml`: Changed `edition.workspace = true` to `edition = "2024"` — rig-core uses let-chains which require Rust 2024 edition.

### Compile Probe (FORK-05)

- `packages/rig-wasi/tests/compile-probe/Cargo.toml`: cdylib crate, depends on `rig-wasi = { path = "../.." }` and `wstd`
- `packages/rig-wasi/tests/compile-probe/src/lib.rs`: Imports `rig::wasm_compat::WasmCompatSend`, uses `wstd::runtime::block_on`
- Added to workspace members in root `Cargo.toml`

## Verification Results

All checks passed:

1. `cargo build -p rig-wasi-compile-probe --target wasm32-wasip2` — **exit 0** (Finished)
2. `cargo tree -p rig-wasi --target wasm32-wasip2 | grep reqwest` — **no output** (reqwest absent)
3. `cargo tree -p rig-wasi --target wasm32-wasip2 -f "{p} {f}" | grep tokio` — **tokio v1.52.1 sync** (rt absent)
4. `grep -rn 'feature = "wasm"' packages/rig-wasi/src/wasm_compat.rs` — **no matches** (only comment)
5. `grep 'AtomicBool' packages/rig-wasi/src/streaming.rs` — **6 matches**
6. `grep 'target_family.*wasm' packages/rig-wasi/src/http_client/sse.rs` — **1 match** (gate at top)
7. `grep "TBD" packages/rig-wasi/FORK_BASIS.md` — **no output** (FORK_BASIS.md finalized)
8. `wasm-tools validate target/wasm32-wasip2/debug/rig_wasi_compile_probe.wasm` — **Validated OK**

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing functionality] Edition override needed in rig-wasi/Cargo.toml**
- **Found during:** Task 1 (first cargo check)
- **Issue:** Workspace uses edition = "2021"; rig-core 0.35.0 source uses let-chains which are only allowed in Rust 2024 or later; this caused 21 "let chains only allowed in Rust 2024" compile errors
- **Fix:** Changed `edition.workspace = true` to `edition = "2024"` in packages/rig-wasi/Cargo.toml; this is the only workspace member that overrides edition
- **Files modified:** packages/rig-wasi/Cargo.toml

**2. [Rule 1 - Bug] BoxedStream type alias removed from sse.rs when SSE is gated**
- **Found during:** Task 1
- **Issue:** sse.rs exports `BoxedStream` which is used in `http_client/mod.rs` (`StreamingResponse = Response<BoxedStream>`). Gating sse.rs entirely removed BoxedStream from scope.
- **Fix:** Moved BoxedStream type alias to `http_client/mod.rs` where it remains accessible on all targets
- **Files modified:** packages/rig-wasi/src/http_client/mod.rs

**3. [Rule 1 - Bug] Additional files had reqwest references not in plan**
- **Found during:** Task 1
- **Issue:** `vector_store/mod.rs`, `http_client/multipart.rs`, `client/model_listing.rs`, `client/builder.rs` all had reqwest references not mentioned in the plan; they prevented compilation
- **Fix:** Gated reqwest usages in each file; gated `client/builder.rs` module behind non-WASM cfg
- **Files modified:** 4 additional files

**4. [Rule 1 - Bug] agent/prompt_request/streaming.rs used old cfg form**
- **Found during:** Task 1
- **Issue:** File had `#[cfg(not(all(feature = "wasm", target_arch = "wasm32")))]` for StreamingResult which fired on wasip2 (since feature="wasm" not set), requiring `+ Send` bound that wasn't satisfied
- **Fix:** Updated to `target_family = "wasm"` as part of Patch 3 scope extension
- **Files modified:** packages/rig-wasi/src/agent/prompt_request/streaming.rs

**5. [Rule 2 - Missing] pub mod providers needed gating**
- **Found during:** Task 1
- **Issue:** All providers use `crate::http_client::sse::{Event, GenericEventSource}` which is gated out on WASM; provider files fail to compile
- **Fix:** Added `#[cfg(not(target_family = "wasm"))]` to `pub mod providers;` in lib.rs; this is per-plan intent (providers will be replaced by WASI-specific impls in Phase 18)
- **Files modified:** packages/rig-wasi/src/lib.rs

**6. [Rule 1 - Bug] crate lib name is "rig" not "rig_wasi"**
- **Found during:** Task 2 (compile probe build)
- **Issue:** Compile probe used `rig_wasi::wasm_compat::WasmCompatSend` but the crate's lib name is `rig` (set in [lib] name = "rig")
- **Fix:** Changed to `rig::wasm_compat::WasmCompatSend` in probe lib.rs
- **Files modified:** packages/rig-wasi/tests/compile-probe/src/lib.rs

## Known Stubs

- **PauseControl** (`packages/rig-wasi/src/streaming.rs`): AtomicBool stub — intentional per FORK-02. Streaming completions are unused in the WASI execution model. Phase 18 uses non-streaming `prompt()` path exclusively.

## Self-Check: PASSED

- `packages/rig-wasi/tests/compile-probe/Cargo.toml` exists: FOUND
- `packages/rig-wasi/tests/compile-probe/src/lib.rs` exists: FOUND
- Commit `33ed637e4` exists in git log: FOUND
- Commit `b9e219cf5` exists in git log: FOUND
- `cargo build -p rig-wasi-compile-probe --target wasm32-wasip2` exit 0: VERIFIED
- `wasm-tools validate` passes: VERIFIED
- No TBD in FORK_BASIS.md: VERIFIED
