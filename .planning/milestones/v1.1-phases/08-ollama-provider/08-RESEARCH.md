# Phase 8: Ollama Provider - Research

**Researched:** 2026-04-08
**Domain:** Ollama integration via pi-ai ModelRegistry + Tauri settings pipeline
**Confidence:** HIGH

## Summary

Phase 8 adds Ollama as a selectable agent provider. The core insight is that Ollama is NOT a `KnownProvider` in the `@mariozechner/pi-ai` library — it must be registered via a `models.json` file consumed by `ModelRegistry.create()`. The current entrypoint.ts uses `ModelRegistry.inMemory()` which ignores models.json entirely; switching to `ModelRegistry.create(authStorage, modelsJsonPath)` is the primary sidecar change.

The data flow is: user picks "ollama" in Settings UI → Tauri saves `agent_model_provider`, `agent_model_id`, `agent_base_url` to settings.json → on agent startup (or save), Rust backend writes `models.json` to authDir → entrypoint.ts reads models.json via `ModelRegistry.create()` → pi-ai finds the ollama model and routes requests to `http://localhost:11434/v1`.

Four integration surfaces must change in parallel: (1) TypeScript Settings interface (`index.ts`), (2) Rust Settings struct (`settings.rs`), (3) Rust command handler (`commands.rs`), (4) React UI (`AgentSection.tsx`), and (5) agent sidecar (`entrypoint.ts`).

**Primary recommendation:** Write models.json from the Rust backend at agent startup time (inside `cmd_start_agent`) using the current settings, rather than on every settings save. This avoids writing an Ollama-specific file when the user hasn't selected Ollama, and keeps a single code path responsible for the file.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Write a `models.json` file from settings at startup — declarative, auto-reloads on `/model` calls, no extension code needed
- Switch from `ModelRegistry.inMemory()` to `ModelRegistry.create(authStorage, modelsJsonPath)` in entrypoint.ts
- Store models.json in same `authDir` as auth.json and settings.json — `path.join(authDir, "models.json")`
- Default Ollama model ID: `llama3.1:8b`
- Add `agent_base_url: string | null` field to Settings interface in `app/src/types/index.ts`
- Default base URL: `http://localhost:11434/v1` pre-filled when Ollama selected
- Show base URL field conditionally — only when provider === "ollama"
- Persist base URL across provider switches (save it, hide when not Ollama, show again if user switches back)
- Use `openai-completions` API mode in models.json — Ollama's `/v1/chat/completions` is OpenAI-compatible
- Set `apiKey: "ollama"` as dummy value — pi-ai requires a non-empty key, Ollama doesn't need auth
- Human verification checkpoint for tool calling — requires running Ollama locally

### Claude's Discretion
- Exact Rust state.rs struct updates for agent_base_url field
- models.json generation logic details (write on settings save vs write on sidecar startup)
- Whether to add Ollama to the alphabetical dropdown position or at end

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PROV-04 | User can select Ollama as an agent provider from the settings dropdown | Add "ollama" option to AgentSection.tsx provider `<select>`; add to DEFAULT_MODELS map |
| PROV-05 | User can configure a base URL for Ollama (defaults to localhost:11434) | Conditional `agent_base_url` field in UI; persisted via existing settings pipeline |
| PROV-06 | Agent sidecar loads custom provider config from models.json at startup | Switch to `ModelRegistry.create()`; generate models.json in Rust before spawning sidecar |
| PROV-07 | User can use the agent with Ollama-hosted open-source models for WAVS tasks | Requires tool calling via openai-completions API; human verification needed |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@mariozechner/pi-coding-agent` | (already installed) | ModelRegistry.create() API | Ships with the agent sidecar — no new install |
| `@mariozechner/pi-ai` | (already installed) | KnownProvider type, openai-completions API | Core inference library |

No new npm packages needed. [VERIFIED: local node_modules inspection]

**models.json format** (verified against `loadCustomModels()` source): [VERIFIED: model-registry.js inspection]

```json
{
  "providers": {
    "ollama": {
      "baseUrl": "http://localhost:11434/v1",
      "api": "openai-completions",
      "apiKey": "ollama",
      "compat": {
        "supportsDeveloperRole": false,
        "supportsReasoningEffort": false
      },
      "models": [
        { "id": "llama3.1:8b" }
      ]
    }
  }
}
```

**Validation rules from `validateConfig()` source:** [VERIFIED: model-registry.js line 316-329]
- When `models` array is non-empty: `baseUrl` is REQUIRED (throws if missing)
- When `models` array is non-empty: `apiKey` is REQUIRED (throws if missing)
- When `models` is empty or absent: must have at least `baseUrl`, `compat`, or `modelOverrides`
- Each model must have an `api` at either model or provider level

## Architecture Patterns

### Data Flow: Settings → models.json → Sidecar

```
Settings.tsx (UI)
  → saveAgentSettings({ agent_model_provider, agent_model_id, agent_base_url })
  → cmd_save_agent_settings (commands.rs) patches Settings struct
  → settings.json written to authDir

