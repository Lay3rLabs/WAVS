---
phase: 06-unified-activity-frontend
plan: 01
subsystem: ui
tags: [react, typescript, zustand, hooks, activity-feed]

# Dependency graph
requires: []
provides:
  - GroupedActivityEvent interface grouping trigger+submission by correlationId
  - useGroupedActivity hook with single-pass grouping, first-write-wins, orphan collection
  - StatusFilter type and STATUS_TABS constant for tab model
  - appStore FIFO eviction guard preserving submission_failed items (ERR-02)
affects: [06-02-unified-activity-frontend]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "useMemo-based grouping hook: accepts flat ActivityItem[], returns grouped + orphan arrays"
    - "ERR-02 eviction guard: separate failed items before FIFO trim, re-merge sorted by id"

key-files:
  created:
    - app/src/hooks/useGroupedActivity.ts
  modified:
    - app/src/types/index.ts
    - app/src/stores/appStore.ts

key-decisions:
  - "First-write-wins on duplicate correlationId in grouping map (defensive, not last-write)"
  - "Orphan submissions (no correlationId or no matching trigger) collected separately, not discarded"
  - "Failed items preserved indefinitely while evictable items follow 2000-cap; clearActivity still clears all"

patterns-established:
  - "Single-pass grouping: one loop over sourceList building Map<correlationId, GroupedActivityEvent>"
  - "Status progression: pending -> complete | failed driven by submission item kind"

requirements-completed: [ERR-02]

# Metrics
duration: 12min
completed: 2026-04-07
---

# Phase 6 Plan 01: Grouped Activity Data Model Summary

**useGroupedActivity hook with single-pass correlationId grouping and appStore ERR-02 eviction guard preserving failed events from FIFO removal**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-04-07T00:00:00Z
- **Completed:** 2026-04-07T00:12:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Created `useGroupedActivity` hook: single-pass iteration groups `ActivityItem[]` by correlationId into `GroupedActivityEvent[]` plus orphan `ActivityItem[]`
- Exported `GroupedActivityEvent` interface, `StatusFilter` type, and `STATUS_TABS` constant for Plan 02 UI consumption
- Added re-exports to `app/src/types/index.ts` so Plan 02 can import from the canonical types barrel
- Modified `appStore.addActivity` to guard `submission_failed` items from the 2000-cap FIFO eviction (ERR-02 compliance)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create useGroupedActivity hook with GroupedActivityEvent type and status filter types** - `1db2422b` (feat)
2. **Task 2: Add failed event eviction guard to appStore** - `be56461b` (feat)

## Files Created/Modified

- `app/src/hooks/useGroupedActivity.ts` - New hook: GroupedActivityEvent interface, StatusFilter type, STATUS_TABS constant, useGroupedActivity hook
- `app/src/types/index.ts` - Added re-exports for GroupedActivityEvent, StatusFilter, STATUS_TABS at bottom of file
- `app/src/stores/appStore.ts` - Modified addActivity to separate and preserve submission_failed items during FIFO trim

## Decisions Made

- First-write-wins on duplicate correlationId: defensive against malformed data; the first trigger seen for a key "owns" the group
- Orphan submissions (kind=submission/submission_failed with no matching trigger in Map) are collected in orphans array rather than discarded, preserving data for Plan 02 to render
- clearActivity action unchanged; it remains a user-initiated full clear unaffected by the ERR-02 guard

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- TypeScript compilation via tsc in the worktree shows pre-existing errors (missing node_modules for react, zustand, etc.) — these are infrastructure-only, not introduced by this plan. Our new files introduce zero additional errors beyond the pre-existing missing-module baseline.

## Known Stubs

None - no placeholder data or TODO values. The hook is fully functional data logic with no UI rendering.

## Threat Flags

None - no new network endpoints, auth paths, file access patterns, or schema changes introduced. All changes are in-memory React state only.

## Next Phase Readiness

- Plan 02 can directly import `useGroupedActivity`, `GroupedActivityEvent`, `StatusFilter`, and `STATUS_TABS` from `app/src/hooks/useGroupedActivity` or via the `app/src/types/index.ts` barrel
- appStore `activityList` now correctly preserves failed events; UI components can rely on this invariant
- No blockers for Plan 02

---
*Phase: 06-unified-activity-frontend*
*Completed: 2026-04-07*
