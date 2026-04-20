# WAVS Agent Runtime

## What This Is

WAVS is a platform for running sandboxed WebAssembly services with cryptographic trust guarantees. v1.0–v1.3 shipped developer experience improvements: OCI distribution, WIT-to-schema, MCP execution with three trust tiers, open-source AI providers, component explorer, activity feed UX, and service reliability fixes. v2.0 makes WAVS a first-class agent runtime — developers write rig-based agents in ~30 lines of Rust that autonomously reason and act inside the WASM sandbox.

## Current Milestone: v2.0 Agent Runtime

**Goal:** Make WAVS a first-class agent runtime by integrating rig-core into the WASI sandbox, so developers can build autonomous LLM agents that reason and act on triggers with full sandbox guarantees.

**Target features:**
- WASI-compatible rig fork (reqwest optional, tokio optional, cfg detection fixes)
- `wavs-rig` integration crate with 4 bridges (HTTP transport, tools, async shim, memory)
- Built-in WAVS host function tools (KV get/set, EVM query, HTTP fetch, structured logging)
- KV-backed conversation memory with token budget management
- Example agent component demonstrating full agent loop (trigger → LLM reasoning → tool use → result)
- End-to-end deployment and execution on WAVS node

## Core Value

Developers can write an autonomous LLM agent in ~30 lines of Rust, compile it to WASM, deploy it as a WAVS service, and have it reason + act on triggers with the same sandbox and cryptographic trust guarantees as any other WAVS component.

## Requirements

### Validated

- ✓ Sandboxed WASM component execution via Wasmtime — existing
- ✓ Per-component network policy (`All` / `Only` / `None` on `AllowedHostPermission`) — existing
- ✓ Cryptographic result signatures by operators — existing
- ✓ Multi-operator execution with configurable quorum — existing
- ✓ EVM and Cosmos blockchain read/write — existing
- ✓ Event-driven execution (EVM logs, Cosmos events, HTTP webhooks, cron) — existing
- ✓ MCP server for service management (deploy, upload, register, simulate) — existing (`wavs-mcp`)
- ✓ Tauri 2 desktop app with wallet, health, service management, logging — existing
- ✓ Self-governing service configuration via on-chain actors — existing
- ✓ OCI component pull — `oci://` URIs in service.json, digest-verified pull and caching — v1.0
- ✓ WIT-to-schema tooling — JSON Schema from component WIT interfaces — v1.0
- ✓ End-user MCP execution interface — deployed components as callable MCP tools — v1.0
- ✓ Three trust tiers per tool call: result only / signed result / on-chain submission — v1.0
- ✓ Event correlation IDs — trigger/submission events linked by correlationId — v1.0
- ✓ Submission failure surfacing — SubmissionFailed events reach GUI with error messages — v1.0
- ✓ Settings page decomposition — sidebar-navigated layout with isolated section components — v1.0
- ✓ Unified activity frontend — nested parent-child events with status filtering and error display — v1.0
- ✓ Groq & OpenRouter agent providers — selectable from settings dropdown with API key persistence — v1.1
- ✓ Ollama agent provider — custom base URL, models.json generation, ModelRegistry.create() for local models — v1.1
- ✓ Settings scroll refactor — single scrollable page with IntersectionObserver sidebar tracking — v1.1
- ✓ Tauri commands exposing wit-schema JSON Schema and component metadata — v1.2
- ✓ Component detail page with full interface profile (functions, permissions, config) — v1.2
- ✓ Improved components list with search/filter, richer cards, and detail navigation — v1.2
- ✓ Richer activity cards with trigger, result summary, and submission info visible without expanding — v1.3
- ✓ Smart result decoding for activity feed (UTF-8 → JSON → hex) — v1.3
- ✓ Service restart reliability fix — v1.3
- ✓ Wallet settings kebab dropdown for uncommon actions — v1.3

### Active

<!-- Current scope. Building toward these. -->

