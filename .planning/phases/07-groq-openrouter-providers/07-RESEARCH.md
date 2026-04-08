# Phase 7: Groq & OpenRouter Providers - Research

**Researched:** 2026-04-08
**Domain:** React/TypeScript desktop app settings UI (Tauri + React 19)
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Default model for Groq: `llama-3.3-70b-versatile`
- Default model for OpenRouter: `anthropic/claude-sonnet-4-20250514`
- Keep model input as free-text field for all providers — users know their model IDs
- Alphabetical dropdown order: Anthropic, Google, Groq, OpenAI, OpenRouter
- Reuse existing `AgentApiKeyField` component for Groq/OpenRouter API key entry
- No OAuth support for these providers — API key only
- No format validation on API keys — runtime errors are sufficient feedback
- Use existing `ModelRegistry.find(provider, modelId)` — both are KnownProviders, no custom registration needed
- Keep `ModelRegistry.inMemory()` for now — models.json plumbing deferred to Phase 8
- Existing restart flow works: settings.json has provider/model, auth.json has key, agent reads both at startup

### Claude's Discretion
- Exact placement of new providers in component JSX
- Any minor UI text changes (placeholder text, labels)

### Deferred Ideas (OUT OF SCOPE)
- None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PROV-01 | User can select Groq as an agent provider from the settings dropdown | `AgentSection.tsx` provider `<select>` — add `<option value="groq">Groq</option>` |
| PROV-02 | User can select OpenRouter as an agent provider from the settings dropdown | Same — add `<option value="openrouter">OpenRouter</option>` |
| PROV-03 | User can configure API keys for Groq and OpenRouter providers | `AgentApiKeyField` already handles any string provider; no new UI component needed |
</phase_requirements>

---

## Summary

Phase 7 is a focused UI extension with no new backend infrastructure required. Both `groq` and `openrouter` are already declared as `KnownProvider` values in `@mariozechner/pi-ai` and have their base URLs hardcoded in `models.generated.js` (`https://api.groq.com/openai/v1` and `https://openrouter.ai/api/v1` respectively). The auth commands (`cmd_agent_set_api_key`, `cmd_agent_get_auth`, `cmd_agent_remove_auth`) are fully provider-agnostic — they write to `auth.json` under a string key and make no assumptions about which providers exist.

The two files that need changes are `AgentSection.tsx` (add two `<option>` elements to the provider dropdown and update the default model ID placeholder) and `entrypoint.ts` (read `settings.json` at startup to apply the saved provider/model instead of the hard-coded Anthropic default). The rest of the pipeline — `cmd_save_agent_settings`, `cmd_agent_set_model`, `AuthStorage`, `ModelRegistry.inMemory()` — already works for any provider string.

**Primary recommendation:** Two targeted edits — `AgentSection.tsx` (provider options + default model logic) and `entrypoint.ts` (read settings at startup) — deliver all three requirements with no new Rust, no new API surface, and no new npm packages.

---

## Project Constraints (from CLAUDE.md)

| Directive | Applies to This Phase |
|-----------|----------------------|
| Desktop app uses Tauri 2 + React 19 + Vite 7 | Yes — all UI changes in `app/src/` |
| State management: Zustand; blockchain: Viem | Zustand store already handles settings |
| Build: `just app-dev` / `just app-build-release` | Use to verify changes |
| `just lint` / `just lint-fix` for Rust | No Rust changes in this phase |

---

## Standard Stack

### Core (already installed — no new packages)

| Library | Installed Version | Purpose | Note |
|---------|-------------------|---------|------|
| `@mariozechner/pi-ai` | in `app/agent/node_modules` | Provider model registry, `KnownProvider` type | `groq` and `openrouter` already declared [VERIFIED: types.d.ts line 5] |
| `@mariozechner/pi-coding-agent` | in `app/agent/node_modules` | `ModelRegistry.inMemory()`, `AuthStorage` | Provider-agnostic [VERIFIED: model-registry.d.ts] |
| React 19 + Vite 7 | `app/` | Frontend framework | No changes needed |
| Tauri 2 | `app/src-tauri/` | IPC bridge | All required commands already registered [VERIFIED: lib.rs] |

