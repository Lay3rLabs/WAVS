---
phase: 16-wallet-kebab-menu
plan: "01"
subsystem: app/frontend
tags: [ui, settings, wallet, kebab-menu]
dependency_graph:
  requires: []
  provides: [wallet-kebab-menu]
  affects: [app/src/components/settings/WalletSection.tsx]
tech_stack:
  added: []
  patterns: [kebab-dropdown, click-outside-useEffect, useRef-containment]
key_files:
  created: []
  modified:
    - app/src/components/settings/WalletSection.tsx
decisions:
  - Kebab hidden during showMnemonic and showResetConfirm states to avoid UI confusion
  - Used document mousedown listener with kebabRef containment for click-outside close
metrics:
  duration: ~5 minutes
  completed: "2026-04-09T16:29:01Z"
  tasks_completed: 2
  files_modified: 1
---

# Phase 16 Plan 01: Wallet Kebab Menu Summary

Three-dot kebab dropdown replacing inline Export Recovery Phrase and Reset Wallet buttons in the wallet settings card header.

## What Was Built

Replaced the two always-visible inline buttons in `WalletSection.tsx` with a kebab (three-dot vertical dots) icon in the card header. The icon only appears when there's a mnemonic and no active flow (mnemonic display or reset confirmation). Clicking it opens a dropdown with both actions. "Reset Wallet" uses `text-red-4` for destructive action signaling.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add kebab dropdown menu to WalletSection | 0e74c7ba | app/src/components/settings/WalletSection.tsx |
| 2 | Verify kebab menu visually (checkpoint) | auto-approved (autonomous mode) | — |

## Key Changes

- Added `useRef` to React import
- Added `kebabOpen` state and `kebabRef` for DOM reference
- Added `useEffect` for click-outside dismissal via document `mousedown` listener
- Wrapped `<h2>` in a flex row with kebab button in the header
- Kebab conditionally rendered: `hasMnemonic && !showMnemonic && !showResetConfirm`
- Dropdown button for Export Recovery Phrase wired to `handleExportWallet()`
- Dropdown button for Reset Wallet wired to `setShowResetConfirm(true)` with `text-red-4`
- Removed standalone inline `<Button text="Export Recovery Phrase">` block
- Removed standalone inline `<Button text="Reset Wallet">` block

## Verification

- All acceptance criteria greps pass: `kebabOpen`, `useRef`, `kebabRef`, `Wallet actions`, `Export Recovery Phrase`, `Reset Wallet`, `text-red-4`
- Inline standalone Button components confirmed absent
- TypeScript compiles without errors (`app/node_modules/.bin/tsc --noEmit`)

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None.

## Threat Flags

None. This is a pure UI reorganization; no new trust boundaries or data flows introduced.

## Self-Check: PASSED

- File exists: app/src/components/settings/WalletSection.tsx - FOUND
- Commit 0e74c7ba - FOUND
