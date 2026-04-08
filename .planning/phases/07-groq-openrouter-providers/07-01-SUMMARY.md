---
phase: 07-groq-openrouter-providers
plan: 01
subsystem: desktop-app
tags: [frontend, agent, providers, groq, openrouter, settings]
dependency_graph:
  requires: []
  provides: [groq-provider-ui, openrouter-provider-ui, settings-aware-sidecar-startup]
  affects: [app/src/components/settings/AgentSection.tsx, app/agent/entrypoint.ts]
tech_stack:
  added: []
  patterns: [DEFAULT_MODELS record for dynamic placeholders, settings.json read at sidecar startup]
key_files:
  created: []
  modified:
    - app/src/components/settings/AgentSection.tsx
    - app/agent/entrypoint.ts
decisions:
  - Used DEFAULT_MODELS record for dynamic placeholder resolution instead of switch/if-else
  - Moved fs imports to top-level to ensure availability before authDir usage
  - Used synchronous readFileSync at startup (not async) for simplicity
  - Kept ModelRegistry.inMemory() — no models.json needed for Groq/OpenRouter
requirements-completed: [PROV-01, PROV-02, PROV-03]
metrics:
  duration: ~30min
  completed: "2026-04-08"
  tasks_completed: 3
  tasks_total: 3
  files_changed: 2
---

# Phase 7 Plan 01: Groq and OpenRouter Providers Summary

**Groq and OpenRouter added as selectable agent providers with dynamic model placeholders and settings-aware sidecar startup via settings.json read at startup.**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-04-08T12:00:00Z
- **Completed:** 2026-04-08T12:30:00Z
- **Tasks:** 3 of 3 (including human-verify checkpoint)
- **Files modified:** 2

## Accomplishments
- Provider dropdown now shows 5 providers in alphabetical order: Anthropic, Google, Groq, OpenAI, OpenRouter
- Dynamic model placeholder updates to provider-appropriate default when user changes selection
- Agent sidecar reads settings.json at startup and uses saved provider/model (falls back to Anthropic)
- Human verification checkpoint approved by user confirming UI and flow work correctly

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Groq and OpenRouter to AgentSection UI** - `ca2ef451` (feat)
2. **Task 2: Read saved settings at sidecar startup** - `3ab2b956` (feat)
3. **Task 3: Verify provider selection and API key flow** - Human-verified and approved

**Plan metadata:** `48ea9ee6` (docs: complete plan summary)

## Files Created/Modified
- `app/src/components/settings/AgentSection.tsx` - Added DEFAULT_MODELS constant, Groq/OpenRouter options in dropdown, dynamic model placeholder
- `app/agent/entrypoint.ts` - Settings-aware model resolution at startup using readFileSync + modelRegistry.find()

## Decisions Made
- Used DEFAULT_MODELS record for dynamic placeholder resolution instead of switch/if-else (cleaner, extensible)
- Moved fs imports to top-level to ensure availability before authDir usage at line 49
- Used synchronous readFileSync at startup (not async) for simplicity — runs once, no blocking concern
- Kept ModelRegistry.inMemory() — no models.json needed for Groq/OpenRouter (they use API keys, not local model files)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Verification Results

- TypeScript compilation (`npx tsc --noEmit`): PASSED — no errors
- `option value="groq"` present in AgentSection.tsx: CONFIRMED
- `option value="openrouter"` present in AgentSection.tsx: CONFIRMED
- `DEFAULT_MODELS` constant present: CONFIRMED
- `DEFAULT_MODELS[settings.agent_model_provider` placeholder: CONFIRMED
- `readFileSync` imported from "node:fs": CONFIRMED
- `saved.agent_model_provider` read from settings: CONFIRMED
- `modelRegistry.find(savedProvider, savedModelId)` resolution: CONFIRMED
- `?? getModel("anthropic", "claude-sonnet-4-20250514")` fallback: CONFIRMED
- `ModelRegistry.create` NOT used (stays with inMemory): CONFIRMED
- `OAUTH_PROVIDERS` does NOT contain 'groq' or 'openrouter': CONFIRMED
- Human verification (Task 3): APPROVED by user

## User Setup Required

None - no external service configuration required. Users enter API keys directly in the app Settings UI.

## Next Phase Readiness

- Groq and OpenRouter provider UI fully integrated; ready for use
- Settings persistence (save/load provider + model + API key) is complete
- Future phases can add additional providers by extending the `DEFAULT_MODELS` record and the `<option>` list in AgentSection.tsx

## Known Stubs

None — all data wired from real settings persistence. The `DEFAULT_MODELS` constant provides real default model IDs per provider, not placeholder text.

## Threat Flags

None — no new network endpoints, auth paths, or schema changes beyond what the plan's threat model covers. T-07-04 mitigation (try/catch around JSON.parse) implemented as required.

## Self-Check: PASSED

- app/src/components/settings/AgentSection.tsx: FOUND
- app/agent/entrypoint.ts: FOUND
- commit ca2ef451: FOUND
- commit 3ab2b956: FOUND

---
*Phase: 07-groq-openrouter-providers*
*Completed: 2026-04-08*
