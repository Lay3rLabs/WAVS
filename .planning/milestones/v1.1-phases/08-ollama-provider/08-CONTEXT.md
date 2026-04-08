# Phase 8: Ollama Provider - Context

**Gathered:** 2026-04-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Add Ollama as a selectable agent provider with a custom base URL field. When Ollama is selected, the app writes a models.json file that the agent sidecar reads via ModelRegistry.create(). The sidecar must work end-to-end with locally-hosted open-source models including tool-calling tasks. Requires switching from ModelRegistry.inMemory() to ModelRegistry.create().

</domain>

<decisions>
## Implementation Decisions

### Ollama Provider Registration
- Write a `models.json` file from settings at startup — declarative, auto-reloads on `/model` calls, no extension code needed
- Switch from `ModelRegistry.inMemory()` to `ModelRegistry.create(authStorage, modelsJsonPath)` in entrypoint.ts
- Store models.json in same `authDir` as auth.json and settings.json — `path.join(authDir, "models.json")`
- Default Ollama model ID: `llama3.1:8b`

### Base URL UX
- Add `agent_base_url: string | null` field to Settings interface in `app/src/types/index.ts`
- Default base URL: `http://localhost:11434/v1` pre-filled when Ollama selected
- Show base URL field conditionally — only when provider === "ollama"
- Persist base URL across provider switches (save it, hide when not Ollama, show again if user switches back)

### Tool Calling & API Compatibility
- Use `openai-completions` API mode in models.json — Ollama's `/v1/chat/completions` is OpenAI-compatible
- Set `apiKey: "ollama"` as dummy value — pi-ai requires a non-empty key, Ollama doesn't need auth
- Human verification checkpoint for tool calling — requires running Ollama locally

### Claude's Discretion
- Exact Rust state.rs struct updates for agent_base_url field
- models.json generation logic details (write on settings save vs write on sidecar startup)
- Whether to add Ollama to the alphabetical dropdown position or at end

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Phase 7 pattern: dropdown + DEFAULT_MODELS map in `AgentSection.tsx`
- Phase 7 pattern: settings.json reading in `entrypoint.ts` (already reads agent_model_provider, agent_model_id)
- `cmd_save_agent_settings` — already handles arbitrary JSON fields
- `AgentApiKeyField` — NOT needed for Ollama (no API key), but should not break when Ollama selected
- `ModelRegistry.create(authStorage, modelsJsonPath?)` — supports optional models.json path

### Established Patterns
- Settings saved via Tauri invoke → state.rs → settings.json on disk
- Agent sidecar reads settings.json at startup (Phase 7 addition)
- auth.json, settings.json both in authDir (Tauri app config dir)
- KnownProvider type allows arbitrary strings via `(string & {})` — "ollama" works as Provider type

### Integration Points
- `app/src/types/index.ts` — add `agent_base_url` to Settings interface
- `app/src/components/settings/AgentSection.tsx` — add Ollama to dropdown, conditional base URL field
- `app/agent/entrypoint.ts` — switch to ModelRegistry.create(), generate models.json from settings
- `app/src-tauri/src/state.rs` — add agent_base_url to Rust Settings struct
- `app/src-tauri/src/commands.rs` — ensure cmd_save_agent_settings handles agent_base_url

### models.json Format (from pi-ai docs)
```json
{
  "providers": {
    "ollama": {
      "baseUrl": "http://localhost:11434/v1",
      "api": "openai-completions",
      "apiKey": "ollama",
      "models": [
        { "id": "llama3.1:8b" }
      ]
    }
  }
}
```

</code_context>

<specifics>
## Specific Ideas

- "ollama" is NOT a KnownProvider in pi-ai — must use models.json or registerProvider
- models.json reloads on `/model` calls — no restart needed for model changes after initial setup
- Ollama compat settings needed: `supportsDeveloperRole: false`, `supportsReasoningEffort: false`
- The base URL field should only appear when Ollama is selected (conditional rendering)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