cmd_start_agent (commands.rs)
  → reads current Settings
  → if agent_model_provider == "ollama": write models.json to authDir
  → if provider != "ollama": delete or skip models.json (sidecar ignores missing file)
  → spawn sidecar with WAVS_AUTH_DIR

entrypoint.ts
  → ModelRegistry.create(authStorage, path.join(authDir, "models.json"))
  → reads settings.json → savedProvider = "ollama", savedModelId = "llama3.1:8b"
  → modelRegistry.find("ollama", "llama3.1:8b") → finds custom model from models.json
  → defaultModel is set to the Ollama model
```

### Recommended File Structure (changes)

```
app/src/types/index.ts         — add agent_base_url: string | null to Settings
app/src/tauri/agent.ts         — add agent_base_url to saveAgentSettings() type
app/src/pages/Settings.tsx     — pass agent_base_url down to AgentSection
app/src/components/settings/
  AgentSection.tsx             — add Ollama option; conditional base URL field; hide API key
packages/gui/shared/src/
  settings.rs                  — add agent_base_url: Option<String>
app/src-tauri/src/
  commands.rs                  — handle agent_base_url in cmd_save_agent_settings;
                                  generate models.json in cmd_start_agent
app/agent/
  entrypoint.ts                — ModelRegistry.create() instead of inMemory()
