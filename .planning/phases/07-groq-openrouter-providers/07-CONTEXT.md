# Phase 7: Groq & OpenRouter Providers - Context

**Gathered:** 2026-04-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Add Groq and OpenRouter as selectable agent providers in the desktop app settings. Users can pick either provider from the dropdown, enter an API key, save it, and after restart the agent sidecar uses the selected provider. Both are already KnownProviders in pi-ai — this phase extends the UI and persistence, not the model registry.

</domain>

<decisions>
## Implementation Decisions

### Provider Dropdown & Defaults
- Default model for Groq: `llama-3.3-70b-versatile` (most capable general-purpose)
- Default model for OpenRouter: `anthropic/claude-sonnet-4-20250514` (familiar default via OpenRouter)
- Keep model input as free-text field for all providers — users know their model IDs
- Alphabetical dropdown order: Anthropic, Google, Groq, OpenAI, OpenRouter

### API Key UX
- Reuse existing `AgentApiKeyField` component for Groq/OpenRouter API key entry
- No OAuth support for these providers — API key only
- No format validation on API keys — runtime errors are sufficient feedback (per REQUIREMENTS.md: no connection test button)

### Agent Sidecar Integration
- Use existing `ModelRegistry.find(provider, modelId)` — both are KnownProviders, no custom registration needed
- Keep `ModelRegistry.inMemory()` for now — models.json plumbing deferred to Phase 8 (Ollama needs custom base URLs)
- Existing restart flow works: settings.json has provider/model, auth.json has key, agent reads both at startup

### Claude's Discretion
- Exact placement of new providers in component JSX
- Any minor UI text changes (placeholder text, labels)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AgentApiKeyField` component — handles API key input, show/hide toggle, save
- `AgentSection.tsx` — existing provider dropdown with Anthropic/OpenAI/Google
- `cmd_agent_set_api_key` / `cmd_agent_get_auth` — Tauri commands for auth persistence
- `cmd_save_agent_settings` — persists provider/model selection to settings.json
- `cmd_agent_set_model` — RPC to agent sidecar to switch provider/model at runtime

### Established Patterns
- Provider is stored as a string in `agent_model_provider` field
- Auth stored in `auth.json` with `{ provider: { type: "api_key", key: "..." } }` format
- Agent entrypoint reads settings at startup: `ModelRegistry.inMemory(authStorage)`
- Settings saved via Tauri invoke → state.rs → settings.json on disk

### Integration Points
- `AgentSection.tsx` line ~203-218: provider dropdown options (add Groq, OpenRouter)
- `AgentSection.tsx`: default model ID logic (switch on selected provider)
- `app/agent/entrypoint.ts` line ~49: default model resolution
- `app/src-tauri/src/commands.rs`: auth commands already provider-agnostic

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Extend existing patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
