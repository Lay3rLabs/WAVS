---
gsd_state_version: 1.0
milestone: v3.0
milestone_name: Agent Composition
status: defining_requirements
last_updated: "2026-04-22"
last_activity: 2026-04-22
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-22)

**Core value:** Developers can write an autonomous LLM agent in ~30 lines of Rust, compile it to WASM, deploy it as a WAVS service, and have it reason + act on triggers with full sandbox and cryptographic trust guarantees.
**Current focus:** Defining v3.0 requirements

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-04-22 — Milestone v3.0 started

## Accumulated Context

### Decisions

- v2.0: Fork rig-core (Option B — thin fork); ~7 patches, all isolated
- v2.0: Sequential tool execution for WASI MVP
- v2.0: AtomicBool stub for PauseControl (streaming not used in WASI)
- v3.0: Agent continuation — both agent-decided and developer-defined multi-step
- v3.0: Auto-persist state to KV between steps (with developer override)
- v3.0: Service-to-service calls — synchronous first, async later
- v3.0: Permission-based service calling (AllowedServiceCalls in service.json)

### Pending Todos

None yet.

### Blockers/Concerns

None yet.
