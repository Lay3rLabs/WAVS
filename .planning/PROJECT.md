# WAVS Improvements

## What This Is

Developer experience and capability improvements to the WAVS platform, closing gaps identified through comparative analysis with Microsoft Wassette (v0.4.0). Three features that position WAVS as the natural upgrade path from Wassette for AI agent developers: WIT-to-schema tooling, an end-user MCP execution interface with three trust tiers, and OCI-based component distribution.

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

### Active

- [ ] WIT-to-schema tooling — auto-generate JSON Schema from component WIT interfaces
- [ ] End-user MCP execution interface — deployed service components surfaced as callable MCP tools
- [ ] Three trust tiers per tool call: result only / result + signature / on-chain submission
- [ ] OCI component pull — `oci://` URIs in service.json, WAVS fetches and caches at deploy time

### Out of Scope

- Demo/doc the `Only` allowlist variant — tracked separately, different repo
- OCI component publishing tooling — deferred to future phase (pull-only for now)
- Wassette feature parity comparison docs — marketing concern, not code
- Changes to the Tauri desktop app — this milestone is platform/MCP focused

## Context

**Strategic framing:** WAVS is a strict superset of Wassette. The trust model is a dial, not a binary: (1) sandboxed execution, (2) signed results, (3) blockchain interactions. Developers who just want a better Wassette use mode 1. Those who need verifiable results use mode 2. Mode 3 is there when on-chain permanence is needed. The pitch: start where Wassette starts, go further when you need to.

**Current positioning gap:** WAVS is presented primarily as a blockchain AVS platform. The sandbox angle is undersold — and that's exactly where Wassette is gaining traction with AI agent developers.

**MCP execution model:** Agent developers deploy a service first (via existing wavs-mcp management tools or CLI). The execution interface then surfaces that service's components as callable MCP tools — one tool per component/workflow. The agent picks the trust tier per call.

**Dependency chain:** WIT-to-schema enables auto-generated tool descriptions, which powers the MCP execution interface. OCI pull is an independent track.

**Existing codebase:** Tauri 2 + React 19 desktop app, Rust WAVS node, wavs-mcp (MCP management server), wavs-cli. Most MVP features shipped (see `app/PLAN.md`).

**Wassette reference:** Microsoft Wassette v0.4.0 (March 2026) — security-oriented MCP server executing AI agent tools as WASM Components. Has OCI distribution (12 curated components via ghcr.io), `component2json` WIT-to-schema crate, and end-user tool execution. Lacks signatures, multi-operator, blockchain, event-driven execution.

## Constraints

- **Tech stack**: Rust for all platform work (node, CLI, MCP server); WASI components via Wasmtime
- **Compatibility**: Must not break existing wavs-mcp management interface or deployed services
- **Dependencies**: WIT-to-schema is prerequisite for auto-generated MCP tool descriptions
- **External**: Bytecode Alliance considering upstreaming `component2json` (Wassette issue #579) — watch for upstream availability before building from scratch

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Extend wavs-mcp rather than separate server | Single MCP server for both management and execution reduces user friction | — Pending |
| OCI pull-only for v1 (no publish tooling) | Lower scope; publishing adds complexity without immediate user value | — Pending |
| Three trust tiers as explicit agent choice | Matches the "dial not binary" positioning; agents pick what they need | — Pending |
| WIT-to-schema before MCP execution | Auto-generated tool descriptions are core to the Wassette-parity experience | — Pending |

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
*Last updated: 2026-03-24 after initialization*
