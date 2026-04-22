---
gsd_state_version: 1.0
milestone: v3.0
milestone_name: Agent Composition
status: ready_to_plan
last_updated: "2026-04-22"
last_activity: 2026-04-22
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-22)

**Core value:** Developers can write an autonomous LLM agent in ~30 lines of Rust, compile it to WASM, deploy it as a WAVS service, and have it reason + act on triggers with full sandbox and cryptographic trust guarantees.
**Current focus:** Phase 20 — WIT Interface & Types

## Current Position

Phase: 20 of 23 (WIT Interface & Types)
Plan: 0 of TBD in current phase
Status: Ready to plan
Last activity: 2026-04-22 — v3.0 roadmap created; 4 phases, 17 requirements mapped

Progress: [░░░░░░░░░░] 0%

## Accumulated Context

### Decisions

- v2.0: Fork rig-core (Option B — thin fork); ~7 patches, all isolated
- v2.0: Sequential tool execution for WASI MVP
- v2.0: AtomicBool stub for PauseControl (streaming not used in WASI)
- v3.0: WIT strategy — additive `run-agent` export alongside existing `run`; backward-compatible with all components at wavs:operator@2.7.0
- v3.0: State persistence — KV-backed only (`wavs_agent_step:` prefix); `Continue` return carries key string only, not inline state (avoids 4KB cap)
- v3.0: `call-service` — must use `func_wrap_async`; re-entrant `Arc<WasmEngine>`, never route through Dispatcher channel
- v3.0: Security invariants ship with features — step limits with Phase 21, cycle detection + AllowedServiceCalls/AllowedCallers with Phase 22

### Pending Todos

None yet.

### Blockers/Concerns

- **Phase 22:** Verify `execute_operator_component` can be called re-entrantly within the same Tokio task without Wasmtime Store aliasing violations (Wasmtime issue #9600) — validate before Phase 22 implementation
- **Multi-operator agents:** LLM continuation agents require temperature=0 for deterministic consensus across operators; must be documented as a deployment constraint

## Session Continuity

Last session: 2026-04-22
Stopped at: Roadmap created — Phase 20 ready to plan
Resume file: None
