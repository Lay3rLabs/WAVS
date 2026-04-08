# Feature Research

**Domain:** AI provider configuration UX — open-source/local AI providers in a desktop settings page
**Researched:** 2026-04-08
**Confidence:** HIGH (code inspection + ecosystem research)

## Context: What Already Exists

The existing AgentSection has: provider dropdown (Anthropic/OpenAI/Google), model ID text input, thinking level selector, OAuth + API key auth per provider. EnvironmentSection has WAVS_ENV_* suggestions including WAVS_ENV_OLLAMA_BASE_URL, WAVS_ENV_GROQ_API_KEY, etc. as clickable chips. Settings page uses sidebar tab-switching (not scrollable single page).

New milestone adds: open-source AI providers to the agent provider dropdown with appropriate per-type configuration, improved env var UX, and settings scrollable page layout.

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist when open-source providers are added. Missing these = provider feels broken or unusable.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Base URL field for local providers (Ollama, LM Studio) | Local providers have no fixed URL; users must point to their running instance | LOW | Conditionally shown only when provider requires it. Default `http://localhost:11434` for Ollama, `http://localhost:1234` for LM Studio |
| No auth field for local providers | Ollama and LM Studio run locally with no API key by default | LOW | Hide API key input when provider is `ollama` or `lm-studio`; do not show empty key field |
| API key field for hosted open-source providers | Groq, Together, Mistral require API keys exactly like OpenAI does | LOW | Same pattern as existing Anthropic/OpenAI auth; no OAuth for these providers |
| Provider-appropriate model ID placeholder | Groq/Together have different model name formats than Anthropic | LOW | Update placeholder text per selected provider (e.g., `llama-3.3-70b-versatile` for Groq, `meta-llama/Llama-3-70b-chat-hf` for Together) |
| Providers in dropdown that match env var suggestions | Env section already suggests WAVS_ENV_GROQ_API_KEY etc.; agent section must offer same providers | LOW | Parity between env suggestions and agent provider list avoids confusion |
| Field persistence across section navigation | All provider config fields must survive tab switches | LOW | Existing pattern: save on change via invoke; already done for existing fields |

### Differentiators (Competitive Advantage)

Features that make the provider configuration meaningfully better than the typical "just a text box" pattern.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Provider-conditional field rendering | Show only what's relevant for the selected provider — base URL for local, API key for hosted, nothing extra for local-no-auth | LOW | AgentSection already conditionally renders OAuth vs API key; same pattern extended for local vs hosted |
| Sensible default base URLs pre-filled | Ollama default is `http://localhost:11434`; LM Studio is `http://localhost:1234`; reduces friction for standard installs | LOW | Pre-fill on provider switch if field is empty; do not overwrite user-entered values |
| Model ID guidance per provider | Most users do not know model ID strings by heart; a placeholder showing the correct format reduces errors | LOW | Static placeholder strings per provider; no dynamic fetch needed for MVP |
| Env section chips remain independent | WASM components use env vars separately from agent; chips still serve that purpose even after agent has first-class provider config | LOW | Keep chips as-is; they are not duplicates because they serve a different subsystem |
| Scrollable settings page with anchor navigation | Settings page currently switches between isolated sections; converting to single scrollable page with sidebar anchors means users can see all settings and scroll naturally | MEDIUM | Sidebar items become anchor links; sections rendered continuously; sticky sidebar highlights active section based on scroll position (IntersectionObserver pattern) |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Dynamic model list fetch from provider endpoint | Feels modern; avoids stale model ID placeholders | Ollama `/api/tags` requires a running Ollama; fetch can fail and block the save flow; for hosted providers it requires an API key to be saved first (chicken-and-egg); adds error state complexity | Static placeholder text per provider; model ID is a text field; users can look up model IDs once |
| Connection test button for local providers | Gives confidence the URL is reachable | Adds async error state to a settings form; local provider may be temporarily down but still be a valid config; adds Rust/IPC plumbing for marginal benefit at this scope | Show a note "Ollama must be running at this address when components execute"; defer validation to runtime errors |
| Auto-detect running Ollama on save | Clever DX | Polling adds background work; fails in sandboxed/Tauri security contexts; false negatives confuse users | Trust the user's configured URL; surface errors when agent actually tries to use it |
| OAuth for open-source providers | Consistency with Anthropic/OpenAI | Groq, Together, Mistral, Ollama do not support OAuth flows; adding a broken OAuth button is worse than no button | API key input only for hosted open-source providers; no auth for local providers |
| Global OpenAI-compatible base URL override for all providers | Power-user request for proxy/gateway scenarios | Conflates provider identity with endpoint; breaks per-provider auth logic; confusing when OpenAI and custom endpoint both exist | If needed later: add an explicit "Custom (OpenAI-compatible)" provider entry with base URL + API key fields |

