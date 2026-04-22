---
phase: 20-wit-interface-types
verified: 2026-04-22T15:13:47Z
status: gaps_found
score: 3/5 must-haves verified
gaps:
  - truth: "A WASM component compiled against the updated operator.wit can export both legacy run and new run-agent simultaneously — existing components continue to load without modification"
    status: partial
    reason: "WIT declarations are correct and component-side bindgen (example-helpers) compiles. However, host-side bindgen (wavs-engine) fails to compile with 12 errors including 2 directly caused by phase 20. cargo check -p wavs-engine exits non-zero, so the engine cannot actually instantiate any components."
    artifacts:
      - path: "packages/engine/src/bindings/types/component_to_wavs.rs"
        issue: "Struct literal for wavs_types::Component at line 135 is missing new fields allowed_callers and max_continuation_steps added by phase 20. Struct literal for wavs_types::Permissions at line 184 is missing new field allowed_service_calls added by phase 20."
    missing:
      - "Add `allowed_callers: None, max_continuation_steps: None` to the Component struct literal in component_to_wavs.rs:135"
      - "Add `allowed_service_calls: wavs_types::AllowedServiceCalls::None` to the Permissions struct literal in component_to_wavs.rs:184"
      - "Note: 10 additional pre-existing engine compile errors (Oci variant removed, digest() API change, exec_enabled field) introduced by a simultaneous worktree merge also need resolution"

  - truth: "The WIT call-service host import is declared in the operator world and wit-bindgen regenerates bindings without errors — downstream Rust code can reference call_service() as a typed function"
    status: partial
    reason: "call-service is correctly declared in operator.wit and the call_service stub exists in host.rs. The component-side bindgen regenerates without errors. However, host-side wavs-engine fails cargo check, so downstream Rust code in the engine package cannot currently reference call_service() in a compiled state."
    artifacts:
      - path: "packages/engine/src/bindings/types/component_to_wavs.rs"
        issue: "Engine fails to compile due to missing new fields from phase 20, preventing the host-side bindings from being usable"
    missing:
      - "Fix the two phase-20-caused missing field errors in component_to_wavs.rs (see gap 1 above)"
---

# Phase 20: WIT Interface Types Verification Report

**Phase Goal:** The interface contract for agent composition is locked in — `operator.wit` has the additive `run-agent` export returning `Continue`/`Done` variants, the `call-service` host import is declared, and all new permission/config fields exist in `service.json` types with correct serde defaults
**Verified:** 2026-04-22T15:13:47Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from Roadmap Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| SC1 | WASM component can export both `run` and `run-agent` simultaneously; existing components load without modification | PARTIAL | WIT correct, example-helpers compiles, but wavs-engine fails cargo check (12 errors, 2 phase-20-caused) |
| SC2 | WIT `call-service` host import declared; wit-bindgen regenerates without errors; downstream Rust can reference `call_service()` | PARTIAL | WIT correct, call_service stub in host.rs, component-side compiles, but host-side engine fails compilation |
| SC3 | `service.json` with `allowed_service_calls: "None"` (or absent) deserializes correctly via serde default | VERIFIED | `cargo test -p wavs-types --lib` passes all 20 tests including `service::allowed_service_calls_variants` and `service::permission_defaults` |
| SC4 | `max_continuation_steps` field in component config, defaults to 10 when absent | VERIFIED | `service::component_new_fields_backward_compat` test asserts `unwrap_or(10)` == 10 |
| SC5 | `AllowedCallers` field in service config with serde default `None`; callee services can declare permitted callers without breaking existing configs | VERIFIED | `service::component_allowed_callers_variants` and backward compat tests pass |

**Score:** 3/5 truths fully verified (SC3, SC4, SC5 pass; SC1, SC2 partial due to engine compilation failure)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `wit-definitions/operator/wit/operator.wit` | step-result variant, agent interface, call-service host import | VERIFIED | Contains `variant step-result`, `%continue(string)`, `interface agent`, `call-service`, `export agent`, `export run:` — all additive, version @2.7.0 unchanged |
| `packages/engine/src/bindings/operator/host.rs` | call_service stub returning Err | VERIFIED | `fn call_service` at line 108 returns `Err("call-service not yet implemented (Phase 22)")` |
| `packages/types/src/service.rs` | AllowedServiceCalls, AllowedCallers, max_continuation_steps | VERIFIED | All three types/fields present with correct serde defaults |
| `packages/engine/src/bindings/types/component_to_wavs.rs` | Struct literals updated for new fields | STUB/BROKEN | Permissions literal missing `allowed_service_calls`, Component literal missing `allowed_callers` and `max_continuation_steps` — causes E0063 compilation errors |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `wit-definitions/operator/wit/operator.wit` | `packages/engine/src/bindings/operator/world.rs` | `wasmtime::component::bindgen!` path reference | VERIFIED | `path: "../../wit-definitions/operator/wit"` present at line 7 |
| `wit-definitions/operator/wit/operator.wit` | `examples/components/_helpers/src/bindings/world.rs` | `wit_bindgen::generate!` path reference | VERIFIED | `path: "../../../wit-definitions/operator/wit"` present at line 7 |
| `packages/types/src/service.rs` | `packages/engine` | `wavs_types` crate dependency | BROKEN | Engine consumes wavs_types but component_to_wavs.rs struct literals are incomplete for new fields — cargo check -p wavs-engine fails |

