---
phase: 14-activity-frontend-ux
plan: "01"
subsystem: app/frontend
tags: [activity-feed, ux, decoding, submission]
dependency_graph:
  requires: [phase-13-activity-backend-pipeline]
  provides: [ACT-03, ACT-04]
  affects: [app/src/components/activity, app/src/utils]
tech_stack:
  added: []
  patterns: [discriminated-union-decode, stopPropagation-clipboard-copy, virtualizer-height-tuning]
key_files:
  created:
    - app/src/utils/decodeResultPayload.ts
  modified:
    - app/src/components/activity/ActivityCard.tsx
    - app/src/components/activity/GroupedActivityCard.tsx
    - app/src/components/activity/ActivityFeed.tsx
decisions:
  - "Use Math.floor for byte array length to safely handle odd-length hex strings"
  - "TextDecoder with fatal:true for strict UTF-8 validation before attempting JSON parse"
  - "SubmissionRows uses bgColor prop to match parent card background for CSS divider knockout"
  - "ResultPreview uses custom row layout (not DetailRow) to avoid break-all vs whitespace-pre-wrap CSS conflict"
  - "e.stopPropagation() in TxHashDisplay to prevent clipboard click from toggling card expand"
metrics:
  duration: ~15 minutes
  completed: 2026-04-09T14:20:34Z
  tasks_completed: 2
  tasks_total: 2
  files_changed: 4
---

# Phase 14 Plan 01: Activity Frontend UX Summary

**One-liner:** Inline submission display with hex-to-UTF8-to-JSON decode chain, clipboard copy, and format badges on activity cards.

## What Was Built

Added inline submission info to activity cards so users see tx hash and decoded result payloads without expanding. Created a pure decode utility and three React sub-components integrated into both ActivityCard and GroupedActivityCard.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create decodeResultPayload utility | 89b7af23 | app/src/utils/decodeResultPayload.ts |
| 2 | Add SubmissionRows, TxHashDisplay, ResultPreview + update virtualizer height | d4a3f2ea | ActivityCard.tsx, GroupedActivityCard.tsx, ActivityFeed.tsx |

## Key Decisions

1. **Math.floor for hex parsing** — PLAN specified this to handle odd-length hex safely (pitfall 3 from RESEARCH). Odd hex length truncates the last nibble rather than producing NaN.

2. **TextDecoder fatal:true** — Strict UTF-8 validation. Any non-UTF-8 byte sequence throws and falls to hex fallback immediately without partial decode.

3. **bgColor prop on SubmissionRows** — The CSS "submission" divider label uses absolute positioning over a horizontal rule. The label background must match the card background to create the knockout effect. ActivityCard uses `bg-charcoal-dark`, child card in GroupedActivityCard uses `bg-charcoal-darkest`.

4. **Custom result row layout** — `ResultPreview` is wrapped in a flex container matching DetailRow structure but without the `break-all` span, allowing `whitespace-pre-wrap` on the JSON `<pre>` tag without conflict.

5. **e.stopPropagation() in clipboard handler** — The GroupedActivityCard header is a click target for expand/collapse. Without stopPropagation, clicking the clipboard button would also toggle the card.

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None. All data flows from `ActivityItem.txHash` and `ActivityItem.resultPayload` which are populated by the Phase 13 backend pipeline.

## Threat Flags

None. All new surface matches the threat model in the plan (T-14-01 through T-14-04 accepted or mitigated by Phase 13's 4KB cap).

## Self-Check: PASSED

Files created/exist:
- app/src/utils/decodeResultPayload.ts — FOUND
- app/src/components/activity/ActivityCard.tsx — FOUND (modified)
- app/src/components/activity/GroupedActivityCard.tsx — FOUND (modified)
- app/src/components/activity/ActivityFeed.tsx — FOUND (modified)

Commits:
- 89b7af23 — FOUND
- d4a3f2ea — FOUND

TypeScript: zero errors (verified with `node_modules/.bin/tsc --noEmit`)
