# Project Research Summary

**Project:** WAVS v1.1 — Open-Source AI Providers + Settings UX
**Domain:** Desktop app (Tauri 2 + React) settings page extension; AI provider integration via pi-coding-agent sidecar
**Researched:** 2026-04-08
**Confidence:** HIGH — all core findings from direct codebase inspection of installed SDK dist types and existing Rust/TypeScript source

## Executive Summary

This milestone extends the WAVS desktop app with two tightly-scoped additions: open-source AI provider support (Ollama, Groq, Together AI, and others) in the agent settings, and a UX refactor that converts the settings page from tab-switching to a single scrollable page with sidebar anchor navigation. No new npm packages or Rust crates are required — all capability exists in already-installed dependencies. The engineering effort is a measured extension of established patterns already in the codebase.

The recommended integration path for AI providers is: add a `CustomProviderConfig` struct to `settings.rs`, write a `models.json` file to the auth dir when providers are saved, and switch the sidecar's `ModelRegistry.inMemory()` call to `ModelRegistry.create()` which reads that file at startup. Groq and OpenRouter are already built-in `KnownProvider` entries in pi-ai v0.65.0 and require only API key configuration. Ollama and Together AI require the `models.json` custom registration path. The settings scroll refactor is fully independent of the provider changes and can proceed in parallel — it requires replacing conditional rendering in `Settings.tsx` with simultaneous rendering plus `IntersectionObserver`-based active-section tracking, with the critical constraint that the scroll container (not the document root) must be passed as the `root` option or Tauri's overflow model will break anchor navigation.

The primary risks are: (1) Ollama's OpenAI compatibility layer silently drops tool calls when streaming — this must be documented and tested end-to-end before shipping, not just verified with a basic completion prompt; (2) the `set_model` RPC fails silently when a provider is not registered in the sidecar, meaning provider UI changes have no visible effect without the corresponding `entrypoint.ts` registration; and (3) the Tauri WebView scroll model requires explicit container-relative scroll math — `element.scrollIntoView()` against the document root will fail visually. All three risks are well-understood, recoverable, and addressable during implementation.

## Key Findings

### Recommended Stack

No new dependencies are needed. The existing stack (Tauri 2, React 19, Vite 7, Tailwind 3.4, Zustand 5, pi-coding-agent 0.65.0, pi-ai 0.65.0) already contains all needed APIs. `ModelRegistry.registerProvider()` is the correct SDK path for custom provider registration, verified in `dist/core/model-registry.d.ts`. The `IntersectionObserver` browser API handles scroll-spy natively in Tauri's Chromium WebView. Tailwind's `scroll-mt-*` utilities handle section offset.

**Core technologies:**
- `@mariozechner/pi-coding-agent` 0.65.0: `ModelRegistry.create()` + `registerProvider()` — already installed, verified API surface
- `@mariozechner/pi-ai` 0.65.0: `KnownProvider` union (includes `groq`, `openrouter`); `openai-completions` api string for custom endpoints — already installed
- React 19 `useRef` + native `IntersectionObserver`: scroll-spy for active section detection — zero deps, native browser API
- Tailwind `scroll-mt-*`: offset scroll targets past sticky headers — already in use throughout the app

**Critical SDK finding:** `ModelRegistry.inMemory(authStorage)` passes `undefined` as `modelsJsonPath`, bypassing all file-based provider loading. Custom providers require `ModelRegistry.create(authStorage, path)` which reads `models.json` at startup. There is no `register_provider` RPC command — confirmed by exhaustive check of the `RpcCommand` union in `rpc-types.d.ts`.

### Expected Features

**Must have (table stakes):**
- Ollama and LM Studio in provider dropdown with base URL field, no auth required — local model users cannot function without this
- Groq and Together AI with API key field (no OAuth) and correct model ID placeholders — hosted open-source coverage
- Provider-conditional field rendering: base URL for local providers, API key for hosted, nothing extra for either
- Backend settings struct extended to persist `agent_custom_providers` (base URL, model IDs) alongside existing provider/model fields
- Settings page converted to scrollable single page with sidebar anchor navigation

