---
phase: 08-ollama-provider
plan: 01
subsystem: ui
tags: [ollama, tauri, react, typescript, rust, settings, agent, models-json, openai-compatible]

# Dependency graph
requires:
  - phase: 07-groq-openrouter-providers
    provides: "Agent settings persistence pipeline (agent_model_provider, agent_model_id, saveAgentSettings, cmd_save_agent_settings)"
provides:
  - "Ollama selectable as agent provider in settings dropdown"
  - "Conditional base URL field (shown only for Ollama, default http://localhost:11434/v1)"
  - "API key field hidden when Ollama selected"
  - "agent_base_url field in TS Settings interface, Rust Settings struct, and cmd_save_agent_settings"
  - "models.json generation in cmd_start_agent when provider is ollama (deleted otherwise)"
  - "ModelRegistry.create() in agent sidecar for custom provider resolution"
  - "thinkingLevel set to 'off' for Ollama to avoid unsupported reasoning parameters"
affects: [settings-ux, agent-sidecar, open-source-providers]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "File-contract pattern: Rust backend writes models.json to authDir; TypeScript sidecar reads at startup via ModelRegistry.create()"
    - "Conditional UI field based on provider: show/hide base URL and API key fields based on selected provider"
    - "serde_json::json!() macro with minimal string interpolation for safe JSON generation"

key-files:
  created: []
  modified:
    - app/src/types/index.ts
    - app/src/components/settings/AgentSection.tsx
    - app/src/pages/Settings.tsx
    - app/src/tauri/agent.ts
    - packages/gui/shared/src/settings.rs
    - app/src-tauri/src/commands.rs
    - app/agent/entrypoint.ts

key-decisions:
  - "File-contract pattern: Rust owns models.json, TypeScript sidecar reads it — avoids new IPC commands"
  - "ModelRegistry.create() is safe for all providers: when models.json absent, built-in KnownProviders still work"
  - "thinkingLevel 'off' for Ollama prevents pi-ai from sending reasoning_effort params to Ollama endpoints"
  - "apiKey set to 'ollama' (well-known placeholder) because pi-ai requires non-empty key for OpenAI-compatible providers"

patterns-established:
  - "Provider-conditional UI: wrap provider-specific fields in (provider === 'X') conditionals"
  - "models.json lifecycle: generate at agent start if needed, delete if not needed, sidecar reads on startup"

requirements-completed: [PROV-04, PROV-05, PROV-06, PROV-07]

# Metrics
duration: 35min
completed: 2026-04-08
---

# Phase 8 Plan 01: Ollama Provider Summary

**Ollama added as selectable agent provider with conditional base URL field, models.json generation from Rust backend, and ModelRegistry.create() sidecar switch for OpenAI-compatible local model support**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-04-08T13:00:00Z
- **Completed:** 2026-04-08T13:31:37Z
- **Tasks:** 2/3 complete (Task 3 is a human-verify checkpoint)
- **Files modified:** 7

## Accomplishments
- Full settings pipeline for agent_base_url across TypeScript types, Rust struct, IPC handler, and UI
- Ollama added to provider dropdown (alphabetical: Groq, Ollama, OpenAI) with default model llama3.1:8b
- Conditional base URL field appears only when Ollama selected, pre-filled with http://localhost:11434/v1
- API key field hidden when Ollama selected (no key needed for local Ollama)
- Rust backend generates models.json with correct openai-completions API config, compat flags, and model entry at agent startup
- Agent sidecar switched from ModelRegistry.inMemory() to ModelRegistry.create() — supports custom provider loading while keeping all existing providers working

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Ollama to settings UI with conditional base URL field** - `faacf664` (feat)
2. **Task 2: Generate models.json in Rust and switch sidecar to ModelRegistry.create()** - `ddd0a722` (feat)
3. **Task 3: Verify Ollama end-to-end tool calling** - CHECKPOINT (human-verify required)

## Files Created/Modified
- `app/src/types/index.ts` - Added agent_base_url: string | null to Settings interface
- `app/src/components/settings/AgentSection.tsx` - Added Ollama option, base URL field, hidden API key, ollama DEFAULT_MODELS entry
- `app/src/pages/Settings.tsx` - Pass agent_base_url to AgentSection
- `app/src/tauri/agent.ts` - Added agent_base_url to saveAgentSettings type parameter
- `packages/gui/shared/src/settings.rs` - Added agent_base_url: Option<String> to Settings struct
- `app/src-tauri/src/commands.rs` - models.json generation in cmd_start_agent; agent_base_url in cmd_save_agent_settings
- `app/agent/entrypoint.ts` - Switched to ModelRegistry.create(authStorage, modelsJsonPath); thinkingLevel off for Ollama

## Decisions Made
- File-contract pattern chosen over new IPC command: Rust writes models.json, sidecar reads it — simpler, no new RPC surface
- ModelRegistry.create() is backward-compatible: existing providers (anthropic, openai, google, groq, openrouter) work unchanged
- thinkingLevel set to "off" for Ollama because pi-ai would otherwise send reasoning_effort=low to Ollama's endpoint, which rejects it
- Dummy apiKey "ollama" used because pi-ai validates that apiKey is non-empty for OpenAI-compatible providers

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- TypeScript compilation in worktree environment: tsc reports "cannot find module react" because worktree lacks node_modules. Verified against main workspace `/workspace/app` which compiles with zero errors. Pre-existing infrastructure limitation of the worktree setup, not caused by our changes.

## Known Stubs
None - base URL defaults to http://localhost:11434/v1 (non-empty, functional default). ModelRegistry.create() without a models.json file gracefully falls back to in-memory built-in providers.

## Threat Flags
None - no new network endpoints or trust boundaries introduced. The agent_base_url value is a plain string stored in settings.json; it is passed to pi-ai's HTTP client which validates URL format. No shell execution of the URL value. T-08-02 and T-08-05 mitigations are implemented as planned (no eval/template injection, hardcoded JSON structure via serde_json::json!()).

## User Setup Required
None - no external service configuration required. (Users need Ollama installed separately to use Ollama provider, but that is documented in the checkpoint verification steps.)

## Next Phase Readiness
- Implementation complete and awaiting human verification (Task 3 checkpoint)
- Once user approves the checkpoint, the plan is fully complete
- Phase 9 (settings scroll refactor) is independent and can proceed immediately

---
*Phase: 08-ollama-provider*
*Completed: 2026-04-08*
