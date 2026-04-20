---
phase: 19-example-agent-e2e-validation
plan: 01
subsystem: examples/agent-example
tags: [wasm, rig, anthropic, agent, wasi]
requires: [packages/wavs-rig, packages/rig-wasi]
provides: [examples/components/agent-example]
affects: [packages/rig-wasi/src/providers, packages/wavs-rig/src/lib.rs]
tech-stack-added: []
tech-stack-patterns: [WavsAgent trait, run_agent shim, WasiHttpClient, KvSetTool]
key-files-created:
  - examples/components/agent-example/Cargo.toml
  - examples/components/agent-example/src/lib.rs
  - packages/wavs-rig/src/anthropic.rs
key-files-modified:
  - Cargo.toml (workspace members + rig-wasi dependency)
  - packages/rig-wasi/src/lib.rs (P7: un-gate providers)
  - packages/rig-wasi/src/providers/mod.rs (P7: gate non-anthropic providers)
  - packages/rig-wasi/src/providers/anthropic/client.rs (P7: cfg-conditional type aliases)
  - packages/rig-wasi/src/providers/anthropic/completion.rs (P7: streaming stub)
  - packages/rig-wasi/src/providers/anthropic/mod.rs (P7: gate streaming module)
  - packages/rig-wasi/src/providers/anthropic/model_listing.rs (P7: cfg-conditional type alias)
  - packages/rig-wasi/FORK_BASIS.md (document P7 patch)
  - packages/wavs-rig/src/lib.rs (expose anthropic module)
decisions:
  - P7 rig-wasi patch: expose providers::anthropic on wasm32-wasip2 with streaming stubbed out
  - wavs_rig::anthropic::build_client() as WASM-safe factory avoids ClientBuilder type inference issues
  - PromptError -> anyhow::Error conversion requires explicit .map_err(|e| anyhow::anyhow!("{e}")) on WASM
completed: 2026-04-20
duration_minutes: ~45
tasks_completed: 2
tasks_total: 2
files_created: 3
files_modified: 9
---

# Phase 19 Plan 01: Agent Example Component Summary

