# Architecture Research

**Domain:** Open-source AI provider integration + Settings page UX refactor
**Researched:** 2026-04-08
**Confidence:** HIGH — all findings from direct codebase inspection

---

## System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Tauri Desktop App                               │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                   React Frontend                             │   │
│  │                                                              │   │
│  │  Settings.tsx (page)                                         │   │
│  │  ┌───────────────────┐  ┌───────────────────────────────┐   │   │
│  │  │  SettingsSidebar  │  │  Content area (tab-switched)  │   │   │
│  │  │  (activeSection   │  │  AgentSection                 │   │   │
│  │  │   state drives    │  │  EnvironmentSection           │   │   │
│  │  │   conditional     │  │  WalletSection, etc.          │   │   │
│  │  │   rendering)      │  │                               │   │   │
│  │  └───────────────────┘  └───────────────────────────────┘   │   │
│  │                                                              │   │
│  │  tauri/agent.ts (invoke bridge)                              │   │
│  │   agentSetModel(provider, modelId)                           │   │
│  │   agentSetApiKey(provider, apiKey)                           │   │
│  │   saveAgentSettings({ agent_model_provider, ... })           │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                          │ Tauri IPC                                │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │               Rust Backend (commands.rs)                     │   │
│  │                                                              │   │
│  │  cmd_agent_set_model → JSON-RPC to sidecar stdin             │   │
│  │  cmd_agent_set_api_key → write to auth.json (0600)           │   │
│  │  cmd_save_agent_settings → SettingsState → settings.json     │   │
│  │                                                              │   │
│  │  PiSidecarState (agent.rs)                                   │   │
│  │   spawn: npx tsx entrypoint.ts                               │   │
│  │   stdin/stdout JSON-line RPC channel                         │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                          │ process stdio                            │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │               Pi Sidecar (agent/entrypoint.ts)               │   │
│  │                                                              │   │
│  │  AuthStorage.create(auth.json)                               │   │
│  │  ModelRegistry.inMemory(authStorage)  <- KEY: no models.json  │   │
│  │                                                              │   │
│  │  runRpcMode(runtime)                                         │   │
│  │   handles: set_model, set_thinking_level, prompt, etc.       │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  Persistent storage (app config dir)                                │
│   auth.json  — provider credentials (api_key / oauth)              │
│   settings.json — agent_model_provider, agent_model_id, etc.       │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Integration Point 1: Open-Source AI Providers in the Pi Sidecar

### What pi-ai supports natively (HIGH confidence)

From `@mariozechner/pi-ai` v0.65.x `types.d.ts`, the `KnownProvider` union includes:

```
"groq" | "openrouter" | "huggingface" | "mistral" | "cerebras" | "xai"
```

**Groq** and **OpenRouter** are built-in with pre-configured `baseUrl` values and model definitions in `models.generated.d.ts`. Groq uses `api: "openai-completions"`. These work today without any extra sidecar config — just an API key in `auth.json` under the `"groq"` key.

**Together AI** is NOT a `KnownProvider` — it is not in the generated models list. Together AI would need to be added as a custom provider via `registerProvider()`.

**Ollama** is NOT in `KnownProvider`. Ollama requires a custom provider registration with a local `baseUrl`.

### The `ModelRegistry.inMemory()` gap

The entrypoint uses `ModelRegistry.inMemory(authStorage)` which bypasses disk-based `models.json` loading entirely. The `ModelRegistry.create(authStorage, modelsJsonPath)` variant supports a `models.json` file that can declare custom providers.

Two approaches to add open-source providers:

**Option A — Switch to `ModelRegistry.create` with a disk-based `models.json`** (MEDIUM complexity)

The sidecar's `authDir` is already `WAVS_AUTH_DIR` (app config dir). Change the entrypoint to:

```typescript
const modelRegistry = ModelRegistry.create(
  authStorage,
  path.join(authDir, "models.json")
);
```

The `models.json` file format (loaded by `loadCustomModels`) is managed by the pi-ai ecosystem. The app could write this file when users configure custom providers. The schema from `ProviderConfigInput` is:

```json
{
  "providers": {
    "ollama": {
      "baseUrl": "http://localhost:11434/v1",
      "api": "openai-completions",
      "models": [
        {
          "id": "llama3.2:3b",
          "name": "Llama 3.2 3B",
          "reasoning": false,
          "input": ["text"],
          "contextWindow": 131072,
          "maxTokens": 8192,
          "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 }
        }
      ]
    }
  }
}
```

