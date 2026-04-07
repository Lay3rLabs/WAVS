---
phase: 04-rust-event-foundation
plan: 01
subsystem: wavs-types, wavs, wavs-gui-shared, app
tags: [correlation-id, events, submission-failed, tauri, typescript]
requirements: [EVT-01, ERR-01]

dependency_graph:
  requires: []
  provides:
    - TriggerAction.correlation_id (UUID v7, all construction sites)
    - SubmissionFailedEvent (Rust + TypeScript)
    - SubmissionEvent.correlation_id (Rust + TypeScript)
    - DispatcherCommand::SubmissionFailed variant
    - ActivityItem.correlationId + error fields
  affects:
    - packages/types/src/service.rs
    - packages/wavs/src/dispatcher.rs
    - packages/wavs/src/subsystems/submission.rs
    - packages/wavs/src/subsystems/aggregator.rs
    - packages/gui/shared/src/event.rs
    - app/src/types/index.ts
    - app/src/tauri/listeners.ts

tech_stack:
  added:
    - uuid workspace dependency to wavs-types, wavs, wavs-benchmark-common, wavs-engine (dev)
  patterns:
    - UUID v7 generated at TriggerAction construction time
    - correlation_id flows: TriggerAction -> aggregator -> SubmissionConfirmed -> SubmissionEvent -> GUI
    - SubmissionFailed emitted at both signing error and dispatch error sites

key_files:
  created: []
  modified:
    - packages/types/Cargo.toml
    - packages/types/src/service.rs
    - packages/wavs/Cargo.toml
    - packages/wavs/src/subsystems/trigger.rs
    - packages/wavs/src/http/handlers/debug.rs
    - packages/wavs/src/subsystems/engine/wasm_engine.rs
    - packages/wavs/src/subsystems/submission.rs
    - packages/wavs/src/subsystems/aggregator.rs
    - packages/wavs/tests/wavs_systems/mock_trigger_manager.rs
    - packages/wavs/tests/wavs_systems/mock_submissions.rs
    - packages/wavs/tests/mock_e2e.rs
    - packages/wavs/benches/common/Cargo.toml
    - packages/wavs/benches/common/src/engine_setup.rs
    - packages/cli/src/command/exec_aggregator.rs
    - packages/cli/src/command/exec_component.rs
    - packages/engine/Cargo.toml
    - packages/engine/tests/aggregator_basic.rs
    - packages/engine/tests/helpers/service.rs
    - packages/gui/shared/src/event.rs
    - packages/wavs/src/dispatcher.rs
    - app/src/types/index.ts
    - app/src/tauri/listeners.ts
    - app/src/components/activity/ActivityCard.tsx
    - app/src/components/activity/ActivityFeed.tsx

decisions:
  - correlation_id uses String (not Uuid type) on TriggerAction to avoid uuid dependency in struct serialization
  - bincode String field needs no with_serde annotation (bincode natively supports String)
  - triggerData made optional in ActivityItem to support submission_failed events (which have no trigger data)
  - ActivityCard.tsx updated with null-safe triggerData access and error display for submission_failed

metrics:
  duration: ~25min
  completed: 2026-04-07
  tasks_completed: 3
  tasks_total: 3
  files_modified: 24
---

# Phase 4 Plan 1: Correlation ID and Submission Failed Events Summary

JWT-style correlation tracing: UUID v7 generated at trigger entry points flows through the pipeline to SubmissionEvent, with SubmissionFailedEvent surfacing previously-silent signing/dispatch failures to the desktop GUI.

## What Was Built

### Task 1: Add correlation_id to TriggerAction

Added `pub correlation_id: String` field to `TriggerAction` struct in `packages/types/src/service.rs`. Added `uuid = { workspace = true }` to `wavs-types`, `wavs`, `wavs-benchmark-common` (Cargo.toml), and `wavs-engine` (dev-dependency). Added `use uuid::Uuid` and `correlation_id: Uuid::now_v7().as_hyphenated().to_string()` at all ~22 construction sites across trigger.rs (7 sites), debug.rs (2 sites), wasm_engine.rs (10+ test sites), mock_trigger_manager.rs (4 sites), mock_submissions.rs (1 site), mock_e2e.rs (1 site), engine_setup.rs (1 site), exec_aggregator.rs (2 sites), exec_component.rs (1 site), aggregator_basic.rs (1 site), and helpers/service.rs (1 site).

### Task 2: SubmissionFailed Event Path