---

## Feature Dependencies

```
[Open-source providers in dropdown]
    └──requires──> [Provider-conditional field rendering]
                       └──requires──> [Base URL field component] (for local providers)
                       └──requires──> [API key field without OAuth] (for hosted open-source)

[Scrollable settings page]
    └──requires──> [Anchor IDs on each section wrapper]
    └──requires──> [Sidebar becomes anchor links instead of state setters]
    └──requires──> [IntersectionObserver for active section highlight]

[Model ID placeholder improvement]
    └──enhances──> [Open-source providers in dropdown]

[Env var UX improvement]
    ──independent of──> [Agent section changes]
    (Env section serves WASM components; agent section serves embedded agent)
```

### Dependency Notes

- **Open-source providers require conditional field rendering:** The existing AgentApiKeyField conditionally renders OAuth vs API key paths. The same gate needs a third path: "no auth" for local providers (Ollama, LM Studio). Base URL field is only needed for local providers. This is purely a frontend concern — the Tauri backend already accepts arbitrary provider strings via `cmd_save_agent_settings`; a `base_url` field needs to be added to that settings struct.
- **Scrollable page is independent of provider changes:** Can be implemented in its own phase. Settings.tsx currently uses `activeSection` state and renders one section at a time behind conditionals. Converting to scroll-based layout means removing the conditional rendering, adding `id` attributes to section containers, and changing SettingsSidebar to use anchor hrefs with `scrollIntoView`. The OAuth listener already lives in the parent Settings.tsx and survives unmount — no regression risk.
- **Env var UX improvement is independent:** EnvironmentSection already has suggestion chips. Improvements such as grouping chips by category (AI providers / Storage / Blockchain) or adding edit-in-place for existing values do not depend on agent section changes.

---

## MVP Definition

### Launch With (v1.1)

Minimum needed to satisfy the milestone goal.

- [ ] Ollama and LM Studio added to provider dropdown with base URL field, no auth required — users running local models can configure the agent
- [ ] Groq and Together AI added with API key field (no OAuth), correct model ID placeholder — hosted open-source providers covered
- [ ] Provider-conditional field rendering: base URL shown for local, API key shown for hosted, nothing extra for either
- [ ] Settings page converted to scrollable single page with sidebar anchor navigation — sidebar items scroll-to-section rather than tab-switch
- [ ] Env var section chips remain unchanged — no regression on WASM component env var workflow
- [ ] Backend settings struct extended to persist `agent_base_url` alongside existing provider/model/thinking fields

### Add After Validation (v1.x)

- [ ] Mistral and OpenRouter as additional hosted providers — same pattern as Groq, add when user demand is confirmed
- [ ] Custom (OpenAI-compatible) provider entry with user-configurable base URL + API key — for proxy scenarios and future providers
- [ ] Env var chips grouped by category (AI / Storage / Chain) — reduces visual noise when suggestion list grows

### Future Consideration (v2+)