**Option B — Add a `register_provider` RPC command** (LOW complexity, more targeted)

The pi-coding-agent's `ModelRegistry` has `registerProvider(providerName, config: ProviderConfigInput)`. The sidecar could expose this via a new RPC command type. The Rust side would send:

```json
{
  "type": "register_provider",
  "provider": "ollama",
  "baseUrl": "http://localhost:11434/v1",
  "api": "openai-completions"
}
```

However, `register_provider` is not in `RpcCommand` today — it would need to be added to the sidecar entrypoint with a custom pre-processing step before `runRpcMode`.

**Recommendation: Option A** (disk-based `models.json`). It aligns with how pi-coding-agent is designed to be extended, requires one line change in `entrypoint.ts` (swapping `inMemory` for `create`), and lets the Rust backend manage provider config by writing `models.json`. No new RPC protocol needed.

### Auth key routing for custom providers

`AuthStorage` resolves API keys with this priority:
1. Runtime override (`setRuntimeApiKey`)
2. `auth.json` entry for provider name
3. OAuth token
4. Environment variable
5. Fallback resolver

For Ollama (no API key needed), the ModelRegistry will send requests without auth — the `openai-completions` provider does not require an auth header if `authHeader: false` is set in the `ProviderConfigInput`. This needs to be set in the `models.json` provider config.

For Together AI (using OpenAI-compatible API), the provider name in `auth.json` must match the key used in `models.json`. If registered as `"together"`, the API key is stored as `auth.json["together"]["key"]`.

### Settings persistence for custom providers

The current `settings.rs` `Settings` struct stores:
- `agent_model_provider: Option<String>` — provider name string
- `agent_model_id: Option<String>` — model ID string
- `agent_thinking_level: Option<String>`

For open-source providers, two additional fields are needed:

```rust
pub agent_custom_providers: Vec<CustomProviderConfig>,  // NEW
```