```

### Pattern 1: ModelRegistry.create() vs inMemory()

**What:** `ModelRegistry.create(authStorage, modelsJsonPath)` reads models.json from disk at construction time. `ModelRegistry.inMemory(authStorage)` skips the file entirely.

**Current code (entrypoint.ts line 47):**
```typescript
// Source: verified in /workspace/app/agent/entrypoint.ts
const modelRegistry = ModelRegistry.inMemory(authStorage);
```

**Target code:**
```typescript
// Source: model-registry.d.ts line 29
const modelsJsonPath = path.join(authDir, "models.json");
const modelRegistry = ModelRegistry.create(authStorage, modelsJsonPath);
```

`ModelRegistry.create()` defaults `modelsJsonPath` to `join(getAgentDir(), "models.json")` if no path is given — but passing the explicit authDir path is correct for our setup. [VERIFIED: model-registry.js line 179]

### Pattern 2: models.json Generation in Rust

Write models.json from Rust `cmd_start_agent` before spawning the sidecar. This is the cleanest timing: always fresh, no stale file risk.

```rust
// In commands.rs, inside cmd_start_agent, after reading settings:
if s.agent_model_provider.as_deref() == Some("ollama") {
    let base_url = s.agent_base_url
        .as_deref()
        .unwrap_or("http://localhost:11434/v1");
    let model_id = s.agent_model_id
        .as_deref()
        .unwrap_or("llama3.1:8b");
    let models_json = generate_ollama_models_json(base_url, model_id);
    let models_path = auth_dir_path.join("models.json");
    tokio::fs::write(&models_path, models_json).await
        .map_err(|e| AppError::Agent(format!("Failed to write models.json: {}", e)))?;
}
```

Where `generate_ollama_models_json` returns the JSON string with `serde_json::json!()`.

### Pattern 3: Conditional Base URL Field in React

```typescript
// Source: AgentSection.tsx pattern (verified existing code)
{(settings.agent_model_provider ?? 'anthropic') === 'ollama' && (
  <div className="flex flex-col gap-1">
    <label className="text-tan-muted text-xs">Base URL</label>
    <input
      type="text"
      placeholder="http://localhost:11434/v1"
      value={settings.agent_base_url ?? ''}
      onChange={async (e) => {
        const { saveAgentSettings } = await import('../../tauri/agent');
        await saveAgentSettings({ agent_base_url: e.target.value || null });
      }}
      className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
    />
  </div>
)}
```

### Pattern 4: Hiding API Key Field for Ollama

`AgentApiKeyField` must not render when provider is "ollama" (Ollama has no API key). The simplest approach: guard it at the call site in `AgentSection`:

```typescript
{(settings.agent_model_provider ?? 'anthropic') !== 'ollama' && (
  <AgentApiKeyField ... />
)}
```

### Anti-Patterns to Avoid

- **Using ModelRegistry.inMemory() after this phase:** It ignores models.json entirely. The switch to `.create()` is unconditional — it's safe for all providers because models.json is only written when provider is Ollama.
- **Writing models.json on every settings save:** This creates a stale models.json when the user switches away from Ollama. Writing only at startup (when the provider is currently Ollama) avoids this issue.
- **Registering Ollama via `registerProvider()` instead of models.json:** Works but bypasses the file-contract pattern established in design decisions and doesn't survive sidecar restarts cleanly.
- **Omitting `compat.supportsDeveloperRole: false`:** Ollama's `/v1/chat/completions` does not support the OpenAI "developer" system role variant. Without this compat flag, pi-ai may send unsupported parameters. [VERIFIED: model-registry.js OpenAICompletionsCompatSchema]
- **Empty or null apiKey in models.json:** `validateConfig()` throws if `apiKey` is absent when models are defined. Must use `"ollama"` (non-empty dummy string). [VERIFIED: model-registry.js line 321-323]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Custom provider config | Custom registry extension code | models.json + ModelRegistry.create() | Already validated and schema-checked by pi-ai |
| JSON schema validation for models.json | Manual validation | pi-ai's AJV schema (built-in) | Errors surfaced via ModelRegistry.getError() |
| API key storage | Custom storage | Existing AuthStorage + `apiKey: "ollama"` dummy | AuthStorage is already wired for all providers |

**Key insight:** The "dummy API key" pattern (`apiKey: "ollama"`) is the idiomatic approach for local models that don't need auth — it satisfies pi-ai's non-empty key requirement without adding new auth infrastructure.

## Common Pitfalls

### Pitfall 1: `find()` Returns undefined for Ollama Model
**What goes wrong:** `modelRegistry.find("ollama", "llama3.1:8b")` returns `undefined`, so `defaultModel` falls back to Anthropic even when Ollama is selected.
**Why it happens:** Either (a) models.json was not written before sidecar spawn, (b) `ModelRegistry.inMemory()` is still used, or (c) the models.json has a schema validation error (missing `apiKey`, missing `api` field).
**How to avoid:** Switch to `ModelRegistry.create()` AND write models.json before spawning. Call `modelRegistry.getError()` at startup and log it to stderr for debugging.
**Warning signs:** Agent responds with Anthropic model despite Ollama being selected; `getError()` returns a non-undefined string.

### Pitfall 2: `agent_base_url` Not Propagated to Settings.tsx
**What goes wrong:** Settings.tsx passes a fixed props shape to AgentSection — adding `agent_base_url` to the TypeScript `Settings` interface doesn't automatically include it in the props passed down.
**Why it happens:** Settings.tsx line 101-106 constructs an explicit object literal for the `settings` prop. It must be updated to include `agent_base_url`.
**How to avoid:** Update the AgentSectionProps interface AND the call site in Settings.tsx simultaneously. [VERIFIED: Settings.tsx line 101-106]

### Pitfall 3: Rust Serde Deserialization Fails for Old settings.json
**What goes wrong:** Users who have an existing `settings.json` without `agent_base_url` get a deserialization error on upgrade.
**Why it happens:** Missing `#[serde(default)]` on the new field.
**How to avoid:** Always annotate new optional fields with `#[serde(default)]` in the Rust Settings struct. [VERIFIED: settings.rs pattern — all existing optional fields use `#[serde(default)]`]

