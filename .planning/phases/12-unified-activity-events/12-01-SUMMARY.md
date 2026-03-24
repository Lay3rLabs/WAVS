---
phase: 12-unified-activity-events
plan: 01
subsystem: ui, api
tags: [tauri, zustand, event-pipeline, correlation, unified-activity]

# Dependency graph
requires:
  - phase: 09-foundation-types-and-settings-refactor
    provides: Settings types, service registry, Tauri event infrastructure
provides:
  - SubmissionEvent with tx_hash field on successful on-chain submission
  - SubmissionErrorEvent with error_message on failed submission
  - DispatcherCommand::SubmissionError variant for error routing
  - UnifiedActivity type replacing ActivityItem with correlation key and status tracking
  - Map-based Zustand store with handleTrigger/handleSubmission/handleSubmissionError
  - Tauri listener for submission_error events
  - correlationKey function for deterministic trigger-to-submission matching
affects: [12-02-PLAN, activity-ui, activity-feed]

# Tech tracking
tech-stack:
  added: []
  patterns: [map-based-correlation-store, event-pipeline-error-forwarding]

key-files:
  created: []
  modified:
    - packages/gui/shared/src/event.rs
    - packages/wavs/src/dispatcher.rs
    - packages/wavs/src/subsystems/aggregator.rs
    - app/src/types/index.ts
    - app/src/stores/appStore.ts
    - app/src/tauri/listeners.ts

key-decisions:
  - "Map-based correlation store keyed by deterministic correlationKey for O(1) trigger-to-submission matching"
  - "Orphaned submissions create standalone entries rather than being dropped"
  - "Fixed pre-existing incomplete Settings initialization in appStore (missing mcp/env fields)"

patterns-established:
  - "Event correlation: correlationKey(serviceId, workflowId, triggerData) produces deterministic key from trigger-specific fields"
  - "Three-handler pattern: handleTrigger (create pending), handleSubmission (confirm), handleSubmissionError (mark error)"

requirements-completed: [ACT-01, ACT-02, ACT-03]

# Metrics
duration: 6min
completed: 2026-03-24
---

# Phase 12 Plan 01: Event Pipeline Summary

**Complete event pipeline from Rust backend error/success emission through Tauri IPC to Map-based Zustand correlation store with UnifiedActivity type**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-24T20:12:23Z
- **Completed:** 2026-03-24T20:18:51Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Backend emits SubmissionEvent with tx_hash on successful on-chain submission and SubmissionErrorEvent with error_message on failure
- Frontend UnifiedActivity type replaces ActivityItem with correlation key, status (pending/confirmed/error), txHash, and errorMessage
- Zustand store uses Map-based correlation with three dedicated event handlers instead of flat array + addActivity
- Tauri listeners wire trigger, submission, and submission_error events to the new store actions
- correlationKey function provides deterministic trigger-to-submission matching based on trigger-specific identifying fields

## Task Commits

Each task was committed atomically:

1. **Task 1: Backend event pipeline** - `30df2fb6` (feat)
2. **Task 2: Frontend types, correlation store, and event listeners** - `2c97c35e` (feat)

## Files Created/Modified
- `packages/gui/shared/src/event.rs` - Added tx_hash to SubmissionEvent, added SubmissionErrorEvent struct
- `packages/wavs/src/dispatcher.rs` - Added SubmissionError variant to DispatcherCommand, tx_hash to SubmissionConfirmed, error event emission handler
- `packages/wavs/src/subsystems/aggregator.rs` - Emit tx_hash on success, emit SubmissionError on failure
- `app/src/types/index.ts` - Replaced ActivityItem/ActivityKind with UnifiedActivity/ActivityStatus, added correlationKey function, added SubmissionErrorEvent interface
- `app/src/stores/appStore.ts` - Replaced flat array with Map-based correlation store, three event handlers
- `app/src/tauri/listeners.ts` - Added submission_error listener, updated trigger/submission listeners to use new handlers

## Decisions Made
- Map-based correlation store keyed by deterministic correlationKey for O(1) trigger-to-submission matching
- Orphaned submissions (received without a prior trigger) create standalone entries rather than being dropped
- Fixed pre-existing incomplete Settings initialization in appStore (missing mcp_enabled, mcp_auto_start, mcp_token, env_vars, saved_services fields)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed incomplete Settings initialization in appStore**
- **Found during:** Task 2 (Frontend store implementation)
- **Issue:** Pre-existing bug: Settings initial value was missing saved_services, mcp_enabled, mcp_auto_start, mcp_token, env_vars fields
- **Fix:** Added all missing fields to the initial Settings object literal
- **Files modified:** app/src/stores/appStore.ts
- **Verification:** TypeScript compilation passes for appStore.ts
- **Committed in:** 2c97c35e (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Pre-existing type mismatch that was always there but hidden by non-strict builds. No scope creep.

## Known Stubs

None -- all data flows are wired end-to-end. ActivityCard.tsx and ActivityFeed.tsx still reference old ActivityItem/ActivityKind types but those will be converted in Plan 02.

## Issues Encountered
- ActivityCard.tsx and ActivityFeed.tsx produce TypeScript errors due to referencing removed ActivityItem/ActivityKind types -- this is expected and documented in the plan as resolved by Plan 02

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Event pipeline fully wired from backend through Tauri IPC to Zustand store
- Plan 02 can now build UI components that consume UnifiedActivity from the store
- ActivityCard and ActivityFeed need updating to use UnifiedActivity type (Plan 02 scope)

## Self-Check: PASSED

All 7 files verified present. Both task commits (30df2fb6, 2c97c35e) confirmed in git log.

---
*Phase: 12-unified-activity-events*
*Completed: 2026-03-24*