Where `CustomProviderConfig` is a new struct:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomProviderConfig {
    pub name: String,        // e.g. "ollama", "together"
    pub base_url: String,    // e.g. "http://localhost:11434/v1"
    pub api: String,         // e.g. "openai-completions"
    pub requires_api_key: bool,
    pub models: Vec<CustomModelConfig>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomModelConfig {
    pub id: String,
    pub name: String,
    pub context_window: u64,
    pub max_tokens: u64,
}
```

The Rust backend writes this to `models.json` in the auth dir when providers are saved. The sidecar picks it up on startup (or via a `reload_models` RPC if hot-reload is needed).

### RPC set_model with custom provider

The existing `set_model` RPC is:
```json
{ "type": "set_model", "provider": "groq", "modelId": "llama-3.3-70b-versatile" }
```

No change needed here — `set_model` takes a provider string and modelId string. Once the provider is registered in ModelRegistry (via `models.json`), `find(provider, modelId)` returns the model and the agent switches to it.

---

## Integration Point 2: Settings Page — Tab-Switch to Scroll+Anchor

### Current architecture (HIGH confidence)

`Settings.tsx` manages:
- `activeSection: SectionKey` state — drives which section component renders
- The sidebar calls `setActiveSection` on click
- Content area: `max-h-[calc(100vh-12rem)] overflow-y-auto` with conditional rendering

```
Settings.tsx
+-- SettingsSidebar (activeSection, onSelect) -- tab nav
+-- div.overflow-y-auto
    +-- {activeSection === 'wallet' && <WalletSection />}
        {activeSection === 'node' && <NodeSection />}
        ... (one section rendered at a time, others unmount)
```

The key architectural decision in v1.0: the OAuth listener lives in `Settings.tsx` (parent) so it survives section navigation. This constraint must be preserved in the refactor.

### Target architecture: Single scrollable page with anchor links

All sections render simultaneously in a vertical scroll container. The sidebar highlights the section currently in the viewport (scroll-spy).

```
Settings.tsx
+-- SettingsSidebar (activeSection, onAnchorClick) -- anchor nav, scroll-spy highlight
+-- div#settings-scroll-container.overflow-y-auto
    +-- section#wallet  <WalletSection />
    +-- section#node    <NodeSection />
    +-- section#environment  <EnvironmentSection />
    +-- section#agent   <AgentSection />
    +-- section#mcp     <McpSection />
    +-- section#reset   <ResetSection />
```

### Scroll-spy implementation pattern

Use `IntersectionObserver` in `Settings.tsx` to watch each section ref. When a section enters the viewport, update `activeSection` state. Sidebar reads `activeSection` for highlight (same as today) but instead of calling `setActiveSection`, sidebar clicks call `scrollIntoView`.

```typescript
// In Settings.tsx
const sectionRefs = useRef<Record<SectionKey, HTMLElement | null>>({
  wallet: null, node: null, environment: null, agent: null, mcp: null, reset: null
});

useEffect(() => {
  const observer = new IntersectionObserver(
    (entries) => {
      // Set activeSection to the first intersecting section
      for (const entry of entries) {
        if (entry.isIntersecting) {
          setActiveSection(entry.target.id as SectionKey);
          break;
        }
      }
    },
    { threshold: 0.3, root: scrollContainerRef.current }
  );
  Object.values(sectionRefs.current).forEach(el => el && observer.observe(el));
  return () => observer.disconnect();
}, []);

const handleAnchorClick = (key: SectionKey) => {
  sectionRefs.current[key]?.scrollIntoView({ behavior: 'smooth' });
};
```

### SettingsSidebar interface change

The `onSelect` prop changes semantics but not signature:

```typescript
// Before: onSelect triggers conditional render
// After: onSelect triggers scrollIntoView
interface SettingsSidebarProps {
  activeSection: SectionKey;
  onSelect: (key: SectionKey) => void;  // same prop name, new behavior
}
```

The sidebar component itself does not change — it still receives `activeSection` for highlighting and calls `onSelect` on click. The parent changes what `onSelect` does.

### OAuth listener preservation

The OAuth listener in `Settings.tsx` is in a `useEffect` at the page level. Since `Settings.tsx` itself is not unmounted during the refactor (only the per-section conditional rendering is removed), the OAuth listener continues to work without changes.

---

## Component Boundaries

### New vs. Modified Components

| Component | Status | Change |
|-----------|--------|--------|
| `Settings.tsx` | Modified | Remove activeSection-driven conditional rendering; add scroll container, IntersectionObserver, scrollIntoView handler |
| `SettingsSidebar.tsx` | Modified | Export type remains `SectionKey`; sidebar may need a new section for "Providers" or the Agent section expands |
| `AgentSection.tsx` | Modified | Add provider dropdown options (Groq, OpenRouter, Ollama, custom); add base URL field when provider requires it |
| `EnvironmentSection.tsx` | Optional minor | May add AI provider suggestions more prominently |
| `settings.rs` | Modified | Add `agent_custom_providers: Vec<CustomProviderConfig>` field |
| `commands.rs` | Modified | Add `cmd_save_custom_providers` or extend `cmd_save_agent_settings` to handle provider configs; add `cmd_write_models_json` |
| `agent/entrypoint.ts` | Modified | Switch `ModelRegistry.inMemory(authStorage)` to `ModelRegistry.create(authStorage, path.join(authDir, 'models.json'))` |
| `tauri/agent.ts` | Modified | Add `saveCustomProviders()` invoke wrapper |

**New components (if needed):**

| Component | Purpose |
|-----------|---------|
| `settings/CustomProviderForm.tsx` | Form for adding Ollama/custom provider: name, baseUrl, model list |
| `settings/ProviderSection.tsx` | Standalone section if provider config grows beyond AgentSection scope |

---

## Data Flow

### Provider Configuration Save Flow

```
User fills CustomProviderForm
  |
saveCustomProviders([{name, baseUrl, api, models}])   [tauri/agent.ts]
  |
cmd_save_custom_providers                              [commands.rs]
  | (1) Write models.json to auth_dir/models.json
  | (2) Update Settings.agent_custom_providers and persist settings.json
  | (3) If agent is running: restart sidecar so it re-reads models.json
        (use existing cmd_stop_agent + cmd_start_agent flow)

Sidecar startup:
  ModelRegistry.create(authStorage, ".../models.json")
  -> loadCustomModels() reads models.json
  -> mergeCustomModels() merges into built-in model list
  -> set_model RPC now resolves custom provider
```

### Model Selection Flow (unchanged from today)

```
User selects provider + model in AgentSection
  |
saveAgentSettings({ agent_model_provider, agent_model_id })   [tauri/agent.ts]
  |
cmd_save_agent_settings -> Settings persisted                  [commands.rs]
  |
agentSetModel(provider, modelId)                              [tauri/agent.ts]
  |
cmd_agent_set_model -> {"type":"set_model","provider":...}     [commands.rs -> sidecar stdin]
  |
runRpcMode handles set_model
  -> modelRegistry.find(provider, modelId)
  -> session.setModel(model)
```

### Scroll Navigation Flow (new)

```
User clicks sidebar item (e.g., "Agent")
  |
handleAnchorClick("agent")                                    [Settings.tsx]
  |
sectionRefs.current["agent"].scrollIntoView({behavior:'smooth'})
  |
IntersectionObserver fires as section enters viewport
  |
setActiveSection("agent")
  |
SettingsSidebar re-renders with activeSection="agent" highlighted
```

---

## Recommended Project Structure (changes only)

```
app/
+-- agent/
|   +-- entrypoint.ts          # change ModelRegistry.inMemory -> .create
+-- src/
|   +-- components/settings/
|   |   +-- AgentSection.tsx   # extend provider dropdown + conditional baseUrl field
|   |   +-- SettingsSidebar.tsx # no interface change; possibly new section entry
|   |   +-- CustomProviderForm.tsx  # NEW: Ollama / custom provider config UI
|   |   +-- ProviderSection.tsx    # NEW (optional): if provider config is large
|   +-- pages/
|       +-- Settings.tsx       # scroll refactor: remove conditional render, add refs + observer
+-- src-tauri/src/
|   +-- commands.rs            # extend cmd_save_agent_settings or add cmd_write_models_json
+-- packages/gui/shared/src/
    +-- settings.rs            # add CustomProviderConfig structs + Vec field
```

---

## Architectural Patterns

### Pattern 1: Scroll-Spy with IntersectionObserver

**What:** All sections render at once in a scrollable container. IntersectionObserver watches section elements; active section highlight follows scroll position.

**When to use:** Settings pages with 5+ sections, where users want to scan all settings, not tab between isolated views.

**Trade-offs:**
- Pro: All settings visible on one page; natural browser scroll behavior
- Pro: Anchor links (shareable URLs with `#agent` hash) possible as a future extension
- Con: All sections mount simultaneously (more DOM); section components must not have expensive mount effects
- Con: IntersectionObserver thresholds need tuning to feel right — `threshold: 0.3` with `root` set to the scroll container is a good starting point

### Pattern 2: models.json as the Provider Config Contract

**What:** The pi-coding-agent's disk-based `models.json` is the single source of truth for custom providers. The Rust backend writes it; the TypeScript sidecar reads it on startup.

**When to use:** When extending a third-party library (pi-coding-agent) that already has a file-based extension mechanism.

**Trade-offs:**
- Pro: No changes to the RPC protocol; no new RPC commands
- Pro: Aligns with how pi-coding-agent itself expects to be configured by users
- Con: Configuration changes require sidecar restart (or a `reload_models` RPC if pi-coding-agent exposes one)
- Con: The `models.json` schema is determined by pi-coding-agent, not this project

### Pattern 3: Provider-Specific UI Branching in AgentSection

**What:** The AgentSection renders a `baseUrl` field only when the selected provider requires it (Ollama, custom). Known providers (Groq, OpenRouter, Anthropic) do not show it.

```typescript
const needsBaseUrl = (provider: string) =>
  provider === 'ollama' || provider === 'custom';
```

**When to use:** Form fields that only apply to certain selections.

**Trade-offs:**
- Pro: Keeps the UI uncluttered for the common case
- Con: Logic for "which providers need a base URL" must be kept in sync between frontend and backend

---

## Anti-Patterns

### Anti-Pattern 1: Storing base_url in auth.json

**What people do:** Shove the provider base URL into `auth.json` alongside the API key because that file is already there.

**Why it's wrong:** `auth.json` is credential storage (API keys, OAuth tokens). Base URLs are configuration, not credentials. Mixing them makes the file semantically confusing and harder to reason about.

**Do this instead:** Store base URL in `settings.json` (via `Settings.agent_custom_providers`) and write `models.json` separately for the sidecar to consume.

### Anti-Pattern 2: Hardcoding the provider list in the frontend

**What people do:** Hardcode `['anthropic', 'openai', 'google', 'groq', 'ollama', ...]` in the React component, duplicating what pi-ai's `KnownProvider` union already defines.

**Why it's wrong:** The list diverges from reality as pi-ai updates. Users on a newer pi-ai version may be confused when providers they configured do not appear.

**Do this instead:** For built-in providers (Groq, OpenRouter, etc.), expose a `get_available_providers` command that reads from the sidecar's `get_available_models` RPC. For custom providers, merge from `settings.agent_custom_providers`. The UI shows the union.

### Anti-Pattern 3: Keeping conditional rendering for the scroll refactor

**What people do:** Keep the tab-switch pattern (conditional rendering) but animate between sections to simulate scrolling.

**Why it's wrong:** Sections still mount/unmount (losing ephemeral state like unsaved form data), and the animation adds complexity without the natural scanability of a real scroll page.

**Do this instead:** Render all sections simultaneously. Accept that all section components mount at page load. Use `React.memo` if any section has expensive initialization.

### Anti-Pattern 4: Writing models.json on every keypress

**What people do:** Eagerly write the models.json file as the user types in the base URL field.

**Why it's wrong:** File I/O on every keystroke causes flicker and race conditions if the sidecar reads the file between writes.

**Do this instead:** Write models.json only when the user clicks "Save". Keep intermediate edits in React state.

---

## Integration Points Summary

### What Already Works (no changes needed)

| Integration | Notes |
|-------------|-------|
| Groq API key -> auth.json["groq"] | Groq is a KnownProvider with built-in models |
| OpenRouter API key -> auth.json["openrouter"] | KnownProvider with many hosted models |
| `set_model` RPC with known providers | No changes to RPC protocol needed |
| OAuth flow for Anthropic/OpenAI/Google | Must be preserved |
| Settings.tsx OAuth listener | Survives the scroll refactor since Settings.tsx itself stays mounted |

### What Needs to Be Built

| Integration | Complexity | Where |
|-------------|------------|-------|
| `models.json` write on provider save | Low | `commands.rs` new function |
| Switch entrypoint to `ModelRegistry.create` | Trivial | `agent/entrypoint.ts` (1 line) |
| `CustomProviderConfig` struct in settings | Low | `packages/gui/shared/src/settings.rs` |
| AgentSection provider dropdown expansion | Low | `AgentSection.tsx` (add options + conditional baseUrl) |
| Custom provider form UI | Medium | New `CustomProviderForm.tsx` |
| Settings page scroll refactor | Medium | `Settings.tsx` + `SettingsSidebar.tsx` |
| Sidecar restart on provider config change | Low | Existing `cmd_stop_agent` + `cmd_start_agent` |

---

## Build Order

Dependencies determine sequencing:

1. **`settings.rs` schema** — Add `CustomProviderConfig` structs. Everything else depends on the shape of persisted config. (Rust compile only, no frontend changes yet.)

2. **`commands.rs` backend** — Implement `cmd_save_custom_providers` (or extend `cmd_save_agent_settings`). Writes `models.json` to auth dir. Blocked by step 1.

3. **`agent/entrypoint.ts` — ModelRegistry switch** — One-line change from `inMemory` to `create`. Can be done independently of the frontend; test by manually creating `models.json` in auth dir.

4. **`tauri/agent.ts` bridge** — Add `saveCustomProviders()` invoke wrapper. Blocked by step 2.

5. **`AgentSection.tsx` — provider dropdown expansion** — Add Groq, OpenRouter, and "Ollama (local)" to the provider select. Show `baseUrl` field conditionally. Blocked by step 4 for persistence.

6. **`CustomProviderForm.tsx`** — Full custom provider UI. Blocked by steps 4-5.

7. **Settings page scroll refactor** — Independent of steps 1-6. Can be built in parallel. Only touches `Settings.tsx` and `SettingsSidebar.tsx`.

---

## Sources

- Direct inspection: `/workspace/app/agent/entrypoint.ts`
- Direct inspection: `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/model-registry.d.ts`
- Direct inspection: `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/auth-storage.d.ts`
- Direct inspection: `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/modes/rpc/rpc-types.d.ts`
- Direct inspection: `/workspace/app/agent/node_modules/@mariozechner/pi-ai/dist/types.d.ts` (KnownProvider list)
- Direct inspection: `/workspace/app/agent/node_modules/@mariozechner/pi-ai/dist/models.generated.d.ts` (groq, openrouter, huggingface present; together/ollama absent)
- Direct inspection: `/workspace/app/src/components/settings/AgentSection.tsx`
- Direct inspection: `/workspace/app/src/pages/Settings.tsx`
- Direct inspection: `/workspace/app/src-tauri/src/commands.rs` (lines 1326-1608)
- Direct inspection: `/workspace/packages/gui/shared/src/settings.rs`

---

*Architecture research for: WAVS v1.1 Open-Source AI Providers + Settings UX*
*Researched: 2026-04-08*