### Data-Flow Trace (Level 4)

Not applicable — this phase produces type definitions and WIT declarations, not dynamic data-rendering components.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| operator.wit contains step-result variant | `grep -c "step-result" wit-definitions/operator/wit/operator.wit` | 4 matches | PASS |
| operator.wit contains %continue escaped keyword | `grep -c "%continue" wit-definitions/operator/wit/operator.wit` | 1 match | PASS |
| operator.wit contains call-service import | `grep -c "call-service" wit-definitions/operator/wit/operator.wit` | 1 match | PASS |
| operator.wit has export agent in world | `grep -c "export agent" wit-definitions/operator/wit/operator.wit` | 1 match | PASS |
| operator.wit preserves existing export run | `grep -c "export run:" wit-definitions/operator/wit/operator.wit` | 1 match | PASS |
| host.rs call_service stub exists | `grep "call_service" host.rs` | Found at line 108 | PASS |
| cargo check -p wavs-engine | compile check | FAIL — 12 errors (2 phase-20-caused, 10 pre-existing from restore commit) | FAIL |
| cargo check -p example-helpers | compile check | PASS — Finished dev profile | PASS |
| cargo test -p wavs-types --lib | 20 unit tests | PASS — 20 passed, 0 failed | PASS |
| package version unchanged | `grep "^package" operator.wit` | `wavs:operator@2.7.0` | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| WIT-01 | 20-01-PLAN.md | `operator.wit` exports `run-agent` returning `result<step-result, string>` with `done`/`continue` variants, backward-compatible | PARTIAL | WIT declarations correct and complete; component-side bindgen passes; host-side engine compilation fails preventing full SC1 verification |
| WIT-02 | 20-01-PLAN.md | `call-service` host import added to operator world | PARTIAL | WIT declared correctly; stub in host.rs; component-side passes; engine fails to compile |
| WIT-03 | 20-02-PLAN.md | `AllowedServiceCalls` type (All/Only/None) on Permissions with serde default None | VERIFIED | Enum exists, Permissions has field, tests pass |
| WIT-04 | 20-02-PLAN.md | `AllowedCallers` type on service config, default None | VERIFIED | Enum exists, Component has `allowed_callers: Option<AllowedCallers>`, tests pass |
| WIT-05 | 20-02-PLAN.md | `max_continuation_steps` field on component config, default 10 | VERIFIED | `Option<u32>` field exists with `unwrap_or(10)` verified by test |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `packages/engine/src/bindings/types/component_to_wavs.rs` | 184 | Struct literal for `wavs_types::Permissions` missing `allowed_service_calls` field added by phase 20 | BLOCKER | Causes E0063 compile error; engine crate cannot compile |
| `packages/engine/src/bindings/types/component_to_wavs.rs` | 135 | Struct literal for `wavs_types::Component` missing `allowed_callers` and `max_continuation_steps` fields added by phase 20 | BLOCKER | Causes E0063 compile error; engine crate cannot compile |
| `packages/engine/src/bindings/types/component_to_wavs.rs` | 114 | `exec_enabled: None` field reference but `wavs_types::Service` no longer has this field (pre-existing from worktree merge) | BLOCKER | E0560 compile error; not phase-20-caused |
| `packages/engine/src/common/base_engine.rs` | 112, 130, 159 | `ComponentSource::Oci` variant and `digest()` API mismatches (pre-existing from worktree merge) | BLOCKER | Multiple E0308/E0599 compile errors; not phase-20-caused |

**Note on pre-existing errors:** The restore commit `8da5fed90` ("chore: restore .planning after worktree merge; keep 20-02 code changes") reverted `base_engine.rs` to an older version that is incompatible with the current `wavs_types` API. This produced 10 of the 12 engine compile errors. These pre-date phase 20's functional scope but occurred during the same session. Phase 20 directly caused 2 of the 12 errors (the E0063 missing field errors in `component_to_wavs.rs`).

### Human Verification Required

None — all verification is code/compilation based.

### Gaps Summary

Phase 20 successfully implemented all WIT declarations and Rust type definitions. The `operator.wit` additions are complete and correct. The service.rs types (AllowedServiceCalls, AllowedCallers, max_continuation_steps) are fully implemented with correct serde defaults and verified by 20 passing unit tests. The component-side bindgen compiles cleanly.

The phase fell short on one critical integration point: when `AllowedServiceCalls` was added to `Permissions` and `AllowedCallers`/`max_continuation_steps` to `Component` in `service.rs`, the struct literal construction in `packages/engine/src/bindings/types/component_to_wavs.rs` was not updated to include the new fields. This causes `cargo check -p wavs-engine` to fail with E0063 errors, which means:

1. The host-side bindings for the entire engine crate do not compile
2. SC1 and SC2 (engine can instantiate components with new WIT) cannot be confirmed as working

The fix is minimal — three field additions to two struct literal blocks in `component_to_wavs.rs`:
- Line 135: Add `allowed_callers: None, max_continuation_steps: None` to the Component literal
- Line 184: Add `allowed_service_calls: wavs_types::AllowedServiceCalls::default()` to the Permissions literal

Additionally, the pre-existing engine errors (Oci variant, digest() API, exec_enabled) should be addressed, though they are not phase-20-caused.

---

_Verified: 2026-04-22T15:13:47Z_
_Verifier: Claude (gsd-verifier)_