- [ ] WASI-compatible rig fork with reqwest/tokio optional and cfg fixes
- [ ] `wavs-rig` integration crate bridging rig into WASI sandbox
- [ ] WAVS host functions exposed as typed rig tools
- [ ] KV-backed conversation memory for agent state persistence
- [ ] Example agent component with full LLM reasoning loop
- [ ] End-to-end agent deployment and execution on WAVS node

### Out of Scope

- Demo/doc the `Only` allowlist variant — tracked separately, different repo
- OCI component publishing tooling — deferred to future phase (pull-only shipped)
- Wassette feature parity comparison docs — marketing concern, not code
- MCP stdio transport signing — Stdio is local-process; trust boundary is machine-level

## Context

**Current State:** v1.3 complete. App polish cycle done (v1.0–v1.3). Now pivoting to runtime-level agent support.

**Tech stack:** Rust (node, CLI, MCP server, types), Tauri 2 + React 19 + Vite 7 (desktop app), Wasmtime (WASI component execution), Zustand (frontend state). New for v2.0: rig-core (Rust agent framework), wasm32-wasip2 target.

**Key context for v2.0:**
- rig-core has WASM-compatible traits (`WasmCompatSend`/`WasmCompatSync`, `HttpClientExt`) but hard blockers: unconditional reqwest, tokio rt feature, cfg inconsistencies. ~300-500 line fork needed.
- WAVS already has `wasi:http/outgoing-handler` and `wasi:keyvalue` host functions — these become the rig bridges.
- `AllowedHostPermission` (`All`/`Only`/`None`) enforces network policy on LLM API calls at the Wasmtime level.
- Sequential tool execution is fine for MVP (WASI is single-threaded). Configure rig concurrency to 1.
- Existing components use `wstd::runtime::block_on` for async — rig's agent loop needs to work within this.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Extend wavs-mcp rather than separate server | Single MCP server for both management and execution reduces user friction | ✓ Good — clean integration |
| OCI pull-only for v1 (no publish tooling) | Lower scope; publishing adds complexity without immediate user value | ✓ Good |
| Three trust tiers as explicit agent choice | Matches the "dial not binary" positioning; agents pick what they need | ✓ Good |
| WIT-to-schema before MCP execution | Auto-generated tool descriptions are core to the Wassette-parity experience | ✓ Good |
| correlation_id as String not Uuid type | Avoids bincode derive complications; String implements bincode natively | ✓ Good |
| OAuth listener in parent Settings.tsx | Survives section navigation since parent never unmounts | ✓ Good |
| Client-side correlationId grouping | Simple, no backend changes needed; single-pass useMemo in ActivityFeed | ✓ Good |
| Status-based filter tabs (All/Pending/Failed/Complete) | More useful than kind-based (trigger/submission) now that events are nested | ✓ Good |
| Groq/OpenRouter as KnownProviders | Already in pi-ai — zero Rust changes, just UI + settings.json read at startup | ✓ Good |
| models.json for Ollama (not registerProvider) | Declarative, auto-reloads on /model calls, no extension code | ✓ Good |
| IntersectionObserver for scroll tracking | Native, performant, no scroll event spam — sidebar highlight stays in sync | ✓ Good |
| result_payload as Option<String> pre-encoded hex | serde_helpers module is private; pre-encode in aggregator avoids cross-crate dep | ✓ Good |
| 4KB cap on result_payload at aggregator | Prevents 100MB hex blowup in Tauri IPC; enforced before channel send | ✓ Good |
| Pending subscription queue for EVM triggers | Standard async ordering fix; queues commands before controller ready, drains after | ✓ Good |
| Kebab menu for uncommon wallet actions | Reduces vertical space; groups rare destructive actions behind disclosure | ✓ Good |
| Fork rig-core rather than build from scratch | Rig has 20+ LLM providers, typed tools, WASM-compat traits. Reimplementing = months of work. Fork ~300-500 lines of platform patches. | — Pending |
| Option B (thin fork) for MVP, upstream later | Move fast, patches are isolated to platform layer. If upstream accepts, drop the fork. | — Pending |
| Sequential tool execution for WASI MVP | Single-threaded sandbox; concurrent tool calls add complexity without benefit | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-20 after v2.0 milestone start*