### Pitfall 4: Thinking Level Sent to Ollama
**What goes wrong:** Pi-ai sends `reasoning_effort` or thinking-related parameters to Ollama's endpoint, causing a 400 error.
**Why it happens:** The `agent_thinking_level` setting is passed to `createAgentSessionFromServices` as `thinkingLevel`, and pi-ai may translate this to `reasoning_effort` for openai-completions providers.
**How to avoid:** Set `supportsReasoningEffort: false` in the models.json compat block. This tells pi-ai not to send reasoning parameters. Also consider not passing `thinkingLevel` at all when provider is ollama (or hardcode "off"). [VERIFIED: OpenAICompletionsCompatSchema in model-registry.js]

### Pitfall 5: Stale defaultModel After Provider Switch
**What goes wrong:** User switches from Ollama → Anthropic in settings, restarts the agent, but `modelRegistry.find("ollama", "llama3.1:8b")` still returns the model because models.json wasn't cleaned up.
**Why it happens:** models.json was written at startup and never deleted when provider changed.
**How to avoid:** Only write models.json when provider === "ollama". When provider is anything else, either skip writing or delete any existing models.json. Since `ModelRegistry.create()` reads on construction and the settings.json savedProvider drives `find()`, the wrong provider won't be selected — but the file should still be cleaned up for clarity.

## Code Examples

### models.json generation in Rust
```rust
// Source: derived from pi-ai model-registry.js validateConfig() requirements
fn generate_ollama_models_json(base_url: &str, model_id: &str) -> String {
    serde_json::json!({
        "providers": {
            "ollama": {
                "baseUrl": base_url,
                "api": "openai-completions",
                "apiKey": "ollama",
                "compat": {
                    "supportsDeveloperRole": false,
                    "supportsReasoningEffort": false
                },
                "models": [
                    { "id": model_id }
                ]
            }
        }
    })
    .to_string()
}
```

### entrypoint.ts switch to ModelRegistry.create()
```typescript
// Source: model-registry.d.ts line 29 (verified)
// Replace line 47 in entrypoint.ts:
// const modelRegistry = ModelRegistry.inMemory(authStorage);
const modelsJsonPath = path.join(authDir, "models.json");
const modelRegistry = ModelRegistry.create(authStorage, modelsJsonPath);

// After creation, log any load error:
const registryError = modelRegistry.getError();
if (registryError) {
  console.error("[ModelRegistry] Error loading models.json:", registryError);
}
```

### Rust Settings struct addition
```rust
// Source: packages/gui/shared/src/settings.rs — follow existing pattern
#[serde(default)]
pub agent_base_url: Option<String>,
```

### cmd_save_agent_settings addition
```rust
// Source: commands.rs line 1582+ — follow existing pattern
if let Some(v) = updates.get("agent_base_url") {
    s.agent_base_url = v.as_str().map(String::from);
}
```

### AgentSection dropdown with Ollama
```typescript
// Source: AgentSection.tsx — alphabetical position places Ollama between OpenAI and OpenRouter
<option value="anthropic">Anthropic</option>
<option value="google">Google</option>
<option value="groq">Groq</option>
<option value="ollama">Ollama</option>
<option value="openai">OpenAI</option>
<option value="openrouter">OpenRouter</option>
```