**Installation:** None. This phase requires zero new dependencies.

---

## Architecture Patterns

### Existing Pattern: Provider Dropdown in AgentSection.tsx

The provider `<select>` is at lines 203–218 of `app/src/components/settings/AgentSection.tsx`. It calls `saveAgentSettings({ agent_model_provider: e.target.value })` on change, which invokes `cmd_save_agent_settings` → writes `agent_model_provider` to `settings.json`. [VERIFIED: commands.rs:1589]

```tsx
// Source: app/src/components/settings/AgentSection.tsx (lines 203-218)
<select
  value={settings.agent_model_provider ?? 'anthropic'}
  onChange={async (e) => {
    try {
      const { saveAgentSettings } = await import('../../tauri/agent');
      await saveAgentSettings({ agent_model_provider: e.target.value });
    } catch (err) {
      console.error('Failed to save agent provider:', err);
    }
  }}
  className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm text-sm outline-none"
>
  <option value="anthropic">Anthropic</option>
  <option value="openai">OpenAI</option>
  <option value="google">Google</option>
</select>
```

**What to change:** Add two options in alphabetical position:
```tsx
<option value="anthropic">Anthropic</option>
<option value="google">Google</option>
<option value="groq">Groq</option>
<option value="openai">OpenAI</option>
<option value="openrouter">OpenRouter</option>
```

### Existing Pattern: Default Model Placeholder

The model `<input>` at line 224 has a static placeholder `"claude-sonnet-4-20250514"`. This should update dynamically based on selected provider so the placeholder shows the correct default.

```tsx
// Current (line 224 — static placeholder)
<input placeholder="claude-sonnet-4-20250514" ... />

// Updated — dynamic placeholder per provider
const DEFAULT_MODELS: Record<string, string> = {
  anthropic: 'claude-sonnet-4-20250514',
  google: 'gemini-2.0-flash',
  groq: 'llama-3.3-70b-versatile',
  openai: 'gpt-4o',
  openrouter: 'anthropic/claude-sonnet-4-20250514',
};
const currentProvider = settings.agent_model_provider ?? 'anthropic';
const placeholderModel = DEFAULT_MODELS[currentProvider] ?? 'enter model id';
```

### Existing Pattern: AgentApiKeyField — Already Works for Any Provider

`AgentApiKeyField` receives `provider: string` and uses it as a key in `auth.json`. Since `OAUTH_PROVIDERS` does not include `groq` or `openrouter`, those providers will render only the API key input path (no OAuth button). [VERIFIED: AgentSection.tsx lines 5-31]

The component already handles:
- Load saved key on mount (masked display)
- Save via `agentSetApiKey(provider, key)`
- Remove via `agentRemoveAuth(provider)`
- Change/cancel editing flow

No changes needed in `AgentApiKeyField`. The existing render at line 263–268 passes `settings.agent_model_provider ?? 'anthropic'` — this automatically works for groq/openrouter once the dropdown is updated.

### Critical Gap: entrypoint.ts Hard-Codes Default Model

`app/agent/entrypoint.ts` line 49 hard-codes:
```ts
const defaultModel = getModel("anthropic", "claude-sonnet-4-20250514");
```

This means after restart, the sidecar always starts with Anthropic regardless of saved settings. The sidecar needs to read `settings.json` at startup to use the saved provider/model.

**Solution pattern** — read settings.json from authDir parent:
```ts
// In entrypoint.ts — after authDir is resolved
import { readFileSync, existsSync } from "node:fs";

function readSavedSettings(authDir: string) {
  const settingsPath = path.join(authDir, "settings.json");
  if (!existsSync(settingsPath)) return null;
  try {
    return JSON.parse(readFileSync(settingsPath, "utf-8"));
  } catch {
    return null;
  }
}

const savedSettings = readSavedSettings(authDir);
const savedProvider = savedSettings?.agent_model_provider ?? "anthropic";
const savedModelId = savedSettings?.agent_model_id ?? "claude-sonnet-4-20250514";
const defaultModel = modelRegistry.find(savedProvider, savedModelId)
  ?? getModel("anthropic", "claude-sonnet-4-20250514");
```

