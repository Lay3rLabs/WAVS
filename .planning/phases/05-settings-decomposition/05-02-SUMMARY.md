---
phase: 05-settings-decomposition
plan: 02
subsystem: ui
tags: [react, typescript, tailwind, settings, components, decomposition]

# Dependency graph
requires: [05-01]
provides:
  - AgentSection isolated component with AgentApiKeyField sub-component and OAUTH_PROVIDERS
  - McpSection isolated component with all MCP state and 3-second polling
  - ResetSection isolated component with inline confirmation flow
  - Settings.tsx finalized as 127-line orchestrating shell
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "AgentApiKeyField moved into AgentSection — component encapsulates OAuth and API key state"
    - "McpSection owns all MCP state (11 useState, 1 useEffect polling loop)"
    - "ResetSection receives onError callback — parent owns error display"
    - "Settings.tsx: OAuth listener only parent-level concern; all section state delegated"

key-files:
  created:
    - app/src/components/settings/AgentSection.tsx
    - app/src/components/settings/McpSection.tsx
    - app/src/components/settings/ResetSection.tsx
  modified:
    - app/src/pages/Settings.tsx

key-decisions:
  - "AgentApiKeyField stays inside AgentSection (not a separate file) — it's an implementation detail of AgentSection"
  - "OAUTH_PROVIDERS const moved into AgentSection — no longer needed in Settings.tsx"
  - "McpSection initializes local auto-start/token state from props — parent no longer holds MCP state"
  - "ResetSection.onError callback propagates errors upward to Settings.tsx error display"

metrics:
  duration_minutes: ~9
  completed_date: "2026-04-07"
  tasks_completed: 2
  files_created: 3
  files_modified: 1
---

# Phase 05 Plan 02: Extract AgentSection, McpSection, ResetSection Summary

**One-liner:** Three remaining settings sections extracted to isolated components; Settings.tsx finalized at 127 lines as pure orchestrating shell with only OAuth listener and section routing.

## What Was Built

Completed the full settings decomposition started in Plan 01. Three new component files were created:

**AgentSection.tsx** (240 lines): Contains the `AgentApiKeyField` sub-component and `OAUTH_PROVIDERS` const that were previously in Settings.tsx. Receives `oauthLoading`, `oauthStatus`, and `onOAuthStart` as props — OAuth listener remains in parent. Handles provider select, model input, thinking level select, and API key/OAuth authentication management with dynamic tauri/agent imports.

**McpSection.tsx** (199 lines): Owns all 11 MCP-related state variables previously in Settings.tsx. Runs a `setInterval(poll, 3000)` polling loop via `useEffect` for MCP server status. Initializes `mcpAutoStart` and `mcpToken` from props. Contains all MCP handlers (toggle, save settings, Claude registration) and the full MCP UI including status badge, token generation, config snippet, and Claude Code registration.

**ResetSection.tsx** (55 lines): Minimal component with `showClearServicesConfirm` state. Uses `clearPersistedServices` from tauri and `usePOAStore.getState().clearRegistries()` directly. Calls `onError(msg)` for error reporting to parent.

**Settings.tsx** (127 lines): Reduced from ~616 lines. Now contains only: imports for 6 section components, 5 state variables (activeSection, hasUnsavedChanges, oauthLoading, oauthStatus, error), the OAuth listener useEffect, `handleOAuthStart`, `handleRestart`, and the layout JSX.

## Verification Results

All acceptance criteria pass:

| Check | Result |
|-------|--------|
| 7 files in app/src/components/settings/ | PASS |
| Settings.tsx under 200 lines (127) | PASS |
| `agent:oauth` listener in Settings.tsx (count=1) | PASS |
| `agent:oauth` not in AgentSection (count=0) | PASS |
| No banned state (mcpStatus, showClearServicesConfirm, etc.) in Settings.tsx | PASS |
| AgentApiKeyField inside AgentSection | PASS |
| getMcpStatus imported in McpSection | PASS |
| showClearServicesConfirm state in ResetSection | PASS |
| All 6 sections imported in Settings.tsx | PASS |
| TypeScript compilation: no errors | PASS |

## Checkpoint: Task 2 (Visual Verification)

**Type:** checkpoint:human-verify  
**Status:** Automated checks passed; visual verification deferred to orchestrator  

Automated verification confirms:
- TypeScript compiles cleanly with no errors
- All 7 section component files exist
- Settings.tsx is 127 lines (well under 200 limit)
- OAuth listener correctly in parent (not child)
- No cross-section state contamination

Visual verification (sidebar navigation, section rendering, banner positioning, OAuth flow, MCP start/stop, Reset confirmation) requires running `just app-dev` and manual inspection.

## Deviations from Plan

None — plan executed exactly as written.

## Commits

- `aee74f06`: feat(05-02): extract AgentSection, McpSection, ResetSection; finalize Settings.tsx shell

## Self-Check: PASSED

All 5 created/modified files confirmed present on disk. Commit `aee74f06` confirmed in git log.