- Added `correlation_id: String` to `SubmissionEvent` struct in `packages/gui/shared/src/event.rs`
- Added `SubmissionFailedEvent` struct with `TauriEventExt` impl (event name: "submission_failed")
- Added `SubmissionFailed { service_id, workflow_id, correlation_id, error }` variant to `DispatcherCommand`
- Added `correlation_id` to `SubmissionConfirmed` variant
- Updated dispatcher match arms to pass/destructure correlation_id for both variants
- Added new `SubmissionFailed` match arm in dispatcher emitting `SubmissionFailedEvent` to GUI
- Added `DispatcherCommand::SubmissionFailed` sends at both signing error (with "Signing error: {}") and dispatch error (with "Dispatch error: {}") sites in `submission.rs`
- Updated `aggregator.rs` SubmissionConfirmed send to include `correlation_id: submission.trigger_action.correlation_id.clone()`

### Task 3: TypeScript Mirror

- Added `correlation_id: string` to `TriggerAction` and `SubmissionEvent` interfaces
- Added `SubmissionFailedEvent` interface
- Updated `ActivityKind` to `'trigger' | 'submission' | 'submission_failed'`
- Made `triggerData` optional in `ActivityItem` (submission_failed events have no trigger data)
- Added `correlationId?: string` and `error?: string` to `ActivityItem`
- Added `SUBMISSION_FAILED: 'submission_failed'` to EVENTS constant
- Added `SubmissionFailedEvent` to imports in `listeners.ts`
- Updated trigger listener to pass `correlationId: action.correlation_id`
- Updated submission listener to pass `correlationId: payload.correlation_id`
- Added new `unlistenSubmissionFailed` listener

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing null checks] Updated ActivityCard.tsx and ActivityFeed.tsx for optional triggerData**
- **Found during:** Task 3
- **Issue:** Making `triggerData` optional in `ActivityItem` broke existing code that passed it directly to `getTriggerDataLabel()`, `getTriggerAccent()`, and `DetailRows` which all required non-optional `TriggerData`
- **Fix:** Updated `ActivityCard.tsx` to use null-conditional calls (`item.triggerData ? ... : fallback`), added error display for `submission_failed` items, updated badge label and color for failed items; updated `ActivityFeed.tsx` search filter with null-safe triggerData access
- **Files modified:** `app/src/components/activity/ActivityCard.tsx`, `app/src/components/activity/ActivityFeed.tsx`
- **Commit:** a36a9f66

**2. [Rule 2 - Missing dependencies] Added uuid to more packages than the plan specified**
- **Found during:** Task 1
- **Issue:** The plan mentioned adding uuid to wavs-types and wavs, but the grep revealed TriggerAction construction sites in packages/engine tests, packages/cli, and packages/wavs/benches/common which also needed uuid
- **Fix:** Added uuid to engine Cargo.toml (dev-dependency), confirmed uuid already present in cli Cargo.toml
- **Files modified:** packages/engine/Cargo.toml, packages/wavs/benches/common/Cargo.toml

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | bdf84136 | feat(04-01): add correlation_id to TriggerAction and update all construction sites |
| 2 | 033b0bbc | feat(04-01): add SubmissionFailed event path and carry correlation_id through SubmissionConfirmed |
| 3 | a36a9f66 | feat(04-01): mirror Rust event changes in TypeScript types and Tauri listeners |

## Verification Results

All plan verification checks pass:
- `cargo check -p wavs-types -p wavs` exits 0
- `cargo check -p wavs-gui-shared -p wavs` exits 0
- `cargo check -p wavs-cli -p wavs-engine` exits 0
- `grep "correlation_id" packages/types/src/service.rs` finds field
- `grep "SubmissionFailed" packages/wavs/src/dispatcher.rs` finds variant and handler
- `grep -c "DispatcherCommand::SubmissionFailed" packages/wavs/src/subsystems/submission.rs` outputs `2`
- `grep "SubmissionFailedEvent" app/src/types/index.ts` finds interface
- TypeScript type errors in modified files: none (all tsc errors are pre-existing missing module declarations due to no node_modules in worktree)

## Known Stubs

None - all data flows are wired.

## Threat Flags

No new network endpoints or trust boundaries introduced. All changes are internal pipeline augmentation (UUID generation is node-internal, not user-supplied). Event emission is local Tauri IPC only.

## Self-Check

All files verified to exist:
- packages/types/src/service.rs contains `pub correlation_id: String`
- packages/gui/shared/src/event.rs contains `struct SubmissionFailedEvent`
- packages/wavs/src/dispatcher.rs contains `SubmissionFailed {` variant
- packages/wavs/src/subsystems/submission.rs contains 2x `DispatcherCommand::SubmissionFailed`
- app/src/types/index.ts contains `SubmissionFailedEvent` interface

All commits verified to exist: bdf84136, 033b0bbc, a36a9f66

## Self-Check: PASSED