**Should have (competitive):**
- Default base URLs pre-filled for local providers (Ollama: `http://localhost:11434/v1`, LM Studio: `http://localhost:1234/v1`) — reduces friction
- Thinking level selector disabled for providers that don't support it (Groq, Together, Ollama) — prevents 400 errors and user confusion
- Model ID placeholder text per provider showing the correct naming format
- Restart prompt when provider credentials change, since `ModelRegistry` is built at sidecar startup

**Defer (v2+):**
- Dynamic model list fetch from provider endpoints — adds async error state complexity for marginal benefit; static placeholders sufficient for MVP
- Connection test / health check button for local providers — premature; surface errors at runtime instead
- Custom "OpenAI-compatible" provider entry with fully user-defined base URL + API key — useful for proxy/gateway power users; add when demand is confirmed
- Env var chip grouping by category — independent improvement, low urgency

### Architecture Approach

The integration follows a file-contract pattern: the Rust backend owns `models.json` and `settings.json`; the TypeScript sidecar reads `models.json` at startup and picks up new providers on restart. This avoids any new RPC protocol commands and aligns with how pi-coding-agent is designed to be extended. The settings page refactor is a pure frontend concern — `Settings.tsx` retains its OAuth listener (which lives at the page level and must not be disrupted), all section components mount simultaneously, and `IntersectionObserver` with `root: scrollContainerRef.current` drives active-section state.

**Major components:**
1. `settings.rs` + `commands.rs` (Rust backend) — persist `CustomProviderConfig` structs to `settings.json`; write `models.json` to auth dir on save; trigger sidecar restart
2. `agent/entrypoint.ts` (Pi sidecar) — switch from `ModelRegistry.inMemory()` to `ModelRegistry.create()` to load `models.json` at startup
3. `AgentSection.tsx` + `CustomProviderForm.tsx` (React frontend) — provider dropdown expansion, conditional field rendering, custom provider add/edit UI
4. `Settings.tsx` + `SettingsSidebar.tsx` (React frontend) — scroll refactor replacing conditional rendering with simultaneous mount + IntersectionObserver anchor navigation

**Build order (dependency-driven):**
1. `settings.rs` schema — struct shape is the contract everything else depends on
2. `commands.rs` backend — write `models.json`, extend save handler
3. `agent/entrypoint.ts` ModelRegistry switch — testable independently with a manually-placed `models.json`
4. `tauri/agent.ts` bridge types
5. `AgentSection.tsx` provider dropdown + conditional fields
6. `CustomProviderForm.tsx` full custom provider UI
7. Settings scroll refactor — fully parallel; only touches `Settings.tsx` and `SettingsSidebar.tsx`

### Critical Pitfalls

1. **Ollama tool calling silently fails via OpenAI compat streaming** — Set `compat: { supportsDeveloperRole: false, supportsReasoningEffort: false }` in provider config; test with a tool-use prompt (e.g., "list services"), not just a basic completion. Document the limitation in the UI if tool calling remains unreliable after testing.

2. **`set_model` RPC fails silently for unregistered providers** — The `agent.rs` relay skips all RPC responses; provider UI changes appear to succeed but the agent stays on the old model. Fix: register all providers in `entrypoint.ts` before `runRpcMode`, and forward `set_model` failure responses to the frontend as a toast.

3. **Scroll anchor navigation breaks in Tauri's WebView** — Tauri has `overflow: hidden` on the document body; `element.scrollIntoView()` walks to the body and does nothing visible. Use `scrollContainerRef.current.scrollTo({ top: sectionRef.offsetTop, behavior: 'smooth' })` and pass `root: scrollContainerRef.current` to `IntersectionObserver`. Design this before writing any section refs.

4. **Thinking level setting breaks on providers that don't support it** — Sending `reasoning_effort` to Groq/Together/Ollama causes 400 errors or silent degradation. Define a capability map in the frontend and disable the thinking selector when the provider doesn't support it. Set `supportsReasoningEffort: false` in provider compat config on the sidecar side.

5. **Sidecar restart required to apply new provider registration** — `ModelRegistry` is built once at startup; runtime credential changes don't take effect until restart. Acceptable for v1.1 if paired with an explicit restart prompt (extend the existing `hasUnsavedChanges` banner pattern).

## Implications for Roadmap

Based on research, two largely parallel workstreams with a shared schema foundation. Suggested 4-phase structure:

