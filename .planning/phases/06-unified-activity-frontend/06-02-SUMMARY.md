---
phase: 06-unified-activity-frontend
plan: 02
subsystem: ui
tags: [react, typescript, components, activity-feed, virtualizer]

# Dependency graph
requires:
  - useGroupedActivity hook (06-01)
  - GroupedActivityEvent interface (06-01)
  - StatusFilter, STATUS_TABS types (06-01)
provides:
  - GroupedActivityCard component with nested child card, status dots, error display
  - ActivityFeed refactored with status-based filter tabs and grouped virtualizer
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "DisplayItem union type dispatching: { type: 'group'; data: GroupedActivityEvent } | { type: 'orphan'; data: ActivityItem }"
    - "Lifted expandedIds as Set<string> keyed by groupKey (string correlationId or String(trigger.id))"
    - "Status dot via conditional rendering: amber=pending, red=failed, none=complete"
    - "Independent raw JSON local state per card: rawExpanded (parent) + childRawExpanded (child)"

key-files:
  created:
    - app/src/components/activity/GroupedActivityCard.tsx
  modified:
    - app/src/components/activity/ActivityCard.tsx
    - app/src/components/activity/ActivityFeed.tsx

key-decisions:
  - "Header row is full click target for expand/collapse (role=button) per UI-SPEC"
  - "Orphan submissions bypass status filter tab — always visible regardless of active tab"
  - "Error text on child card has no truncate class — full message always visible (ERR-04)"
  - "Child card raw JSON toggle is independent local state, not part of parent expandedIds"

patterns-established:
  - "DisplayItem merge pattern: groups sorted by trigger.ts, orphans by .ts, merged and re-sorted"
  - "Shared helper exports: formatTimestamp, getTriggerAccent, DetailRow, DetailRows exported from ActivityCard"

requirements-completed: [EVT-02, EVT-03, EVT-04, EVT-05, ERR-03, ERR-04]

# Metrics
duration: 18min
completed: 2026-04-07
---

# Phase 6 Plan 02: Unified Activity Frontend Summary

**GroupedActivityCard component and ActivityFeed refactor delivering nested trigger-submission cards with amber/red status dots, full error display, and status-based filter tabs replacing kind-based tabs**

## Performance

- **Duration:** ~18 min
- **Completed:** 2026-04-07
- **Tasks:** 3 (2 auto + 1 checkpoint:human-verify)
- **Files modified:** 3

## Accomplishments

- Created `GroupedActivityCard.tsx`: renders trigger as parent card with optional nested submission child card, amber pulsing dot for pending status, red pulsing dot for failed status, no dot for complete
- Exported `formatTimestamp`, `getTriggerAccent`, `DetailRow`, `DetailRows` from `ActivityCard.tsx` for reuse by GroupedActivityCard
- Refactored `ActivityFeed.tsx` to use `useGroupedActivity` hook, replacing flat list iteration with grouped + orphan dispatch
- Replaced `KindFilter` kind-based tabs with `StatusFilter` status-based tabs: All / Pending / Failed / Complete
- Changed `expandedIds` from `Set<number>` to `Set<string>` keyed by `groupKey` (correlationId string or String(trigger.id))
- Added `DisplayItem` union type merging groups and orphans into a single virtualizer-compatible array sorted by timestamp
- Both Activity page and ServiceActivity tab display unified grouped view since both consume `ActivityFeed` (EVT-05)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create GroupedActivityCard component** - `938121dd` (feat)
2. **Task 2: Refactor ActivityFeed with status filter tabs** - `3e17f3ec` (feat)

## Checkpoint Status

**Task 3: Visual verification** (checkpoint:human-verify)

Programmatic checks all passed:
- GroupedActivityCard.tsx exists with `export function GroupedActivityCard`
- `animate-glow-amber` and `animate-glow-red` class strings present
- `aria-label="Waiting for submission"` and `aria-label="Submission failed"` present
- Child card has `bg-charcoal-darkest` and `border-charcoal-light`
- Error div has `text-red-400` with NO `truncate` class (ERR-04 compliant)
- `role="button"` on header row
- ActivityCard.tsx exports all 4 helper functions
- ActivityFeed imports `useGroupedActivity` and `GroupedActivityCard`
- No `KindFilter` references remain in ActivityFeed
- TypeScript compiles with zero errors (`/workspace/app/node_modules/.bin/tsc --noEmit`)

Human visual verification of the running app is pending — start with `just app-dev-frontend` and navigate to the Activity page.

## Files Created/Modified

- `app/src/components/activity/GroupedActivityCard.tsx` - New component: GroupedActivityCard with status dots, nested child card, full error display, independent raw JSON toggles
- `app/src/components/activity/ActivityCard.tsx` - Added `export` to formatTimestamp, getTriggerAccent, DetailRow, DetailRows
- `app/src/components/activity/ActivityFeed.tsx` - Refactored: status filter tabs, useGroupedActivity integration, DisplayItem virtualizer, grouped/orphan dispatch

## Decisions Made

- Full header row is click target for expand (role=button), Raw button has stopPropagation to prevent double-toggle
- Orphans always bypass status filter — a submission_failed orphan (no matching trigger) stays visible on "All" and hidden on Pending/Failed/Complete since orphans have no status field
- Error text intentionally has no `truncate` class: ERR-04 requires full error visibility in expanded view
- Child card uses independent `childRawExpanded` local state so opening child raw JSON does not close the parent card

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - all components wire real data from the useGroupedActivity hook and appStore.

## Threat Flags

None - error messages rendered as JSX text content (not dangerouslySetInnerHTML), search filter uses client-side string matching only, no new network endpoints or auth paths introduced.

## Self-Check

- [x] `app/src/components/activity/GroupedActivityCard.tsx` exists
- [x] `app/src/components/activity/ActivityCard.tsx` exports 4 helpers
- [x] `app/src/components/activity/ActivityFeed.tsx` refactored
- [x] Commit `938121dd` exists in git log
- [x] Commit `3e17f3ec` exists in git log
- [x] TypeScript compiles with zero errors

## Self-Check: PASSED

---
*Phase: 06-unified-activity-frontend*
*Completed: 2026-04-07*
