---
phase: 18-wavs-rig-integration-crate
verified: 2026-04-20T18:00:00Z
status: human_needed
score: 5/5 must-haves verified
re_verification: false
human_verification:
  - test: "Deploy a minimal WASI component that uses WasiHttpClient and makes an actual LLM API call (e.g., to api.anthropic.com)"
    expected: "The HTTP request routes through wasi:http/outgoing-handler and a valid JSON response is returned; Authorization header is forwarded correctly"
    why_human: "Cannot test outbound HTTP from wasm32-wasip2 without a running WAVS node; compile-time check confirms wiring but cannot verify runtime behavior"
  - test: "Deploy a component with AllowedHostPermission::None, call check_http_permission, and observe the returned error"
    expected: "Exact error string 'WAVS agent requires HTTP access — set AllowedHostPermission to All or Only' is returned before any LLM request is attempted"
    why_human: "The permission enum is a local mirror type (not the host WIT type); wiring from host::get_service() to HttpPermission requires a live WAVS node to validate end-to-end"
  - test: "Deploy a component with WavsMemory, run two invocations with messages that exceed the token budget, then retrieve history"
    expected: "Second retrieval shows oldest messages truncated, newest retained; conversation does not grow beyond token budget across separate component invocations"
    why_human: "KV persistence across invocations requires a live WAVS node with wasi:keyvalue host; cannot simulate without runtime"
---

# Phase 18: wavs-rig Integration Crate Verification Report