**Key insight:** `ModelRegistry.find(provider, modelId)` returns `Model | undefined` — the fallback to Anthropic default is correct behavior when the model is not found (e.g., user entered an invalid model ID).

**Where is settings.json?** The `authDir` is `app.path().app_config_dir()` per `cmd_start_agent` (commands.rs:1244–1249). The `SettingsState::load_or_new` in lib.rs loads from `config_dir.join("settings.json")` (state.rs:47). So `settings.json` lives in the same directory as `auth.json`. [VERIFIED: state.rs, commands.rs]

### Existing Pattern: Runtime Model Switch

When a user changes the provider/model in settings while the agent is running, `saveAgentSettings` persists to disk. The agent is NOT live-switched — a restart is required. The existing restart banner in `Settings.tsx` (lines 76–81) triggers on `hasUnsavedChanges`, but note the current `AgentSection.tsx` does not call `onUnsavedChange`. The restart flow is: user saves settings → banner appears → user clicks "Restart Application" → sidecar restarts → reads settings.json → uses new provider.

**Verify:** Does saving provider trigger the restart banner? Look at how `onUnsavedChange` is wired in `Settings.tsx` — it's passed to `NodeSection` but not `AgentSection`. This means provider changes currently do NOT show a restart banner. This is acceptable per the CONTEXT.md decisions ("existing restart flow works") but worth noting: the banner will appear only if AgentSection calls `onUnsavedChange` or if the developer adds that.

### Anti-Patterns to Avoid

- **Don't validate API key format in the UI** — CONTEXT.md locked: no format validation, runtime errors are sufficient.
- **Don't add a "Test connection" button** — Out of scope per REQUIREMENTS.md.
- **Don't use `ModelRegistry.create()` with a modelsJsonPath** — CONTEXT.md locked: keep `inMemory()` for now.
- **Don't add Groq/OpenRouter to `OAUTH_PROVIDERS`** — These providers are API key only; adding them would show an OAuth button with no working flow.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| API key masking/display | Custom masking logic | `AgentApiKeyField` (existing) | Already handles mask, show/hide, save, remove |
| Auth persistence | Direct file I/O | `cmd_agent_set_api_key` Tauri command | Handles permissions (0600 on Unix), atomic R-M-W |
| Provider model lookup | Hard-coded model objects | `ModelRegistry.find(provider, modelId)` | Already handles unknown models, returns undefined |
| Settings persistence | Direct file writes | `cmd_save_agent_settings` Tauri command | Handles serialization, state sync, event emission |

---

## Common Pitfalls

### Pitfall 1: entrypoint.ts Default Model Not Updated
**What goes wrong:** Agent always uses Anthropic even after selecting Groq/OpenRouter in settings and restarting.
**Why it happens:** `entrypoint.ts` line 49 is hardcoded to `getModel("anthropic", "claude-sonnet-4-20250514")`.
**How to avoid:** Read `settings.json` from `authDir` at startup and call `modelRegistry.find(provider, modelId)`.
**Warning signs:** After saving Groq as provider and restarting, agent responds with `claude-sonnet` in its AssistantMessage metadata.

### Pitfall 2: Model Input Placeholder Stays "claude-sonnet"
**What goes wrong:** After selecting Groq, the model input still shows "claude-sonnet-4-20250514" as placeholder, confusing users.
**Why it happens:** The placeholder is static in the current code.
**How to avoid:** Derive placeholder from current provider selection using a `DEFAULT_MODELS` map.
**Warning signs:** User selects Groq but leaves model field empty; agent uses Anthropic default because empty string → null → fallback.

### Pitfall 3: Provider String Case Mismatch
**What goes wrong:** Saving `"Groq"` (capitalized from a display label) instead of `"groq"` means `ModelRegistry.find("Groq", ...)` returns `undefined`.
**Why it happens:** `<option value="groq">` value must match the KnownProvider string exactly.
**How to avoid:** Option `value` attributes must be lowercase: `"groq"`, `"openrouter"`. [VERIFIED: types.d.ts — KnownProvider union uses lowercase]
**Warning signs:** Auth key stored as `"Groq"` in auth.json but ModelRegistry looks for `"groq"`.