- [ ] Dynamic model list fetch from provider endpoint — only worthwhile if model IDs become unstable or too numerous to document
- [ ] Connection health indicator for local providers — useful signal at scale, premature now

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Ollama/LM Studio local provider support | HIGH | LOW | P1 |
| Groq/Together hosted provider support | HIGH | LOW | P1 |
| Provider-conditional field rendering | HIGH | LOW | P1 |
| Default base URL pre-fill for local providers | MEDIUM | LOW | P1 |
| Model ID placeholder per provider | MEDIUM | LOW | P1 |
| Backend settings struct for base_url | HIGH | LOW | P1 |
| Scrollable settings page with anchor nav | MEDIUM | MEDIUM | P1 |
| Env var chip grouping by category | LOW | LOW | P2 |
| Custom OpenAI-compatible provider entry | MEDIUM | MEDIUM | P2 |
| Mistral / OpenRouter providers | LOW | LOW | P2 |
| Dynamic model fetch | LOW | HIGH | P3 |
| Connection test button | LOW | HIGH | P3 |

---

## Ecosystem Patterns Observed

**How AnythingLLM handles local providers:** Shows base URL input (defaults to `http://127.0.0.1:11434`), then model selection. No auth fields. Explicit note that Ollama must be running separately. Model selection happens after connecting.

**How LibreChat handles provider switching:** Provider dropdown, optional base URL override, API key, model ID text input. No connection test. Dynamic model fetch is optional and gated on a `fetch: true` config flag — not shown by default.

**How Open WebUI handles it:** Any OpenAI-compatible endpoint via base URL + optional API key. Model list populated from `/v1/models` if endpoint supports it. Falls back gracefully if not. LM Studio integrates the same way as Ollama — just a different port.

**Standard OpenAI-compatible endpoint pattern (industry consensus):** Three fields: `base_url`, `api_key` (may be empty/dummy for local), `model`. Local providers (Ollama): base URL only, model by name, no key. Hosted open-source (Groq, Together, Mistral): base URL is fixed/well-known, API key required, model by name. The right UX exposes only what varies per provider.

**Settings scroll pattern:** Native `scrollIntoView({ behavior: 'smooth' })` + IntersectionObserver for active section highlight. The current codebase uses no scroll library. The react-scroll npm package is an option but adds a dependency for functionality achievable with a ~20-line custom hook. Prefer native browser APIs.

**Thinking level selector for open-source providers:** Anthropic and OpenAI have thinking/reasoning modes. Ollama, Groq, and Together do not expose a thinking level parameter. The thinking level dropdown should either be hidden for providers that do not support it, or shown with a note that it has no effect. Hiding is cleaner.

---

## Sources

- Code inspection: `/workspace/app/src/components/settings/AgentSection.tsx`, `EnvironmentSection.tsx`, `SettingsSidebar.tsx`, `Settings.tsx`, `app/src/tauri/agent.ts`
- [AnythingLLM Ollama configuration docs](https://docs.useanything.com/setup/llm-configuration/local/ollama) — base URL pattern, no-auth for local
- [LibreChat custom endpoint object structure](https://www.librechat.ai/docs/configuration/librechat_yaml/object_structure/custom_endpoint) — model fetch, base URL patterns
- [Open WebUI docs](https://docs.openwebui.com/) — OpenAI-compatible endpoint pattern, LM Studio integration
- [Groq quickstart](https://console.groq.com/docs/quickstart) — GROQ_API_KEY, no OAuth, model naming conventions
- [Infralovers: Ollama 2025 updates](https://www.infralovers.com/blog/2025-08-13-ollama-2025-updates/) — Ollama native desktop app, standard local port 11434
- [Sailing Byte: Free LLM Desktop Tools comparison 2025](https://sailingbyte.com/blog/the-ultimate-comparison-of-free-desktop-tools-for-running-local-llms/) — UX comparison across AnythingLLM, LM Studio, Open WebUI
- [react-scroll npm](https://www.npmjs.com/package/react-scroll) — anchor nav pattern for React (evaluated and deprioritized in favor of native APIs)

---
*Feature research for: WAVS v1.1 open-source AI provider settings UX*
*Researched: 2026-04-08*
