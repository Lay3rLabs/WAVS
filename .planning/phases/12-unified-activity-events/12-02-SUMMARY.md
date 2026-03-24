---
phase: 12-unified-activity-events
plan: 02
subsystem: ui
tags: [tauri, react, activity-card, status-badge, filter]

# Dependency graph
requires:
  - phase: 12-unified-activity-events
    plan: 01
    provides: UnifiedActivity type, correlation store, event listeners

key-files:
  created: []
  modified:
    - app/src/components/activity/ActivityCard.tsx
    - app/src/components/activity/ActivityFeed.tsx
    - app/src/types/index.ts
    - app/src/stores/appStore.ts
---

## What Was Built

Rewrote the activity UI components to render unified event cards with status progression and inline error display. Added smart detection for no-submission services.

### ActivityCard
- **StatusBadge**: Color-coded badge showing PENDING (amber), EXECUTED (blue), CONFIRMED (green), or ERROR (red)
- **SubmissionSection**: Shows tx_hash for confirmed cards, error message with scroll cap for error cards
- **Status-based borders**: Card border color reflects current status
- Removed old "Trigger"/"Submit" kind badge entirely

### ActivityFeed
- **Status filter tabs**: All / Pending / Executed / Confirmed / Error (replaces old kind filter)
- Updated types from ActivityItem/ActivityKind to UnifiedActivity/ActivityStatus throughout
- Virtualizer re-measures on status updates (card height changes when submission section appears)

### No-Submission Service Detection
- `handleTrigger` in store looks up `workflow.submit` from the services map
- Services with `submit: 'none'` get status `'executed'` immediately instead of stuck at `'pending'`

## Deviations

- **Added 'executed' status**: Not in original plan. User identified that no-submission services would be perpetually stuck at "pending". Added a 5th status (`executed`, blue) that the store sets automatically when `workflow.submit === 'none'`.

## Self-Check: PASSED

- [x] ActivityCard imports UnifiedActivity (not ActivityItem)
- [x] StatusBadge renders 4 statuses with correct colors
- [x] SubmissionSection shows tx_hash or error message
- [x] ActivityFeed uses StatusFilter with 5 tabs
- [x] No references to old ActivityItem/ActivityKind/addActivity remain
- [x] `cargo check -p wavs-app` passes
- [x] `npx vite build` passes
