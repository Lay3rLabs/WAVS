---
phase: 05-settings-decomposition
plan: 01
subsystem: ui
tags: [react, typescript, tailwind, settings, sidebar, components]

# Dependency graph
requires: []
provides:
  - SettingsSidebar component with SectionKey union type (6 navigation items)
  - WalletSection isolated component with balance fetching and export/reset handlers
  - NodeSection isolated component with TOML editor and wavs_home management
  - EnvironmentSection isolated component with ENV_VAR_SUGGESTIONS
  - Settings.tsx rewritten as sidebar-navigated orchestrating shell
  - OAuth listener lifted to parent Settings component
affects: [05-02]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SectionKey union type for compile-time sidebar navigation safety"
    - "Section callback pattern: onUnsavedChange(bool) / onChanged() / onError(msg)"
    - "OAuth listener in parent pattern: listener survives section navigation"
    - "Conditional rendering (activeSection === 'x') for section isolation"

key-files:
  created:
    - app/src/components/settings/SettingsSidebar.tsx
    - app/src/components/settings/WalletSection.tsx
    - app/src/components/settings/NodeSection.tsx
    - app/src/components/settings/EnvironmentSection.tsx
  modified:
    - app/src/pages/Settings.tsx

key-decisions:
  - "OAuth listener stays in parent Settings component, not in AgentApiKeyField, to survive section unmounts"
  - "AgentApiKeyField updated to accept oauthLoading/oauthStatus/onOAuthStart as props"
  - "SettingsSidebar is pure presentational (no internal state), driven by parent activeSection"
  - "Restart banner rendered above sidebar+content row (not inside content area)"
  - "NodeSection onUnsavedChange callback drives parent hasUnsavedChanges for banner"

patterns-established:
  - "Section components own all their local state — no cross-section state reads"
  - "Sections report errors upward via onError(msg) callback"
  - "Sections report unsaved changes via onUnsavedChange(bool) callback"
  - "Active sidebar item: border-l-2 border-purple-2 bg-charcoal-medium"
  - "Inactive sidebar item: border-l-2 border-transparent hover:bg-charcoal-medium"

requirements-completed: [SET-01, SET-02, SET-03, SET-04, SET-06]

# Metrics
duration: 5min
completed: 2026-04-07
---

# Phase 5 Plan 01: Settings Decomposition — Sidebar Layout and Section Extraction Summary

**1221-line monolithic Settings.tsx decomposed into 4 isolated section components + SettingsSidebar + 615-line orchestrating shell with sidebar navigation and parent OAuth listener**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-04-07T07:32:19Z
- **Completed:** 2026-04-07T07:37:27Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Created `SettingsSidebar` pure presentational component with SectionKey type and 6 sidebar items with correct active/inactive Tailwind styling
- Extracted `WalletSection`, `NodeSection`, and `EnvironmentSection` as isolated components each owning their local state
- Rewrote `Settings.tsx` from 1221 lines to 615 lines: sidebar layout, banner above split, OAuth listener in parent, Agent/MCP/Reset sections inline ready for Plan 02

## Task Commits

1. **Task 1: Create SettingsSidebar component with SectionKey type** - `f3a74f05` (feat)
2. **Task 2: Extract WalletSection, NodeSection, and EnvironmentSection** - `2c0e279a` (feat)
3. **Task 3: Rewrite Settings.tsx as orchestrating shell with sidebar layout** - `f14c6f13` (feat)

## Files Created/Modified

- `app/src/components/settings/SettingsSidebar.tsx` — Pure presentational sidebar with 6 items, SectionKey export, active/inactive state styling
- `app/src/components/settings/WalletSection.tsx` — Wallet management with balance fetching (BalanceRow, ChainBalance, fetchBalances all moved here)
- `app/src/components/settings/NodeSection.tsx` — WAVS home dir + TOML editor with onUnsavedChange/onChanged/onError callbacks
- `app/src/components/settings/EnvironmentSection.tsx` — Env vars management with ENV_VAR_SUGGESTIONS array moved here
- `app/src/pages/Settings.tsx` — Rewritten as 615-line orchestrating shell: sidebar layout, banner above split, AgentApiKeyField updated with OAuth props, MCP/Agent/Reset inline

## Decisions Made

- AgentApiKeyField refactored to accept `oauthLoading`, `oauthStatus`, `onOAuthStart` props instead of managing OAuth listener locally — listener moved to parent Settings to survive section unmounts
- `hasUnsavedChanges` in parent driven by `NodeSection.onUnsavedChange(bool)` callback for TOML dirty tracking and `onChanged()` for browse/save events
- WalletSection uses `onError(msg)` callback for error propagation to parent rather than maintaining separate parent error state for wallet errors
- Reset section confirmation button text changed from "Yes, Clear Everything" to "Confirm Clear" to match UI-SPEC.md copywriting contract

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] Updated Reset section confirmation button text**
- **Found during:** Task 3 (Settings.tsx rewrite)
- **Issue:** Original Settings.tsx used "Yes, Clear Everything" but UI-SPEC.md copywriting contract specifies "Confirm Clear" / "Keep Services"
- **Fix:** Updated both button texts to match the UI-SPEC.md contract
- **Files modified:** app/src/pages/Settings.tsx
- **Verification:** Text matches UI-SPEC.md copywriting contract
- **Committed in:** f14c6f13 (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 correctness/spec alignment)
**Impact on plan:** Minor text correction to match UI specification. No scope creep.

## Issues Encountered

None — TypeScript compiled clean after each task with no errors.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `app/src/components/settings/` directory established with 4 component files
- Agent/MCP/Reset sections remain inline in Settings.tsx, ready for Plan 02 extraction
- `SectionKey` type exported from SettingsSidebar for use by Plan 02 section components
- All section component patterns (onUnsavedChange, onChanged, onError callbacks) established

---
*Phase: 05-settings-decomposition*
*Completed: 2026-04-07*