**One-liner:** cdylib WASI agent component (~90 lines) demonstrating full trigger→Anthropic LLM reasoning→KvSetTool→structured JSON result loop, compiling clean to wasm32-wasip2.

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Create agent-example crate and register in workspace | 68b9d88 | examples/components/agent-example/Cargo.toml, Cargo.toml |
| 2 | Implement agent component with ~30 lines domain logic | 0c3b140 | examples/components/agent-example/src/lib.rs, packages/wavs-rig/src/anthropic.rs, packages/rig-wasi/* |

## What Was Built

### examples/components/agent-example/

A `cdylib` WASI component that implements the full agent loop:

1. **HTTP permission check** — reads `AllowedHostPermission` from WIT host, maps to `HttpPermission`, calls `check_http_permission()` to fail fast if LLM access is blocked
2. **API key from env** — reads `WAVS_ENV_ANTHROPIC_API_KEY` (never hardcoded)
3. **Raw trigger extraction** — accepts `TriggerData::Raw(bytes)` as the prompt
4. **Agent execution** — `run_agent()` as the sole `block_on` boundary wraps `WavsAgent::run` which builds an Anthropic client + agent with `KvSetTool`, calls `agent.prompt(&prompt).await`
5. **Structured result** — returns `AgentResult { prompt, answer }` serialized to JSON

### packages/wavs-rig/src/anthropic.rs

New module providing `build_client(api_key: &str) -> Result<Client<WasiHttpClient>>` — a clean WASM-safe factory for Anthropic clients that avoids type inference complexity with `ClientBuilder`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] P7 rig-wasi patch: expose providers::anthropic on wasm32-wasip2**

- **Found during:** Task 2
- **Issue:** `rig::providers` is entirely gated behind `#[cfg(not(target_family = "wasm"))]` in `rig-wasi/src/lib.rs` (P4 patch) because providers use SSE streaming which is unavailable on WASM. This blocked `use rig::providers::anthropic` from compiling on `wasm32-wasip2`.
- **Fix:**
  - Un-gated `pub mod providers` in `lib.rs`
  - In `providers/mod.rs`: kept `pub mod anthropic` ungated; added `#[cfg(not(target_family = "wasm"))]` to all other 19 providers
  - In `providers/anthropic/mod.rs`: gated `pub mod streaming` behind `cfg(all(not(target_family = "wasm"), feature = "reqwest"))`
  - In `providers/anthropic/completion.rs`: gated streaming import and `stream()` method; added `WasmNoStreamingResponse` stub type for WASM builds; gated `CompletionModel<T>` type parameter default behind reqwest feature
  - In `providers/anthropic/client.rs`: gated `Client<H>` and `ClientBuilder<H>` type aliases to use `H = ()` default instead of `H = reqwest::Client` on WASM/no-reqwest
  - In `providers/anthropic/model_listing.rs`: same type alias fix for `AnthropicModelLister<H>`
  - Documented as P7 patch in `FORK_BASIS.md`
- **Files modified:** packages/rig-wasi/src/lib.rs, providers/mod.rs, providers/anthropic/{mod,client,completion,model_listing}.rs, FORK_BASIS.md
- **Commit:** 0c3b140

**2. [Rule 3 - Blocking] WASM PromptError conversion requires explicit .map_err**

- **Found during:** Task 2
- **Issue:** `agent.prompt(&prompt).await?` fails on WASM because `PromptError` contains `CompletionError` which contains `Box<dyn StdError>` (without Send+Sync bounds). `anyhow::Error` requires `Send + Sync` for the `?` operator.
- **Fix:** Changed to `.map_err(|e| anyhow::anyhow!("{e}"))?`
- **Commit:** 0c3b140

**3. [Rule 3 - Blocking] Missing CompletionClient + Prompt traits in scope**

- **Found during:** Task 2  
- **Issue:** `client.agent()` and `agent.prompt()` methods require `CompletionClient` and `Prompt` traits to be in scope
- **Fix:** Added `use rig::client::completion::CompletionClient;` and `use rig::completion::Prompt;` imports
- **Commit:** 0c3b140

**4. [Rule 2 - Missing critical] wavs-rig anthropic module**

- **Found during:** Task 2
- **Issue:** `ClientBuilder::default()` for the anthropic type alias creates `ClientBuilder<AnthropicBuilder, AnthropicKey, H>` but the `Default` impl only works for `NeedsApiKey` middle param. Direct usage of the type alias `default()` fails.
- **Fix:** Added `wavs_rig::anthropic::build_client()` function that uses `ClientBuilder::<AnthropicBuilder>::default().api_key(...).http_client(WasiHttpClient::default()).build()` pattern internally
- **Files modified:** packages/wavs-rig/src/anthropic.rs (new), packages/wavs-rig/src/lib.rs
- **Commit:** 0c3b140

## Verification

```
cargo check -p agent-example --target wasm32-wasip2  # passes, no errors
grep "impl WavsAgent for ExampleAgent" examples/components/agent-example/src/lib.rs  # PASS
grep "run_agent" examples/components/agent-example/src/lib.rs  # PASS
grep "check_http_permission" examples/components/agent-example/src/lib.rs  # PASS
grep "WAVS_ENV_ANTHROPIC_API_KEY" examples/components/agent-example/src/lib.rs  # PASS
grep "export_layer_trigger_world!" examples/components/agent-example/src/lib.rs  # PASS
wc -l examples/components/agent-example/src/lib.rs  # 92 lines
```

## Known Stubs

None — the agent component is functionally complete for compilation. E2E deployment and live execution are covered in Plan 02.

## Threat Flags

No new threat surface introduced beyond what the plan's threat model covers:
- `WAVS_ENV_ANTHROPIC_API_KEY` read from env (T-19-01: mitigated)
- `trigger_data` parsed as UTF-8 (T-19-02: accepted per plan)
- `AllowedHostPermission::Only` check at startup (T-19-03: accepted per plan)

## Self-Check: PASSED

- `examples/components/agent-example/Cargo.toml` — FOUND
- `examples/components/agent-example/src/lib.rs` — FOUND
- `packages/wavs-rig/src/anthropic.rs` — FOUND
- Commit 68b9d88 — FOUND
- Commit 0c3b140 — FOUND