### Pitfall 4: Restart Banner Not Shown After Provider Change
**What goes wrong:** User changes provider and expects to see "Restart for changes to take effect" banner, but it doesn't appear.
**Why it happens:** `AgentSection` is not wired to `onUnsavedChange` in `Settings.tsx`.
**How to avoid:** Either wire `onUnsavedChange` into `AgentSection` props, or accept the current behavior (no banner) since the agent reads settings at startup automatically.
**Warning signs:** User changes provider, doesn't restart, agent still uses old provider.

---

## Code Examples

### How auth.json looks after setting a Groq API key
```json
{
  "anthropic": { "type": "api_key", "key": "sk-ant-..." },
  "groq": { "type": "api_key", "key": "gsk_..." },
  "openrouter": { "type": "api_key", "key": "sk-or-..." }
}
```
[VERIFIED: commands.rs:1500-1508 — `{ "type": "api_key", "key": api_key }` format]

### How ModelRegistry.inMemory resolves Groq auth
```ts
// Source: model-registry.d.ts — existing API
const model = modelRegistry.find("groq", "llama-3.3-70b-versatile");
// Returns Model with baseUrl: "https://api.groq.com/openai/v1", api: "openai-completions"
const auth = await modelRegistry.getApiKeyAndHeaders(model);
// Reads from AuthStorage (auth.json) under key "groq"
```
[VERIFIED: models.generated.js:3968-3969]

### how entrypoint.ts should resolve startup model
```ts
// Read saved provider/model from settings.json (in authDir)
const settingsPath = path.join(authDir, "settings.json");
let startProvider = "anthropic";
let startModelId = "claude-sonnet-4-20250514";
try {
  if (existsSync(settingsPath)) {
    const saved = JSON.parse(readFileSync(settingsPath, "utf-8"));
    startProvider = saved.agent_model_provider ?? startProvider;
    startModelId = saved.agent_model_id ?? startModelId;
  }
} catch { /* use defaults */ }
const defaultModel = modelRegistry.find(startProvider, startModelId)
  ?? getModel("anthropic", "claude-sonnet-4-20250514");
```

### AgentSection.tsx — full updated provider dropdown
```tsx
// Source: app/src/components/settings/AgentSection.tsx (modified)
const DEFAULT_MODELS: Record<string, string> = {
  anthropic: 'claude-sonnet-4-20250514',
  google: 'gemini-2.0-flash',
  groq: 'llama-3.3-70b-versatile',
  openai: 'gpt-4o',
  openrouter: 'anthropic/claude-sonnet-4-20250514',
};

// Provider dropdown (alphabetical)
<option value="anthropic">Anthropic</option>
<option value="google">Google</option>
<option value="groq">Groq</option>
<option value="openai">OpenAI</option>
<option value="openrouter">OpenRouter</option>

// Model input — dynamic placeholder
<input
  placeholder={DEFAULT_MODELS[settings.agent_model_provider ?? 'anthropic'] ?? 'enter model id'}
  ...
/>
```

---

## Files to Change

| File | Change | Lines |
|------|--------|-------|
| `app/src/components/settings/AgentSection.tsx` | Add Groq + OpenRouter options to provider dropdown; add `DEFAULT_MODELS` map; update model input placeholder to be dynamic | ~203-235 |
| `app/agent/entrypoint.ts` | Read `settings.json` at startup to resolve initial provider/model instead of hard-coded Anthropic | ~44-50 |

