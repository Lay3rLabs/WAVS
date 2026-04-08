# Stack Research

**Domain:** Open-source AI provider configuration + scrollable settings UX (WAVS v1.1)
**Researched:** 2026-04-08
**Confidence:** HIGH — primary research is reading installed SDK dist types and existing Rust source, not inference

## Context

This is a **subsequent milestone** addendum. The existing v1.0 stack (Tauri 2, React 19, Vite 7, Tailwind 3, Zustand 5, pi-coding-agent 0.65.0) is validated. This document covers only the new capabilities needed for v1.1.

**Key finding:** No new npm packages or Rust crates are needed. All required APIs exist in already-installed dependencies.

---

## Recommended Stack — New Capabilities Only

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `@mariozechner/pi-coding-agent` | 0.65.0 (already installed) | `ModelRegistry.registerProvider()` for adding OpenAI-compat providers at sidecar startup | Already in use. `registerProvider(name, { baseUrl, api: "openai-completions", models })` is the correct SDK path — verified in `dist/core/model-registry.d.ts`. No new dep needed. |
| `@mariozechner/pi-ai` | 0.65.0 (already installed) | `openai-completions` api string + `OpenAICompletionsCompat` type covers any OpenAI-compat endpoint | Already in use. `KnownProvider` includes `groq`, `openrouter`, etc. Custom endpoints use `api: "openai-completions"` with a custom `baseUrl`. Verified in `dist/types.d.ts`. |
| React `useRef` + `IntersectionObserver` | React 19 (already installed) | Scrollspy to track which section is active for sidebar highlight | Native browser API available in Tauri's Chromium WebView. Zero deps. Matches existing codebase style. |
| Tailwind `scroll-mt-*` | 3.4 (already installed) | Offset scroll target so section headings clear sticky headers | Available since Tailwind 3.1. Already using Tailwind throughout. |

### Supporting Libraries

No new libraries. All capabilities use already-installed code.

| Capability | Mechanism | Notes |
|------------|-----------|-------|
| OpenAI-compat endpoint | `modelRegistry.registerProvider(name, { baseUrl, apiKey, api: "openai-completions", models: [...] })` in `app/agent/entrypoint.ts` | Provider name, base URL, and model IDs come from `WAVS_OPENAI_COMPAT_PROVIDERS` env var injected at spawn. API keys resolve from `auth.json` via existing `AuthStorage`. |
| Ollama (no auth) | Same; `apiKey: "ollama"` as placeholder | Ollama ignores the key. Schema requires non-empty string. Base URL: `http://localhost:11434/v1`. |
| Groq / Together / LM Studio | Same; real API keys | Different `baseUrl` per provider. Groq: `https://api.groq.com/openai/v1`. Together: `https://api.together.xyz/v1`. LM Studio: `http://localhost:1234/v1`. |
| Sidebar anchor scroll | `element.scrollIntoView({ behavior: 'smooth', block: 'start' })` | No library. Replaces `setActiveSection` onClick. |
| Active section tracking | `IntersectionObserver` on section `ref` elements | Standard React `useEffect` pattern with cleanup. Updates `activeSection` to topmost visible section. |
| Settings persistence | Extend `Settings` struct + `cmd_save_agent_settings` | Existing Rust settings pattern. |

---

## How Custom Providers Are Configured: Architecture

### Critical SDK Finding

The sidecar creates `ModelRegistry.inMemory(authStorage)` — this passes `undefined` as `modelsJsonPath`, skipping `models.json` loading entirely (verified in `model-registry.js` line 182). Custom providers **cannot** be added by writing a file; they must be registered at startup via `registerProvider()`.

The RPC protocol has no `register_provider` command. The full `RpcCommand` union was checked exhaustively in `rpc-types.d.ts` — it contains `set_model`, `get_available_models`, and others, but no provider registration command. Providers must exist in the registry before the agent starts.

### Integration Path (5 steps)

**Step 1 — Settings struct (Rust):** Add to `packages/gui/shared/src/settings.rs`:

```rust
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct OpenAICompatProvider {
    pub name: String,         // Display name and provider key
    pub base_url: String,     // e.g. "http://localhost:11434/v1"
    pub model_ids: Vec<String>, // e.g. ["llama3.2", "mistral"]
}

// In Settings struct:
#[serde(default)]
pub agent_openai_compat_providers: Vec<OpenAICompatProvider>,
```

**Step 2 — Sidecar spawn (Rust, `agent.rs`):** Serialize provider list (no keys) and inject as env var:

```rust
let providers_json = serde_json::to_string(&config.openai_compat_providers).unwrap_or_default();
cmd.env("WAVS_OPENAI_COMPAT_PROVIDERS", providers_json);
```

API keys are NOT passed here — they live in `auth.json` which is already accessible to the sidecar via `WAVS_AUTH_DIR`.

**Step 3 — Sidecar startup (TypeScript, `entrypoint.ts`):** Read env var and register providers:

