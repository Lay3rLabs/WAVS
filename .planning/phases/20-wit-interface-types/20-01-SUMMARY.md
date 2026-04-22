---
phase: 20-wit-interface-types
plan: "01"
subsystem: wit-definitions, engine-bindings
tags: [wit, wasm, agent-composition, bindgen, backward-compatible]
dependency_graph:
  requires: []
  provides: [WIT-step-result-variant, WIT-agent-interface, WIT-call-service-import, engine-call-service-stub]
  affects: [packages/engine, examples/components/_helpers]
tech_stack:
  added: []
  patterns: [additive-wit-extension, wasmtime-bindgen-stub]
key_files:
  created: []
  modified:
    - wit-definitions/operator/wit/operator.wit
    - packages/engine/src/bindings/operator/host.rs
key_decisions:
  - "call_service stub uses Result<Vec<u8>, String> — wasmtime bindgen does NOT wrap with outer wasmtime::Result for inline host interface functions"
  - "agent interface declared as standalone named interface (not bare world-level export) to keep GuestAgent trait separate from Guest trait"
  - "%continue used as escaped WIT keyword for continue variant in step-result"
metrics:
  duration_minutes: 20
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
  completed_date: "2026-04-22T14:47:11Z"
requirements_completed: [WIT-01, WIT-02]
---

# Phase 20 Plan 01: WIT Interface Types Summary

**One-liner:** Additive WIT extension adding step-result variant, agent named interface, and call-service host import to operator.wit@2.7.0 with host-side stub and verified backward-compatible bindgen compilation.

## What Was Built

Extended `wit-definitions/operator/wit/operator.wit` with three additive changes required for v3.0 agent composition:

1. **step-result variant** — Added to `interface output`, providing `done(list<wasm-response>)` and `%continue(string)` arms for agent step returns.

2. **agent named interface** — Standalone `interface agent` with `run-agent: func(trigger-action) -> result<step-result, string>`. Declared as a named interface (not a bare world export) so that wit-bindgen generates a separate `GuestAgent` trait, leaving the existing `Guest` trait (for `run`) unchanged.

3. **call-service host import** — Added to the `import host: interface {}` block inside `world wavs-world`. Added `use output.{step-result}` and `export agent;` to the world. Existing `export run` is unchanged.

Updated `packages/engine/src/bindings/operator/host.rs` with a `call_service` stub returning `Err("call-service not yet implemented (Phase 22)")`.

## Verification Results

- `grep "step-result"` — 4 matches in operator.wit (variant declaration + uses)
- `grep "%continue"` — 1 match in operator.wit (escaped keyword correct)
- `grep "call-service"` — 1 match in operator.wit
- `grep "export agent"` — 1 match in operator.wit
- `grep "export run:"` — 1 match in operator.wit (unchanged)
- `cargo check -p wavs-engine` — PASS
- `cargo check -p example-helpers` — PASS (component-side unaffected)
- `cargo test -p wavs-engine --lib` — PASS (0 tests, backward compatible)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Adjusted call_service return type**
- **Found during:** Task 2 compilation
- **Issue:** Plan suggested `wasmtime::Result<Result<Vec<u8>, String>>` as a possible signature, but wasmtime bindgen for inline host interface functions generates `Result<Vec<u8>, String>` (no outer wasmtime::Result wrapper). Initial stub with the double-wrapped return caused E0053.
- **Fix:** Used `Result<Vec<u8>, String>` with `Err("call-service not yet implemented (Phase 22)".into())`
- **Files modified:** packages/engine/src/bindings/operator/host.rs
- **Commit:** c62db2031

## Commits

| Task | Description | Hash |
|------|-------------|------|
| 1 | feat(20-01): add step-result variant, agent interface, and call-service import to operator.wit | a4d62be12 |
| 2 | feat(20-01): add call_service stub to host.rs; verify both bindgen sites compile | c62db2031 |

## Known Stubs

| File | Description |
|------|-------------|
| packages/engine/src/bindings/operator/host.rs | call_service always returns Err — functional implementation deferred to Phase 22 |

## Threat Flags

None. No new trust boundaries, network endpoints, or runtime-evaluated surfaces introduced. WIT is consumed at compile time only; call_service stub returns immediate Err.

## Self-Check: PASSED

- [x] `wit-definitions/operator/wit/operator.wit` exists and contains all required additions
- [x] `packages/engine/src/bindings/operator/host.rs` contains call_service stub
- [x] Commits a4d62be12 and c62db2031 exist in git log
- [x] `cargo check -p wavs-engine` passes
- [x] `cargo check -p example-helpers` passes
