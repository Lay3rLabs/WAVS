---
phase: 20-wit-interface-types
plan: 02
subsystem: types
tags: [service-config, serde, permissions, continuation, agent-composition]
dependency_graph:
  requires: []
  provides: [AllowedServiceCalls, AllowedCallers, max_continuation_steps]
  affects: [packages/engine, Phase 21, Phase 22]
tech_stack:
  added: []
  patterns: [serde-default-enum, option-skip-serializing, backward-compat-json]
key_files:
  modified:
    - packages/types/src/service.rs
decisions:
  - AllowedCallers and AllowedServiceCalls modeled on existing AllowedHostPermission pattern for consistency
  - Both enums default to None (deny-all) as safe default per threat model
  - Component fields use Option<T> + skip_serializing_if pattern matching existing exec_enabled pattern
  - ComponentDigest in test JSON uses raw 64-char hex (no sha256: prefix) — deserialized via const_hex::decode
metrics:
  duration_minutes: 15
  completed: "2026-04-22"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 1
requirements: [WIT-03, WIT-04, WIT-05]
---

# Phase 20 Plan 02: Service Config Permission Types Summary

Added three new permission/config types to `packages/types/src/service.rs` with full serde backward compatibility. These types define the permission schema for service-to-service calls and continuation limits consumed by the engine in Phases 21-22.

## What Was Built

**AllowedServiceCalls enum** — caller-side permission controlling which services a component may invoke via `call-service`. Modeled identically on `AllowedHostPermission` with `All`/`Only(Vec<String>)`/`None` variants, defaulting to `None`.

**AllowedCallers enum** — callee-side permission controlling which services may call this component. Same structure as `AllowedServiceCalls`.

**Permissions.allowed_service_calls field** — added to `Permissions` struct which already has `#[serde(default)]` at struct level, so no per-field annotation needed.

**Component.allowed_callers field** — `Option<AllowedCallers>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.

**Component.max_continuation_steps field** — `Option<u32>` with same serde attributes. Engine reads as `unwrap_or(10)`.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 | `03106ed6d` | AllowedServiceCalls enum + Permissions field + permission_defaults test extension |
| Task 2 | `f6ebd0cca` | AllowedCallers enum + Component fields + 3 new backward-compat tests |

## Test Results

All 20 wavs-types tests pass:
- `service::permission_defaults` — verifies AllowedServiceCalls defaults to None
- `service::component_new_fields_backward_compat` — verifies existing service.json loads without change
- `service::component_allowed_callers_variants` — verifies All variant and max_continuation_steps parsing
- `service::allowed_service_calls_variants` — verifies All/Only/None variant deserialization

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ComponentDigest test JSON used incorrect sha256: prefix format**
- **Found during:** Task 2 test execution
- **Issue:** Plan's test JSON used `"sha256:0000..."` but `ComponentDigest::from_str` uses `const_hex::decode_to_slice` which expects raw hex without prefix — causes "odd number of digits" parse error
- **Fix:** Changed test JSON to use plain 64-char hex `"0000..."` matching the actual serialization format
- **Files modified:** packages/types/src/service.rs (test JSON)
- **Commit:** f6ebd0cca

**2. [Rule 1 - Bug] Component::new() test helper was missing new fields**
- **Found during:** Task 2 — compiler error when adding fields without updating struct literal
- **Fix:** Added `allowed_callers: None, max_continuation_steps: None` to the `mod test_ext` impl block
- **Files modified:** packages/types/src/service.rs
- **Commit:** f6ebd0cca

**3. Discovery: Cargo ran against wrong directory**
- When running `cd /workspace/WAVS && cargo test`, tests ran against the main repo tree, not the worktree. The worktree is at `/workspace/WAVS/.claude/worktrees/agent-a5c37214/` — cargo must be invoked from within the worktree directory.

## Known Stubs

None — these are pure type definitions with no rendering or data flow stubs.

## Threat Flags

No new trust boundaries. These types define schema only — enforcement is Phase 22.

## Self-Check: PASSED

- `packages/types/src/service.rs` contains `pub enum AllowedServiceCalls` — FOUND
- `packages/types/src/service.rs` contains `pub enum AllowedCallers` — FOUND
- `packages/types/src/service.rs` contains `pub allowed_service_calls: AllowedServiceCalls` — FOUND
- `packages/types/src/service.rs` contains `pub allowed_callers: Option<AllowedCallers>` — FOUND
- `packages/types/src/service.rs` contains `pub max_continuation_steps: Option<u32>` — FOUND
- Commit `03106ed6d` exists — FOUND
- Commit `f6ebd0cca` exists — FOUND
- All 20 wavs-types tests pass — VERIFIED
