# WAVS Improvements

## What This Is

Developer experience and capability improvements to the WAVS platform. Six features shipped in v1.0: OCI component distribution, WIT-to-schema tooling, MCP execution interface with three trust tiers, event correlation IDs, settings page decomposition, and unified activity feed with error surfacing. v1.1 added open-source AI providers (Groq, OpenRouter, Ollama) and settings scroll refactor. v1.2 added component detail pages with interface/schema/permissions exploration and enhanced components list.

## Current Milestone: v1.3 Activity UX & Bug Fixes

**Goal:** Improve activity feed usability with richer cards, smarter result decoding, fix service restart reliability, and streamline wallet settings.

**Target features:**
- Richer activity cards showing trigger + result + submission info (incl. tx hash) without expanding
- Smart result decoding (UTF-8 → JSON pretty-print → hex fallback) for byte vec payloads
- Fix service restart: services not restoring correctly on WAVS process restart
- Wallet settings kebab menu for uncommon actions (reset wallet, reveal seed)

## Core Value

AI agent developers can use WAVS components as MCP tools with the same ease as Wassette, but with cryptographic trust guarantees Wassette structurally cannot provide.

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

### Active

<!-- Current scope. Building toward these. -->

- [ ] Richer activity cards with trigger, result summary, and submission info visible without expanding
- [ ] Smart result decoding for activity feed (UTF-8 → JSON → hex)
- [ ] Service restart reliability fix
- [ ] Wallet settings kebab dropdown for uncommon actions

### Out of Scope

- Demo/doc the `Only` allowlist variant — tracked separately, different repo
- OCI component publishing tooling — deferred to future phase (pull-only shipped)
- Wassette feature parity comparison docs — marketing concern, not code
- MCP stdio transport signing — Stdio is local-process; trust boundary is machine-level

## Context

**Current State:** v1.3 in progress. Phase 13 complete — tx_hash and result_payload now flow from aggregator through Rust pipeline to frontend (ACT-01, ACT-02). Activity feed frontend UX (Phase 14), service restart fix (Phase 15), and wallet kebab menu (Phase 16) remain.

**Tech stack:** Rust (node, CLI, MCP server, types), Tauri 2 + React 19 + Vite 7 (desktop app), Wasmtime (WASI component execution), Zustand (frontend state).

**Known tech debt:** REQUIREMENTS.md checkboxes incomplete for phases 1-3, phases 2-3 missing VERIFICATION.md (pre-date GSD verification), 6 deferred human visual verification items, ERR-04 partial gap on orphan card path, OCI source type silently falls to "Digest" in ComponentsPage filter, redundant #[derive(Default)] on SchemaCacheState.

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

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-09 after v1.3 milestone start*