### DEFAULT_MODELS addition for Ollama
```typescript
// Source: AgentSection.tsx DEFAULT_MODELS map — add Ollama entry
const DEFAULT_MODELS: Record<string, string> = {
  anthropic: 'claude-sonnet-4-20250514',
  google: 'gemini-2.0-flash',
  groq: 'llama-3.3-70b-versatile',
  ollama: 'llama3.1:8b',           // add this
  openai: 'gpt-4o',
  openrouter: 'anthropic/claude-sonnet-4-20250514',
};
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `ModelRegistry.inMemory()` | `ModelRegistry.create(authStorage, path)` | Phase 8 | Enables models.json for custom/local providers |
| No Ollama support | Ollama via openai-completions + models.json | Phase 8 | Enables local open-source model use |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Writing models.json only at agent startup (not on every settings save) is the better approach | Architecture Patterns | If wrong: user edits base URL, saves, but the running sidecar still uses old models.json. Mitigation: sidecar restart is already required to pick up provider changes since defaultModel is set at startup. |
| A2 | Ollama alphabetical position in dropdown: between OpenAI and OpenRouter (O-l before O-p, after O-p is before O-p-e-n-R) — actually Ollama (O-l) comes before OpenAI (O-p) and OpenRouter (O-p) | Code Examples | If wrong: minor UX ordering issue only |
| A3 | The `compat` block in models.json is sufficient to prevent reasoning_effort from being sent | Common Pitfalls | If wrong: Ollama calls fail with 400; fix by also hardcoding thinkingLevel to "off" in entrypoint |

**Note on A2:** Alphabetical: G-r-o-q < O-l-l-a-m-a < O-p-e-n-A-I < O-p-e-n-R-o-u-t-e-r. So Ollama goes between Groq and OpenAI.

## Open Questions

1. **models.json cleanup when switching away from Ollama**
   - What we know: models.json is only read at ModelRegistry construction time (sidecar startup)
   - What's unclear: Should we delete models.json when provider != "ollama" at startup? Or leave it?
   - Recommendation: Delete (or skip writing) when provider is not Ollama. Stale files are confusing. A simple `tokio::fs::remove_file` with ignored error is sufficient.

2. **Thinking level for Ollama**
   - What we know: `supportsReasoningEffort: false` in compat prevents reasoning_effort parameter
   - What's unclear: Does `thinkingLevel: "low"` in `createAgentSessionFromServices` still affect request format in other ways?
   - Recommendation: Planner should include a task to test with thinking level set to "off" for Ollama, or explicitly set thinkingLevel to "off" when provider is ollama in entrypoint.ts.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Ollama (local) | PROV-07 tool calling test | Unknown | — | Human UAT gate — not needed for code tasks |
| Node.js + npx tsx | Agent sidecar | Already required | — | — |

**Missing dependencies with no fallback:** None that block code implementation. Ollama itself is only needed for the human UAT verification task (PROV-07).

## Sources

### Primary (HIGH confidence)
- `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/model-registry.js` — ModelRegistry.create(), validateConfig(), parseModels() implementation verified
- `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/model-registry.d.ts` — Public API surface
- `/workspace/app/agent/node_modules/@mariozechner/pi-ai/dist/types.d.ts` — KnownProvider type (ollama confirmed absent)
- `/workspace/app/agent/entrypoint.ts` — Current ModelRegistry.inMemory() usage
- `/workspace/packages/gui/shared/src/settings.rs` — Settings struct patterns (serde(default))
- `/workspace/app/src-tauri/src/commands.rs` — cmd_save_agent_settings pattern
- `/workspace/app/src/components/settings/AgentSection.tsx` — Existing provider dropdown and DEFAULT_MODELS

### Secondary (MEDIUM confidence)
- `/workspace/app/src-tauri/src/agent.rs` — PiSidecarConfig and spawn sequence
- `/workspace/app/src/tauri/agent.ts` — saveAgentSettings TypeScript bindings

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified against installed node_modules source
- Architecture: HIGH — all integration points verified in codebase
- Pitfalls: HIGH — derived from validateConfig() source and existing patterns
- models.json format: HIGH — verified against schema and validation code

**Research date:** 2026-04-08
**Valid until:** 2026-05-08 (stable library — pi-coding-agent unlikely to change schema)