**Phase Goal:** `packages/wavs-rig` is a library crate that bridges rig into the WASI component sandbox — providing an HTTP transport over wasi:http, five typed built-in tool implementations, KV-backed conversation memory, and the `run_agent` async shim
**Verified:** 2026-04-20T18:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `WasiHttpClient` routes LLM API calls through `wasi:http/outgoing-handler` implementing `HttpClientExt` with no reqwest | VERIFIED | `impl HttpClientExt for WasiHttpClient` in `src/http.rs`; uses `WstdClient::new().send()` which maps to wasi:http; `reqwest` count in Cargo.toml = 0; `parts.headers.iter()` copies all headers including Authorization |
| 2 | All five built-in tools compile to wasm32-wasip2, have typed args/output, and produce JSON Schema definitions | VERIFIED | `cargo check -p wavs-rig --target wasm32-wasip2` passes (no errors); all five files exist with `impl Tool for ...`; all args structs derive `#[derive(JsonSchema)]`; `schemars::schema_for!` called in every `definition()` |
| 3 | `WavsMemory` appends to KV, retrieves history, and truncates when over token budget | VERIFIED | `pub fn append`, `pub fn retrieve`, `pub fn clear` present; `estimate_tokens` uses `(role.len() + content.len()) / 4`; truncation loop `while estimate_tokens > budget && len > 1 { messages.remove(0) }`; `DEFAULT_TOKEN_BUDGET = 4000` |
| 4 | `WavsAgent` + `run_agent` bridges async agent loop to WASI component via single `block_on` | VERIFIED | `pub trait WavsAgent` declared; `pub fn run_agent<A: WavsAgent>` calls `block_on` exactly once (line 56); `block_on` appears once functionally (remaining 6 hits are comments/imports); output JSON-serialized |
| 5 | `AllowedHostPermission::None` returns clear error instead of silent trap | VERIFIED | `check_http_permission` returns `Err("WAVS agent requires HTTP access — set AllowedHostPermission to All or Only")` for `HttpPermission::None`; exact string matches ROADMAP requirement |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `packages/wavs-rig/Cargo.toml` | Crate manifest with rig-wasi, wstd, serde, schemars, bytes, anyhow deps | VERIFIED | All deps present; crate-type = ["rlib"]; edition = "2024"; no reqwest |
| `packages/wavs-rig/src/lib.rs` | Crate root with module declarations and public re-exports | VERIFIED | `pub mod` for all 6 modules; re-exports WasiHttpClient, WavsMemory, Message, WavsAgent, run_agent, HttpPermission, check_http_permission |
| `packages/wavs-rig/src/http.rs` | WasiHttpClient implementing HttpClientExt | VERIFIED | Full implementation; all 3 trait methods; wstd transport; header copy; StringError wrapper for anyhow::Error |
| `packages/wavs-rig/src/tools/mod.rs` | Tool module re-exports | VERIFIED | Re-exports all 5 tool types; `pub mod kv`, `http`, `evm`, `log` |
| `packages/wavs-rig/src/tools/kv.rs` | KvGetTool and KvSetTool implementations | VERIFIED | Both `impl Tool for KvGetTool` and `impl Tool for KvSetTool`; uses `crate::kv_bindings::wasi::keyvalue::store` |
| `packages/wavs-rig/src/tools/http.rs` | HttpFetchTool implementation | VERIFIED | `impl Tool for HttpFetchTool`; uses `wstd::http::Client` directly (not WasiHttpClient); typed args/output with JsonSchema |
| `packages/wavs-rig/src/tools/evm.rs` | EvmQueryTool implementation | VERIFIED | `impl Tool for EvmQueryTool`; raw JSON-RPC eth_call over wstd HTTP; JsonSchema on args |
| `packages/wavs-rig/src/tools/log.rs` | LogTool implementation | VERIFIED | `impl Tool for LogTool`; uses `eprintln!` (correct for rlib — wasi:logging not accessible); JsonSchema on args |
| `packages/wavs-rig/src/memory.rs` | WavsMemory with append, retrieve, and token budget truncation | VERIFIED | `pub struct WavsMemory`; `pub fn append`/`retrieve`/`clear`; token budget; `wavs_agent_memory:` key prefix |
| `packages/wavs-rig/src/agent.rs` | WavsAgent trait and run_agent entry-point shim | VERIFIED | `pub trait WavsAgent`; `pub fn run_agent`; single `block_on` call; JSON output serialization |
| `packages/wavs-rig/src/permissions.rs` | AllowedHostPermission startup check | VERIFIED | `HttpPermission` enum; `check_http_permission` function; exact error message |
| `packages/wavs-rig/src/kv_bindings.rs` | wit_bindgen generated wasi:keyvalue bindings | VERIFIED | `wit_bindgen::generate!` with `kv-world` WIT path; `generate_all` present |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `packages/wavs-rig/src/http.rs` | `rig::http_client::HttpClientExt` | `impl HttpClientExt for WasiHttpClient` | WIRED | Pattern found; all 3 trait methods implemented |
| `Cargo.toml` | `packages/wavs-rig/Cargo.toml` | workspace members list | WIRED | `"packages/wavs-rig"` at line 16 in workspace members; `wavs-rig = { path = "packages/wavs-rig" }` at line 296 in workspace.dependencies |
| `packages/wavs-rig/src/tools/kv.rs` | `wasi:keyvalue/store` | wit_bindgen via `kv_bindings` module | WIRED | `use crate::kv_bindings::wasi::keyvalue::store`; `store::open` called in both tools |
| `packages/wavs-rig/src/memory.rs` | `wasi:keyvalue/store` | `kv_bindings` module | WIRED | `use crate::kv_bindings::wasi::keyvalue::store`; `store::open` called in `load()` and `save()` |
| `packages/wavs-rig/src/agent.rs` | `wstd::runtime::block_on` | single executor boundary | WIRED | `use wstd::runtime::block_on`; `block_on(async { ... })` at line 56 — exactly one functional call |
| `packages/wavs-rig/src/permissions.rs` | `HttpPermission` enum | permission enum check | WIRED | `HttpPermission::None` match arm returns exact error string |
| `packages/wavs-rig/src/kv_bindings.rs` | `packages/wavs-rig/wit/world.wit` | `wit_bindgen::generate!` path | WIRED | `path: "wit"` resolves to `packages/wavs-rig/wit/world.wit` which imports `wasi:keyvalue/imports@0.2.0-draft2` |

### Data-Flow Trace (Level 4)