```typescript
const providersJson = process.env.WAVS_OPENAI_COMPAT_PROVIDERS;
if (providersJson) {
  const providers = JSON.parse(providersJson);
  for (const p of providers) {
    modelRegistry.registerProvider(p.name, {
      baseUrl: p.base_url,
      api: "openai-completions",
      models: p.model_ids.map(id => ({
        id,
        name: id,
        reasoning: false,
        input: ["text"] as const,
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
        contextWindow: 131072,
        maxTokens: 4096,
      })),
    });
  }
}
```

**Step 4 — Frontend UI (`AgentSection.tsx`):** New "Custom Providers" subsection:
- List of configured custom providers with delete buttons
- "Add Provider" form: name field, base URL field, model IDs field (newline-separated or tag input)
- Saves via `saveAgentSettings({ agent_openai_compat_providers: [...] })`
- API key for the custom provider goes through existing `agentSetApiKey(providerName, key)` — no new auth infrastructure

**Step 5 — Provider dropdown:** Custom providers appear in `<select>` below built-in providers (anthropic, openai, google). Built-in OAuth providers remain unchanged. Custom providers use API key auth only (no OAuth flow — `OAUTH_PROVIDERS` set stays as-is).

### `ProviderConfigInput` Schema Reference

From `model-registry.d.ts`:

```typescript
interface ProviderConfigInput {
  baseUrl?: string;
  apiKey?: string;       // Optional — resolved from auth.json if not given
  api?: Api;             // "openai-completions" for OpenAI-compat endpoints
  models?: Array<{
    id: string;
    name: string;
    reasoning: boolean;
    input: ("text" | "image")[];
    cost: { input: number; output: number; cacheRead: number; cacheWrite: number };
    contextWindow: number;
    maxTokens: number;
    compat?: OpenAICompletionsCompat;  // Optional; auto-detected from baseUrl
  }>;
}
```

All model fields have defaults in the schema. The `compat` field auto-detects from `baseUrl` (e.g., Ollama gets `requiresToolResultName: true` automatically).

---

## Scrollable Settings Page: Architecture

### Current Architecture (tab-swap)

`Settings.tsx` renders one section at a time (`{activeSection === 'wallet' && <WalletSection />}`). Clicking a sidebar item calls `setActiveSection`. Only one section is mounted at a time.

### Target Architecture (scroll-spy)

Convert to a single scrollable column with sticky sidebar anchors:

**Remove** all conditional renders in `Settings.tsx`. Render all sections stacked:

```tsx
<div ref={scrollRef} className="flex-1 overflow-y-auto px-6 py-4 max-h-[calc(100vh-12rem)]">
  <div id="section-wallet" ref={walletRef} className="scroll-mt-4"><WalletSection /></div>
  <div id="section-node" ref={nodeRef} className="scroll-mt-4"><NodeSection /></div>
  {/* ... rest of sections ... */}
</div>
```

**Sidebar becomes anchor nav.** Replace `onSelect={setActiveSection}` with scroll-to:

```tsx
onClick={() => document.getElementById(`section-${item.key}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' })}
```

**IntersectionObserver** tracks which section is topmost visible and updates `activeSection` for sidebar highlight:

```tsx
useEffect(() => {
  const refs = [walletRef, nodeRef, environmentRef, agentRef, mcpRef, resetRef];
  const keys: SectionKey[] = ['wallet', 'node', 'environment', 'agent', 'mcp', 'reset'];
  const observer = new IntersectionObserver(
    (entries) => {
      const visible = entries.filter(e => e.isIntersecting);
      if (visible.length > 0) setActiveSection(/* topmost */ ...);
    },
    { threshold: 0, rootMargin: '-10% 0px -80% 0px' }
  );
  refs.forEach(r => { if (r.current) observer.observe(r.current); });
  return () => observer.disconnect();
}, []);
```

`rootMargin: '-10% 0px -80% 0px'` means "trigger when the section top enters the top 20% of the scroll container" — produces natural active-section behavior matching common settings UX patterns.

**State change summary:**

| Before | After |
|--------|-------|
| `activeSection` controls which section renders | `activeSection` controls sidebar highlight only |
| `setActiveSection` called on sidebar click | `setActiveSection` called by IntersectionObserver; sidebar click scrolls |
| One section mounted at a time | All sections always mounted |
| OAuth listener in parent needed to survive nav | Sections always mounted; OAuth listener still fine in parent |

---

## Files That Change

| File | Change | What |
|------|--------|------|
| `packages/gui/shared/src/settings.rs` | Add | `OpenAICompatProvider` struct + `agent_openai_compat_providers` field |
| `app/src-tauri/src/agent.rs` | Extend | Serialize + inject `WAVS_OPENAI_COMPAT_PROVIDERS` env var at spawn |
| `app/src-tauri/src/commands.rs` | Extend | Handle `agent_openai_compat_providers` in `cmd_save_agent_settings` |
| `app/agent/entrypoint.ts` | Extend | Read env var, call `modelRegistry.registerProvider()` per entry at startup |
| `app/src/tauri/agent.ts` | Extend | Add `agent_openai_compat_providers` to `saveAgentSettings` type |
| `app/src/components/settings/AgentSection.tsx` | Extend | New "Custom Providers" subsection; extended provider `<select>` |
| `app/src/pages/Settings.tsx` | Refactor | Remove conditional renders, add `useRef` per section, add IntersectionObserver |
| `app/src/components/settings/SettingsSidebar.tsx` | Refactor | onClick scrolls to anchor; highlight driven by `activeSection` prop (unchanged interface) |

### Files That Do NOT Change

- `auth.json` format — `agentSetApiKey(providerName, key)` works unchanged for custom providers
- `AgentApiKeyField` component — accepts any string as `provider`, no changes needed
- Zustand `appStore` — settings propagate through existing `SettingsEvent` mechanism
- Vite / Tailwind config — no new config needed

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `registerProvider()` at startup via env var | Write `~/.pi/agent/models.json` | That path is pi's own config dir; WAVS writing there conflicts with user's standalone pi installation. Also `ModelRegistry.inMemory()` skips file loading entirely. |
| `registerProvider()` at startup | `register_provider` RPC command | Does not exist. Verified by exhaustive check of `RpcCommand` union in `rpc-types.d.ts`. |
| `IntersectionObserver` native | `react-scroll` npm package | Adds a dep for a pattern that's 15 lines of native code. No benefit in a Tauri Chromium context. |
| `IntersectionObserver` native | `react-intersection-observer` | Same argument. The wrapper adds no meaningful value for this use case. |
| Env var for provider config | New Tauri command to call `registerProvider` at runtime | Agent must be restarted to pick up new providers anyway (model registry is built at startup). Env var at spawn is simpler and aligns with existing `WAVS_AUTH_DIR` pattern. |
| `agent_openai_compat_providers` in settings | Separate file (e.g., `custom-providers.json`) | Keeps all user config in one place (`settings.json`). Avoids a new file to manage/migrate. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `ModelRegistry.create()` with `~/.pi/agent/models.json` | pi's config dir; conflicts with user's own pi install; current code uses `inMemory()` | `registerProvider()` called from entrypoint at startup |
| `openai` npm SDK on the frontend | Not needed; pi-ai wraps openai SDK internally in the sidecar | Nothing — pi-ai already handles this |
| Storing `base_url` in `auth.json` | `auth.json` is for credentials only; mixing config there breaks the isolation pattern | `agent_openai_compat_providers` field in `settings.json` via Settings struct |
| Any scrollspy library (`react-scroll`, `react-scrollspy`, etc.) | Zero benefit over native API in this context; adds a dep | Native `IntersectionObserver` + `scrollIntoView` |

---

## Version Compatibility

No new packages introduced. No compatibility concerns.

| Package | Version | Notes |
|---------|---------|-------|
| `@mariozechner/pi-coding-agent` | 0.65.0 | `registerProvider()` verified present in `dist/core/model-registry.d.ts` |
| `@mariozechner/pi-ai` | 0.65.0 | `openai-completions` api, `KnownProvider` union verified in `dist/types.d.ts` |
| `react` | 19.1.0 | `useRef`, `useEffect`, `IntersectionObserver` — no issues |
| `tailwindcss` | 3.4.0 | `scroll-mt-*` utility available since Tailwind 3.1 |

---

## Sources

- `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/model-registry.d.ts` — `registerProvider()`, `ProviderConfigInput`, `ModelRegistry.inMemory()` — HIGH confidence (dist types, source of truth)
- `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/model-registry.js` lines 182, 208 — confirmed `inMemory()` passes `undefined` path, skipping models.json — HIGH confidence
- `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/modes/rpc/rpc-types.d.ts` — full `RpcCommand` union exhaustively checked: no `register_provider` command exists — HIGH confidence
- `/workspace/app/agent/node_modules/@mariozechner/pi-ai/dist/types.d.ts` — `KnownProvider` union (includes `groq`, `openrouter`), `Model.baseUrl`, `OpenAICompletionsCompat` — HIGH confidence
- `/workspace/app/agent/entrypoint.ts` — confirmed `ModelRegistry.inMemory(authStorage)` in use — HIGH confidence
- `/workspace/app/src-tauri/src/agent.rs` — `PiSidecarConfig`, env var injection pattern (`WAVS_AUTH_DIR` etc.) — HIGH confidence
- `/workspace/packages/gui/shared/src/settings.rs` — current `Settings` struct fields — HIGH confidence
- `/workspace/app/src-tauri/src/commands.rs` — `cmd_save_agent_settings` pattern — HIGH confidence
- [Ollama OpenAI compatibility docs](https://docs.ollama.com/api/openai-compatibility) — `http://localhost:11434/v1` base URL pattern, placeholder apiKey requirement — MEDIUM confidence (web source)

---
*Stack research for: WAVS v1.1 open-source AI provider support + settings UX*
*Researched: 2026-04-08*