**No Rust changes.** No new packages. No schema changes. No new Tauri commands.

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Hard-coded Anthropic default in entrypoint.ts | Read settings.json at startup | Enables any saved provider to take effect after restart |
| 3-provider dropdown (Anthropic/OpenAI/Google) | 5-provider dropdown (+ Groq + OpenRouter) | Fulfills PROV-01, PROV-02 |
| Static "claude-sonnet" placeholder | Dynamic placeholder per provider | Better UX — user sees expected model ID for selected provider |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `settings.json` lives in the same directory as `auth.json` (i.e., `app_config_dir()`) | Code Examples / Architecture | entrypoint.ts reads wrong path; low risk since both use `authDir` from same env var |
| A2 | `createAgentSessionFromServices` uses `defaultModel` parameter as the startup model | Architecture Patterns | entrypoint.ts change wouldn't take effect; verify by checking `createAgentSessionFromServices` signature |
| A3 | Google default model is `gemini-2.0-flash` (used in DEFAULT_MODELS map) | Code Examples | Wrong placeholder for Google — low impact since user can type any model |

**Notes on A1:** `authDir` env var in `cmd_start_agent` is set to `app_config_dir()` (commands.rs:1244). The `SettingsState::load_or_new` reads from `config_dir.join("settings.json")` (state.rs:47). Same base path → same directory. [VERIFIED]

**Notes on A2:** `createAgentSessionFromServices` call at entrypoint.ts:119 passes `model: defaultModel ?? undefined`. The `model-registry.d.ts` shows `ModelRegistry.find()` returns `Model<Api> | undefined`. The fallback chain (`?? getModel(...)`) is correct. [VERIFIED: model-registry.d.ts:60]

---

## Open Questions

1. **Does saving provider/model trigger the restart banner?**
   - What we know: `Settings.tsx` passes `onUnsavedChange` to `NodeSection` but not `AgentSection`
   - What's unclear: Whether Phase 7 should wire this up, or leave it to Phase 9 (settings UX)
   - Recommendation: Wire `onUnsavedChange` into AgentSection for provider/model changes — it's a one-line addition and prevents user confusion

2. **Should the model field auto-populate the default when provider changes?**
   - What we know: Currently the field shows the last saved model ID (or empty)
   - What's unclear: If user switches from Anthropic (claude-sonnet) to Groq, should the model field auto-fill `llama-3.3-70b-versatile`?
   - Recommendation: Auto-fill on provider change, but only if the current model_id field is empty or matches the previous provider's default. This is Claude's discretion per CONTEXT.md.

---

## Environment Availability

Step 2.6: SKIPPED — this phase is code/config-only changes to existing TypeScript and JSX files. No external tools, databases, or new runtimes required. All dependencies (Node.js, npm packages, Tauri) are already installed and verified by the existing dev environment.

---

## Sources

### Primary (HIGH confidence)
- `app/src/components/settings/AgentSection.tsx` — full component code verified
- `app/agent/entrypoint.ts` — startup model resolution verified (hard-coded Anthropic)
- `app/agent/node_modules/@mariozechner/pi-ai/dist/types.d.ts` — KnownProvider union type verified (groq, openrouter present at line 5)
- `app/agent/node_modules/@mariozechner/pi-ai/dist/models.generated.js` — groq baseUrl `https://api.groq.com/openai/v1` and openrouter baseUrl `https://openrouter.ai/api/v1` verified
- `app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/model-registry.d.ts` — ModelRegistry.find() and ModelRegistry.inMemory() signatures verified
- `app/src-tauri/src/commands.rs` — auth commands and cmd_save_agent_settings verified as provider-agnostic
- `packages/gui/shared/src/settings.rs` — Settings struct with agent_model_provider/agent_model_id fields verified
- `app/src/types/index.ts` — TypeScript Settings interface verified
- `.planning/phases/07-groq-openrouter-providers/07-CONTEXT.md` — locked decisions

### Secondary (MEDIUM confidence)
- `app/src/pages/Settings.tsx` — restart banner and AgentSection wiring verified
- `app/src/App.tsx` — agent auto-start flow verified (does not pass provider/model to sidecar)
- `app/src-tauri/src/agent.rs` — PiSidecarConfig struct verified (no provider fields)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all packages verified in node_modules
- Architecture: HIGH — all three files read directly from source
- Pitfalls: HIGH — identified from actual code gaps (hard-coded default, missing restart banner wiring)

**Research date:** 2026-04-08
**Valid until:** 2026-05-08 (stable domain — pi-ai package updates are the main risk)