Not applicable — `packages/wavs-rig` is an rlib library crate, not a rendered UI component. No JSX/TSX data rendering paths exist. All data flows are through synchronous function calls and KV operations verified via Level 3 wiring above.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| wavs-rig compiles to wasm32-wasip2 | `cargo check -p wavs-rig --target wasm32-wasip2` | `Finished dev profile` — 0 errors, 16 warnings (pre-existing in rig-wasi upstream) | PASS |
| All 5 tool impl patterns present | grep for all 5 `impl Tool for` patterns | All 5 found in respective files | PASS |
| No reqwest in crate | `grep -c reqwest packages/wavs-rig/Cargo.toml` | 0 | PASS |
| KV namespacing present | `grep "wavs_agent_memory:"` | Found in memory.rs as `const KEY_PREFIX` | PASS |
| Single block_on boundary | `grep -c "block_on" agent.rs` (functional) | 1 functional call at line 56 | PASS |
| Exact permission error string | grep for exact message | Found: "WAVS agent requires HTTP access — set AllowedHostPermission to All or Only" | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| RIG-01 | 18-01-PLAN.md | `WasiHttpClient` implements `HttpClientExt` over wasi:http/outgoing-handler | SATISFIED | `impl HttpClientExt for WasiHttpClient` in src/http.rs; wstd transport; no reqwest |
| RIG-02 | 18-02-PLAN.md | Five built-in tools with typed args/output and JSON Schema | SATISFIED | All 5 tools exist and compile; all args structs derive `JsonSchema`; `schema_for!` in all `definition()` methods |
| RIG-03 | 18-03-PLAN.md | `WavsMemory` with KV-backed history, append, retrieve, token budget truncation | SATISFIED | src/memory.rs: all three methods; truncation loop; char/4 heuristic; namespaced keys |
| RIG-04 | 18-03-PLAN.md | `WavsAgent` + `run_agent` bridges async loop to WASI via single `block_on` | SATISFIED | src/agent.rs: trait + shim; single `block_on`; JSON output |
| RIG-05 | 18-03-PLAN.md | Startup validation for `AllowedHostPermission::None` | SATISFIED | src/permissions.rs: `check_http_permission` returns clear error string for `HttpPermission::None` |

All 5 requirements claimed by Phase 18 plans are accounted for and satisfied. No orphaned requirements (FORK-01 through FORK-05 belong to Phase 17; E2E-01 through E2E-03 belong to Phase 19).

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found | — | No TODOs, FIXMEs, placeholders, or empty stub implementations in any wavs-rig source file | — | — |

The Plan 02 deviation (LogTool using `eprintln!` instead of `wasi:logging`) is documented as an intentional design decision: `host::log()` is unavailable in rlib crates. `eprintln!` is the correct fallback — the WAVS runtime captures stderr. This is NOT a stub or anti-pattern.

### Human Verification Required

#### 1. WasiHttpClient Live Transport Test

**Test:** Create a minimal WASI component that imports wavs-rig, constructs a `WasiHttpClient`, builds an Anthropic API request with an Authorization header and JSON body, calls `send()`, and returns the response status.
**Expected:** The request exits the sandbox via wasi:http/outgoing-handler, the Authorization header is forwarded (not stripped), the API returns 200, and the response body is deserialized correctly.
**Why human:** Cannot test live outbound HTTP from a wasm32-wasip2 binary without a running WAVS node with wasi:http host support configured. Compile verification confirms the trait is correctly implemented but cannot validate runtime HTTP routing.

#### 2. AllowedHostPermission::None End-to-End Error Path

**Test:** Deploy a minimal agent component that calls `check_http_permission(&HttpPermission::None)` at startup and observe what happens in the WAVS node UI/logs.
**Expected:** The component returns the string "WAVS agent requires HTTP access — set AllowedHostPermission to All or Only" as the error response before any network activity occurs.
**Why human:** The `HttpPermission` enum is a local mirror type. The actual mapping from `host::get_service().service.permissions.allowed_http_hosts` (an `AllowedHostPermission` WIT type) to `HttpPermission` must be performed by the consuming component — this wiring cannot be verified in the rlib itself. Needs end-to-end testing.

#### 3. WavsMemory Cross-Invocation Persistence and Truncation

**Test:** Deploy a component using `WavsMemory`, invoke it 10 times appending 500-character messages each time (total ~1250 tokens, exceeding DEFAULT_TOKEN_BUDGET=4000 at ~3125 chars/message), then retrieve history.
**Expected:** Oldest messages are dropped; history stays within the token budget; conversation correctly persists to KV between separate component invocations via the WAVS host.
**Why human:** KV persistence across invocations requires a live WAVS node with wasi:keyvalue host bindings active. The wit_bindgen-generated bindings are present and the code logic is correct, but actual KV round-trip cannot be verified without the runtime.

### Gaps Summary

No gaps. All 5 ROADMAP success criteria are met at the code level. All 5 requirements (RIG-01 through RIG-05) are fully implemented and verified. The 3 human verification items are runtime integration checks that require a live WAVS node — they are not blockers to the crate's structural completeness.

---

_Verified: 2026-04-20T18:00:00Z_
_Verifier: Claude (gsd-verifier)_
