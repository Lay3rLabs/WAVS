# Research Summary: WAVS v2.0 Agent Runtime

**Synthesized:** 2026-04-20
**Sources:** STACK.md, FEATURES.md, ARCHITECTURE.md, PITFALLS.md

## Executive Summary

WAVS v2.0 integrates rig-core 0.35.0 (20+ LLM providers, typed Tool trait) into the WASI sandbox via a ~300-500 line fork (`rig-wasi`) and a new bridge library crate (`packages/wavs-rig`). The entire integration lives inside the WASM boundary — the WAVS node, engine, WIT definitions, and aggregator are all unchanged. The approach leverages existing WASI host functions (`wasi:http`, `wasi:keyvalue`, `host::log`) that are already wired and working.

## Stack Additions

| Dependency | Version | Purpose |
|-----------|---------|---------|
| rig-core (fork) | 0.35.0 | Agent framework — LLM providers, Tool trait, agent loop |
| wstd | 0.6.6 | WASI async runtime (upgrade from 0.6.5) |
| futures | 0.3.x | Channel replacement for tokio::sync::watch |

**Fork patches (~6 files):** reqwest optional, tokio rt removed, cfg unified to `target_family = "wasm"`, SSE dead zones fixed, getrandom wasm_js feature removed.

## Feature Priorities

### Table Stakes
- WASI HTTP transport (HttpClientExt over wasi:http)
- Async runtime shim (wstd::runtime::block_on)
- WavsAgent trait + run_agent entry point
- Working example agent component

### Differentiators (free from existing WAVS)
- Network sandboxing on LLM calls (AllowedHostPermission)
- Cryptographic signatures on agent results
- Multi-operator execution
- Built-in typed tools for WAVS host functions (KV, EVM, HTTP, logging)
- KV-backed conversation memory with token budget enforcement

### Anti-features (explicitly excluded)
- Streaming LLM responses (single-threaded WASI, no SSE consumer)
- Concurrent tool execution (requires threading)
- RAG/vector store (P3 future)
- Continue/checkpoint mode (P1 future)
- Service-to-service RPC (P1 future)

## Architecture

**Zero engine changes.** Two new workspace members: `packages/wavs-rig` (rlib) and `examples/components/agent-defi-monitor` (cdylib). One line change to `WAVS/Cargo.toml`.

**Data flow:** Trigger → engine invokes component → `run_agent` calls rig agent loop → rig calls LLM via WasiHttpClient → rig dispatches tools (KV, EVM, HTTP) → agent returns WasmResponse → engine returns to aggregator.

**Build order (strict sequential):**
1. rig-wasi fork compiles to wasm32-wasip2
2. packages/wavs-rig depends on fork
3. Example agent component depends on wavs-rig

## Critical Pitfalls

1. **Single block_on constraint:** Entire agent loop must run inside one `wstd::runtime::block_on`. Nested executors deadlock.
2. **KV memory unbounded growth:** Without token budget enforcement at write time, conversation history poisons KV after ~50 invocations.
3. **Fuel budget calibration:** Agent components need higher fuel budgets than simple query components — each wasi:http call is expensive.
4. **AllowedHostPermission::None = silent death:** HTTP trap failure looks like a crash. Startup validation required.
5. **Fork divergence:** Pin to git rev with FORK_BASIS.md. Rig releases every 2-3 weeks.

## Suggested Phase Structure

| Phase | Focus | Depends On |
|-------|-------|------------|
| 1 | rig-wasi fork — compile to wasm32-wasip2 | — |
| 2 | packages/wavs-rig — 4 bridges + built-in tools | Phase 1 |
| 3 | Example agent + E2E validation on WAVS node | Phase 2 |

Phase ordering is strict — Phase 1 is a compile-time prerequisite for Phase 2, which is a prerequisite for Phase 3. No parallelism between phases.
