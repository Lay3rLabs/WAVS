---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Agent Runtime
status: defining_requirements
stopped_at: Milestone v2.0 started; defining requirements
last_updated: "2026-04-20"
last_activity: 2026-04-20
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-20)

**Core value:** Developers can write an autonomous LLM agent in ~30 lines of Rust, compile it to WASM, deploy it as a WAVS service, and have it reason + act on triggers with the same sandbox and cryptographic trust guarantees as any other WAVS component.
**Current focus:** Defining requirements for v2.0 Agent Runtime

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-04-20 — Milestone v2.0 started

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- v2.0: Fork rig-core (Option B — thin fork) rather than build agent framework from scratch
- v2.0: Sequential tool execution for WASI MVP (single-threaded sandbox)
- v2.0: Runtime-level changes (Continue/checkpoint, service-to-service RPC) deferred to post-MVP

### Pending Todos

None yet.

### Blockers/Concerns

- rig-core has hard WASI blockers: unconditional reqwest, tokio rt, cfg inconsistencies. Fork required (~300-500 lines).
- Async runtime shim: rig uses tokio internally, WASI uses wstd::runtime::block_on. Compatibility TBD.
- LLM API latency: 10-turn agent loop may take 30-60s. Need to verify WAVS timeout/fuel limits are sufficient.

## Session Continuity

Last session: 2026-04-20
Stopped at: Milestone v2.0 started; defining requirements
Resume file: None
