---
phase: 11-bls-service-builder-and-registration
plan: 01
subsystem: ui
tags: [react, tauri, bls, service-builder, dropdown, zustand]

# Dependency graph
requires:
  - phase: 09-foundation-types-and-settings-refactor
    provides: "SignatureAlgorithm type, SubmitDraft.signatureAlgorithm field, serviceBuilderStore"
provides:
  - "ALGORITHM_OPTIONS dropdown in SubmitEditor (ECDSA/BLS selector)"
  - "isBLSService() helper function in types/index.ts"
  - "Post-deploy BLS G1 pubkey display card in ServiceDeploy"
affects: [11-02-bls-registration, service-detail-page]

# Tech tracking
tech-stack:
  added: []
  patterns: ["BLS-conditional UI rendering via isBls derived value", "Fallback chain: getServiceSigner -> deriveBlsPubkey"]

key-files:
  created: []
  modified:
    - app/src/types/index.ts
    - app/src/components/service/SubmitEditor.tsx
    - app/src/components/service/ServiceDeploy.tsx

key-decisions:
  - "Used useMemo with service object to detect BLS rather than reading from store directly -- avoids coupling to store internals"
  - "Fallback from getServiceSigner to deriveBlsPubkey(0) for BLS key display -- covers case where service not yet recognized by running node"

patterns-established:
  - "BLS detection pattern: check wf.submit.aggregator.signature_kind.algorithm === 'bls12381' across workflows"
  - "Post-deploy conditional card: render extra UI when deploy completes AND service has specific properties"

requirements-completed: [BLS-01, BLS-02]

# Metrics
duration: 2min
completed: 2026-03-24
---

# Phase 11 Plan 01: BLS Service Builder Summary

**BLS algorithm selector dropdown in SubmitEditor with auto-prefix-reset, isBLSService helper, and post-deploy BLS G1 pubkey card with copy-to-clipboard**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-24T17:31:01Z
- **Completed:** 2026-03-24T17:33:15Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Added Signature Algorithm dropdown (ECDSA/BLS) as first field in aggregator options panel
- BLS selection auto-sets signaturePrefix to 'none' (BLS does not use EIP-191)
- Added isBLSService() helper to types/index.ts for reuse across components
- Post-deploy BLS key card renders with AddressDisplay (copy-to-clipboard), loading, and error states

## Task Commits

Each task was committed atomically:

1. **Task 1: Add isBLSService helper and algorithm selector to SubmitEditor** - `abe15c45` (feat)
2. **Task 2: Add post-deploy BLS key display to ServiceDeploy** - `5716c1e7` (feat)

## Files Created/Modified
- `app/src/types/index.ts` - Added isBLSService() helper function
- `app/src/components/service/SubmitEditor.tsx` - Added ALGORITHM_OPTIONS dropdown with BLS auto-prefix logic
- `app/src/components/service/ServiceDeploy.tsx` - Added BLS pubkey state, post-deploy fetch, and BLS Operator Key card

## Decisions Made
- Used useMemo with service object to detect BLS rather than reading signatureAlgorithm from store directly -- decouples from store shape
- Fallback from getServiceSigner to deriveBlsPubkey(0) for BLS key display -- covers case where service not yet recognized by running node

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Vite build command needed to run from app/ directory (not project root) -- trivial path issue, resolved immediately.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- BLS algorithm selector and post-deploy key display complete
- Plan 02 (BLS registration on service detail page) can proceed -- isBLSService helper and types are in place
- ServiceDetailPage modification (BLS-03, BLS-04) will use the same isBLSService pattern established here

## Self-Check: PASSED

All files verified present, all commits verified in git log.

---
*Phase: 11-bls-service-builder-and-registration*
*Completed: 2026-03-24*
