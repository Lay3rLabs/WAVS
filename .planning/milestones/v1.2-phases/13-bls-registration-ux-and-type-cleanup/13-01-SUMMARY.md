---
phase: 13-bls-registration-ux-and-type-cleanup
plan: 01
subsystem: ui
tags: [typescript, types, bls, registration, guidance-banner, tailwind]

# Dependency graph
requires:
  - phase: 11-bls-operator-key-display-and-registration
    provides: BLS key display and registration flow in ServiceDetailPage
provides:
  - Unified SignaturePrefix type alias as single source of truth
  - BLS guidance banner for missing POA registry in ServiceDetailPage
affects: [service-builder, service-detail, type-system]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Centralized type alias with import (no inline unions) for domain enums"

key-files:
  created: []
  modified:
    - app/src/types/index.ts
    - app/src/stores/serviceBuilderStore.ts
    - app/src/components/service/SubmitEditor.tsx
    - app/src/pages/services/ServiceDetailPage.tsx

key-decisions:
  - "No new decisions -- followed plan as specified"

patterns-established:
  - "SignaturePrefix type alias is the single source of truth for signature prefix values"

requirements-completed: [BLS-03, FND-01]

# Metrics
duration: 2min
completed: 2026-03-24
---

# Phase 13 Plan 01: BLS Registration UX and Type Cleanup Summary

**Unified SignaturePrefix type alias across 3 files and added amber guidance banner for BLS services missing POA registry**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-24T23:30:14Z
- **Completed:** 2026-03-24T23:32:32Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Widened SignaturePrefix type alias to `'eip191' | 'none'` and eliminated all inline union duplications
- Replaced local SigPrefix type in SubmitEditor with imported SignaturePrefix
- Added amber guidance banner in ServiceDetailPage when BLS service has no connected POA registry
- Verified TypeScript type check (tsc --noEmit) and production build (vite build) both pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix SignaturePrefix type drift across types, store, and editor** - `351dc998` (fix)
2. **Task 2: Add BLS registration guidance banner for missing POA registry** - `d5f81f7f` (feat)

## Files Created/Modified
- `app/src/types/index.ts` - Widened SignaturePrefix from `'eip191'` to `'eip191' | 'none'`
- `app/src/stores/serviceBuilderStore.ts` - Added SignaturePrefix import, replaced inline union in SubmitDraft
- `app/src/components/service/SubmitEditor.tsx` - Imported SignaturePrefix, removed local SigPrefix type
- `app/src/pages/services/ServiceDetailPage.tsx` - Added amber guidance banner in BLS section when registry is null

## Decisions Made
None - followed plan as specified.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all changes are fully wired with no placeholder data.

## Next Phase Readiness
- Phase 13 is complete (single-plan phase)
- SignaturePrefix type is now the canonical source of truth for all signature prefix values
- BLS registration UX provides clear guidance when registry is missing

## Self-Check: PASSED

- All 5 files verified present on disk
- Both task commits (351dc998, d5f81f7f) verified in git history

---
*Phase: 13-bls-registration-ux-and-type-cleanup*
*Completed: 2026-03-24*