### Phase 1: Settings Schema and Backend Foundation
**Rationale:** Everything — frontend provider UI, sidecar registration, and settings persistence — depends on the `CustomProviderConfig` struct shape. This must come first so downstream work has a stable contract.
**Delivers:** `CustomProviderConfig` + `CustomModelConfig` structs in `settings.rs`; extended `cmd_save_agent_settings` (or new `cmd_save_custom_providers`) in `commands.rs` that writes `models.json` to auth dir; `saveCustomProviders()` invoke wrapper in `tauri/agent.ts`; restart prompt wired into provider save flow.
**Addresses:** Provider persistence, `models.json` file contract
**Avoids:** Pitfall 6 (restart required) — build the restart prompt into the save handler from the start

### Phase 2: Sidecar Provider Registration
**Rationale:** The provider UI is purely cosmetic until the sidecar can resolve the providers. This must be proven working before building the full UI on top of it.
**Delivers:** `agent/entrypoint.ts` switched from `ModelRegistry.inMemory()` to `ModelRegistry.create()`; Groq and OpenRouter confirmed working (already `KnownProvider`); Ollama and Together AI registered via `models.json`; `set_model` failure forwarded as a toast event; Ollama tool calling tested end-to-end.
**Uses:** pi-coding-agent `ModelRegistry.create()`, `ProviderConfigInput` schema, `openai-completions` API string
**Implements:** File-contract integration pattern (Rust writes, TypeScript reads)
**Avoids:** Pitfall 2 (`set_model` silent failure), Pitfall 1 (Ollama tool calling — acceptance-tested here)

### Phase 3: Provider Configuration UI
**Rationale:** Frontend work unblocked once schema (Phase 1) and sidecar registration (Phase 2) are proven. UI can be built knowing persistence and model resolution work end-to-end.
**Delivers:** Extended provider dropdown in `AgentSection.tsx` (Groq, OpenRouter, Ollama, LM Studio); provider-conditional field rendering (base URL vs API key vs no auth); thinking level selector disabled for unsupported providers; `CustomProviderForm.tsx` for add/edit custom providers; correct model ID placeholder text per provider.
**Addresses:** All P1 features from FEATURES.md
**Avoids:** Pitfall 4 (thinking level breaks), Pitfall 5 (Ollama auth confusion)

### Phase 4: Settings Page Scroll Refactor
**Rationale:** Fully independent of Phases 1-3; can be built in parallel after Phase 1 (no schema dependency). Kept as a separate phase because the risk profile is different — a structural UI change with specific Tauri scroll gotchas — and should be reviewed independently.
**Delivers:** `Settings.tsx` converted to simultaneous-section rendering with `IntersectionObserver` scroll-spy; `SettingsSidebar.tsx` sidebar items trigger `scrollTo` on container ref; all sections always mounted; OAuth listener preserved at page level.
**Uses:** Native `IntersectionObserver`, `useRef`, Tailwind `scroll-mt-*`
**Implements:** Scroll-spy with container-relative root (not document root)
**Avoids:** Pitfall 3 (Tauri WebView scroll container height breaks anchor sync)

### Phase Ordering Rationale

- Phase 1 before all others: struct shape determines IPC types, Tauri command signatures, and the `models.json` schema the sidecar reads. No parallel work is stable until this is committed.
- Phase 2 before Phase 3: proving the sidecar resolves a provider before building the UI prevents discovering the integration is broken after the UI is complete.
- Phase 4 parallel with Phases 2-3: it touches only `Settings.tsx` and `SettingsSidebar.tsx` with no shared files — safe to develop concurrently.
- Testing cadence: each phase ends with explicit acceptance tests. Phase 2 specifically must test tool calling (not just basic completion) per Pitfall 1.

### Research Flags

Phases with standard patterns (skip `/gsd-research-phase`):
- **Phase 1 (Schema):** Straightforward Rust struct extension following existing `Settings` patterns — `#[serde(default)]` field addition is well-understood.
- **Phase 4 (Scroll refactor):** IntersectionObserver scroll-spy is a standard web pattern; Tauri-specific container root requirement is fully documented in PITFALLS.md.

