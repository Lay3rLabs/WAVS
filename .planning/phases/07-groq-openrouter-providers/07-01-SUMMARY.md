---
phase: 07-groq-openrouter-providers
plan: 01
subsystem: desktop-app
status: checkpoint-pending-human-verify
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
metrics:
  duration: ~25min
  completed: "2026-04-08T12:24:00Z"
  tasks_completed: 2
  tasks_total: 3
  files_changed: 2
---

# Phase 7 Plan 01: Groq and OpenRouter Providers Summary

**One-liner:** Groq and OpenRouter added as selectable agent providers with dynamic model placeholders and settings-aware sidecar startup via settings.json read at startup.

## What Was Built

Two file changes implementing Groq/OpenRouter provider support in the WAVS desktop app:

1. **AgentSection.tsx** — Added `DEFAULT_MODELS` constant mapping each provider to its recommended default model ID. Added `groq` and `openrouter` options to the provider dropdown (now 5 options in alphabetical order: Anthropic, Google, Groq, OpenAI, OpenRouter). Replaced static model placeholder with dynamic lookup via `DEFAULT_MODELS[settings.agent_model_provider ?? 'anthropic']`.

2. **entrypoint.ts** — Moved `mkdirSync, existsSync, readFileSync` imports to top-level. Added settings-aware model resolution: reads `settings.json` from `authDir` at startup, extracts `agent_model_provider` and `agent_model_id`, resolves via `modelRegistry.find()`, falls back to Anthropic claude-sonnet-4 if settings missing, file unreadable, or model not found in registry.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| Task 1: AgentSection UI | ca2ef451 | feat(07-01): add Groq and OpenRouter to agent provider dropdown |
| Task 2: Sidecar startup | 3ab2b956 | feat(07-01): read saved provider/model from settings.json at sidecar startup |

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

## Deviations from Plan

None — plan executed exactly as written.

## Checkpoint Pending

**Task 3 (checkpoint:human-verify)** requires human verification of the running app:
- Launch `just app-dev`
- Confirm 5 providers in alphabetical order in dropdown
- Confirm model placeholder updates dynamically when provider changes
- Confirm API key save/remove works for Groq and OpenRouter

## Known Stubs

None — all data wired from real settings persistence. The `DEFAULT_MODELS` constant provides real default model IDs per provider, not placeholder text.

## Threat Flags

None — no new network endpoints, auth paths, or schema changes beyond what the plan's threat model covers. T-07-04 mitigation (try/catch around JSON.parse) implemented as required.

## Self-Check: PASSED

- app/src/components/settings/AgentSection.tsx: FOUND
- app/agent/entrypoint.ts: FOUND
- commit ca2ef451: FOUND
- commit 3ab2b956: FOUND