Phases that may benefit from targeted research during planning:
- **Phase 2 (Sidecar registration):** The `models.json` schema owned by pi-coding-agent is defined by a third-party library. Verify the exact schema against `loadCustomModels()` in `model-registry.js` before writing the Rust serializer. The note about passing `models: []` wiping the provider catalog warrants direct code verification.
- **Phase 3 (Provider UI):** The thinking level capability map needs to be defined authoritatively. Check pi-ai's `compat` flag documentation to confirm which flags suppress `reasoning_effort` for each provider type before implementing the disable logic.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All findings from direct dist type inspection of installed packages; no inference |
| Features | HIGH | Code inspection + strong ecosystem consensus (AnythingLLM, LibreChat, Open WebUI all converge on same 3-field pattern) |
| Architecture | HIGH | Direct codebase inspection of all files in the change set; build order derived from actual import dependencies |
| Pitfalls | HIGH / MEDIUM | Tauri scroll + Ollama tool calling confirmed by multiple credible sources; Ollama streaming pitfall is an open GitHub issue (not yet resolved as of research date) |

**Overall confidence:** HIGH

### Gaps to Address

- **`models.json` exact schema:** The file format loaded by `loadCustomModels()` is inferred from `ProviderConfigInput` types — validate against the actual `model-registry.js` parsing code before implementing the Rust serializer.
- **Ollama tool calling reliability:** The streaming + tool-calling limitation is confirmed as an open issue but may have been partially addressed in newer Ollama builds. Test against the specific Ollama version users are likely to run before deciding whether to document-and-ship or gate the feature.
- **Together AI base URL:** Confirmed absent from `KnownProvider` and `models.generated.d.ts`, but verify the correct `baseUrl` (`https://api.together.xyz/v1`) against Together's current API docs at implementation time — API base URLs for hosted providers can change.

## Sources

### Primary — HIGH confidence (direct codebase inspection)
- `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/model-registry.d.ts` — `registerProvider()`, `ProviderConfigInput`, `ModelRegistry.inMemory()` vs `ModelRegistry.create()`
- `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/model-registry.js` lines 182, 208 — confirmed `inMemory()` skips models.json
- `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/modes/rpc/rpc-types.d.ts` — `RpcCommand` union exhaustively checked; no `register_provider` command exists
- `/workspace/app/agent/node_modules/@mariozechner/pi-ai/dist/types.d.ts` — `KnownProvider` union; `openai-completions` API string
- `/workspace/app/agent/node_modules/@mariozechner/pi-ai/dist/models.generated.d.ts` — groq, openrouter present; together, ollama absent
- `/workspace/app/agent/entrypoint.ts` — `ModelRegistry.inMemory(authStorage)` confirmed in use
- `/workspace/app/src-tauri/src/agent.rs` — env var injection pattern (`WAVS_AUTH_DIR` etc.)
- `/workspace/packages/gui/shared/src/settings.rs` — current `Settings` struct fields
- `/workspace/app/src-tauri/src/commands.rs` — `cmd_save_agent_settings` pattern
- `/workspace/app/src/components/settings/AgentSection.tsx`, `Settings.tsx`, `SettingsSidebar.tsx` — current tab-switch architecture

### Secondary — MEDIUM confidence (official docs + credible community sources)
- [Ollama OpenAI compat docs](https://docs.ollama.com/api/openai-compatibility) — base URL pattern, placeholder apiKey requirement
- [Groq quickstart](https://console.groq.com/docs/quickstart) — API key auth, model naming conventions
- [AnythingLLM Ollama docs](https://docs.useanything.com/setup/llm-configuration/local/ollama) — base URL pattern, no-auth for local providers
- [LibreChat custom endpoint docs](https://www.librechat.ai/docs/configuration/librechat_yaml/object_structure/custom_endpoint) — base URL patterns, model fetch design
- [Ollama issue #10870](https://github.com/ollama/ollama/issues/10870) — streaming + tool calling limitation (open issue)
- [Tauri discussion #8829](https://github.com/orgs/tauri-apps/discussions/8829), [Tauri issue #6067](https://github.com/tauri-apps/tauri/issues/6067) — WebView overflow and scroll behavior
- [pi-mono custom-provider.md](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/custom-provider.md) — custom provider registration docs
- [thomasledoux.be blog](https://www.thomasledoux.be/blog/highlighting-navigation-items-on-scroll), [DEV Community](https://dev.to/maciekgrzybek/create-section-navigation-with-react-and-intersection-observer-fg0) — IntersectionObserver scroll-spy patterns

---
*Research completed: 2026-04-08*
*Ready for roadmap: yes*
