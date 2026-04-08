# Pitfalls Research

**Domain:** Adding open-source AI provider support and settings UX improvements to existing Tauri 2 + React desktop app with pi-coding-agent sidecar
**Researched:** 2026-04-08
**Confidence:** HIGH (codebase-verified + official docs) / MEDIUM (official docs only) / MEDIUM-LOW (multiple credible sources)

---

## Critical Pitfalls

### Pitfall 1: OpenAI-Compatible Endpoint Drops Tool Calls When Streaming

**What goes wrong:**
Ollama's `/v1/chat/completions` compatibility layer silently drops tool calls when streaming is enabled. The model decides to call a tool, the streaming response returns empty content with `finish_reason: "stop"`, and the tool call is lost entirely. The agent appears to just stop responding with no error. This is a known open issue in Ollama's OpenAI compat layer (as of 2025).

**Why it happens:**
The OpenAI compatibility layer is an overlay on Ollama's native protocol. Its streaming path doesn't correctly serialize tool use payloads. The pi-ai SDK uses `openai-completions` as the API type for all OpenAI-compatible endpoints, which means it goes through the same streaming + tool-calling path that breaks on Ollama.

**How to avoid:**
When adding Ollama as a provider, set `compat` flags in the provider config to disable features that Ollama's OpenAI compat layer doesn't handle correctly. The pi SDK's `CompatOptions` includes `supportsDeveloperRole: false` and `supportsReasoningEffort: false` — verify these are set. However, for Ollama specifically, tool calling via the OpenAI compat path may be fundamentally unreliable; document this limitation clearly in the UI and test end-to-end with a tool-using prompt before shipping.

**Warning signs:**
- Agent returns empty response after receiving a user message with a clear tool-use trigger
- No error in Tauri logs, no `extension_error` event emitted
- Setting a non-Ollama provider makes the same prompt work

**Phase to address:**
Provider configuration phase — test with a tool-using prompt as part of acceptance criteria for each new provider, not just a basic completion prompt.

---

### Pitfall 2: `set_model` RPC Fails Silently for Unknown Providers

**What goes wrong:**
The current `cmd_agent_set_model` sends `{"type": "set_model", "provider": provider, "modelId": model_id}` to the sidecar. If the sidecar's `ModelRegistry` does not have the provider registered (e.g., "groq", "together", "ollama"), the RPC response has `success: false` — but the Rust relay in `agent.rs` only logs responses and does not forward failures to the frontend. The UI shows no indication that the model change failed. The agent continues using its previously configured model.

**Why it happens:**
The relay logic in `agent.rs` skips all RPC responses: `if json.get("type")... == Some("response") { ... continue; }`. This was intentional for most responses but means `set_model` failures are invisible. Custom providers must be registered in `entrypoint.ts` before `set_model` can switch to them.

**How to avoid:**
Two-pronged fix: (1) Register custom providers in `entrypoint.ts` via `modelRegistry.registerProvider(...)` for each new provider before `runRpcMode` is called. (2) Forward `set_model` response failures to the frontend as a status event so the user knows the switch failed.

**Warning signs:**
- Selecting a new provider in the UI appears to succeed but the agent still attributes responses to the old provider in `modelInfo`
- `message_end` events show `provider: "anthropic"` even after switching to "ollama"

**Phase to address:**
Provider registration in sidecar entrypoint — this is the first thing to implement. Without it, all provider UI is cosmetic.

---

### Pitfall 3: Settings Scroll Container Height Breaks Sidebar Anchor Synchronization

**What goes wrong:**
The current Settings page has an explicit `max-h-[calc(100vh-12rem)] overflow-y-auto` on the content div, making that element the scroll container — not the window. When converting to a scrollable single-page layout with sidebar anchors, `scrollIntoView()` on section refs and `IntersectionObserver` both work against the document root by default. Anchors scroll the outer window (which may be overflow:hidden in Tauri), not the inner container, so nothing visually scrolls.

**Why it happens:**
Tauri's WebView has `overflow: hidden` on the document body in many configurations to prevent elastic scrolling on macOS. The real scroll container is always a child div with bounded height. `element.scrollIntoView()` walks up to the nearest scrollable ancestor — if the content div gets `overflow: visible` (the default after removing `overflow-y-auto`), the call propagates past it to the immovable body.

**How to avoid:**
Keep one explicit scroll container div with `overflow-y-auto`. When clicking a sidebar item, call `scrollContainer.scrollTo({ top: sectionRef.offsetTop, behavior: 'smooth' })` using a ref to the container, not `element.scrollIntoView()`. For `IntersectionObserver` to detect visible sections correctly, pass `root: scrollContainerRef.current` in the observer options — otherwise the viewport root is used and all sections appear "visible" because the container itself is visible.

**Warning signs:**
- Clicking a sidebar anchor does nothing visible even though no JS error appears
- All sidebar items highlight simultaneously (IntersectionObserver using window root)
- Clicking anchor briefly scrolls then snaps back (body elastic scroll fighting with explicit scroll container)

**Phase to address:**
Settings UX conversion phase — design the scroll architecture before writing any section refs or anchor click handlers.

---

### Pitfall 4: Thinking Level Setting Breaks on Providers That Don't Support It

**What goes wrong:**
The settings UI lets users select "low/medium/high" thinking for the agent. When the provider is Groq, Together, Ollama, or most non-Anthropic providers, the model does not support extended thinking / reasoning effort. The pi SDK sends `reasoning_effort` or budget tokens in the request, the API returns a 400 or silently ignores it, and the agent either fails hard or produces lower quality output without explanation.

**Why it happens:**
The `agent_thinking_level` setting is stored globally and applied to all providers. The `set_thinking_level` RPC is sent to the sidecar regardless of the current provider. The pi SDK's `compat` flags (`supportsReasoningEffort: false`) would prevent this but only if set per-provider — custom providers registered via `registerProvider` need explicit compat flags.

**How to avoid:**
When displaying the thinking level selector in `AgentSection`, disable it (greyed out with a tooltip) when the selected provider does not support thinking. Define a provider capability map in the frontend: `{ anthropic: true, openai: ['o1', 'o3'], google: true, groq: false, together: false, ollama: false }`. On the sidecar side, set `compat.supportsReasoningEffort: false` for all OpenAI-compatible custom providers.

**Warning signs:**
- 400 errors from Groq/Together API appearing in Tauri logs after switching provider and keeping thinking enabled
- Agent stops mid-task silently after switching to a local model

**Phase to address:**
Provider capability modeling — implement this before exposing the thinking control for open-source providers.

---

### Pitfall 5: API Key Auth vs. Base URL Auth Conflation for Ollama

**What goes wrong:**
Ollama does not require an API key. The `AgentApiKeyField` component in `AgentSection.tsx` will show an empty "not configured" state for Ollama because `agentGetAuth()` returns nothing for a provider with no key. The user sees a key input field and tries to enter something, which may break authentication (Ollama ignores the key, but pi-ai may pass it as `Authorization: Bearer <value>` and some Ollama builds reject non-empty auth headers).

**Why it happens:**
The current auth model assumes every provider needs either OAuth or an API key. The `OAUTH_PROVIDERS` set controls OAuth display, and anything outside it shows the API key input. Ollama and LM Studio are neither — they're base-URL-only providers.

**How to avoid:**
Add a provider type classification: `'api-key'`, `'oauth'`, `'base-url-only'`. For base-URL-only providers, replace the auth field with a base URL input field. Store the base URL in `agent_base_url` (new settings field) or use the existing `env_vars` store under a key like `WAVS_AGENT_OLLAMA_BASE_URL`. Pass it to the sidecar via a new environment variable at startup.

**Warning signs:**
- User enters a dummy key for Ollama and the agent fails to connect
- `agentGetAuth()` returns empty for "ollama" and the UI shows "not configured" permanently

**Phase to address:**
Provider type system — define this before building the Ollama/LM Studio configuration UI.

---

### Pitfall 6: Sidecar Restart Required to Apply New Provider Registration

**What goes wrong:**
Custom providers are registered in `entrypoint.ts` which runs once at sidecar startup. If a user adds a new provider (e.g., enters a Together API key or an Ollama base URL) while the agent is running, the provider is not available to `set_model` until the sidecar restarts. But the current UX has no automatic restart — the user switches provider in the dropdown, nothing happens, and there's no error.

**Why it happens:**
`ModelRegistry.registerProvider()` is called at initialization time in `entrypoint.ts`. The RPC protocol does not have a "register provider" command — only `set_model`. So runtime provider addition requires either re-reading config from disk on each `set_model` call, or restarting the sidecar.

**How to avoid:**
Design the sidecar to re-read provider config on every `set_model` RPC, OR trigger a sidecar restart when provider credentials change (same pattern as the "restart for changes to take effect" banner already in `Settings.tsx`). The restart banner approach is lower risk — extend `hasUnsavedChanges` to also track provider credential changes and prompt restart.

**Warning signs:**
- Entering a new API key and immediately switching provider does not work without restart
- Tests pass when provider is configured before sidecar starts but fail when configured at runtime

**Phase to address:**
Settings save flow — address this when implementing the provider credential save handlers.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Storing custom provider base URLs in `env_vars` instead of a typed settings field | No schema migration needed | Env vars and agent settings become conflated; hard to validate URLs | Only for MVP; add typed field before v1.2 |
| Always showing the thinking level selector regardless of provider | Simple UI, no capability map needed | 400 errors for unsupported providers; confusing UX | Never — capability check must ship with the feature |
| Using `element.scrollIntoView()` for anchor navigation without a container ref | Fewer lines of code | Breaks in Tauri's constrained WebView scroll model | Never — the container-ref approach is only marginally more code |
| Registering providers only at sidecar startup | Simple implementation | Runtime credential changes require restart; non-obvious UX | Acceptable for v1.1 if paired with restart prompt |
| Listing Ollama in the provider dropdown without testing tool calling | Covers the use case nominally | Users discover the limitation through agent failure, not documentation | Never — document limitation or don't ship the option |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| pi-coding-agent `ModelRegistry.inMemory()` | Assuming `inMemory` means "no file needed" — it just means no models.json is loaded, but `AuthStorage` still persists credentials to disk | Always pass `authStorage` pointing to the correct `authDir`; `ModelRegistry.inMemory()` is the right choice for WAVS's controlled provider set |
| pi-coding-agent `registerProvider` | Passing `models: []` when only overriding `baseUrl` — this replaces all existing models for that provider with an empty list | Omit `models` entirely when only setting `baseUrl` or `headers` |
| pi-ai `getEnvApiKey` | Assuming Groq reads `GROQ_API_KEY` from the process environment automatically | It does, but only for the `"groq"` provider string — custom providers with custom names need explicit `apiKey` field in `registerProvider` |
| Ollama `/v1/chat/completions` | Using default streaming with tool-capable models | Set `compat: { supportsDeveloperRole: false, supportsReasoningEffort: false }` at minimum; test tool calling explicitly |
| Tauri sidecar env passthrough | Adding a new env var to sidecar config in `commands.rs` but forgetting to add it to the `cmd.env(...)` chain in `agent.rs` `PiSidecarConfig` | Both `PiSidecarConfig` struct and the `Command::new("npx")` builder in `start()` must be updated together |
| `agentGetAuth` / `agentSetApiKey` for new providers | The auth commands delegate to the pi sidecar — if the sidecar doesn't recognize the provider string, it stores the credential but can't use it | Provider strings in the frontend `<select>` must exactly match provider names registered via `registerProvider` in `entrypoint.ts` |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| IntersectionObserver on multiple sections without `root` | All sections simultaneously marked active as user scrolls | Always pass `root: scrollContainerRef.current` | Immediately — even with 2 sections |
| Re-creating `IntersectionObserver` on every render | Scroll position detection flickers; console shows repeated observe/unobserve | Create observer in `useEffect` with stable `[]` or `[sectionRefs]` dep array | Any re-render that changes props passed to the settings component |
| Calling `agentGetAuth()` on every provider select change | Network delay on each keystroke if auth check is slow | Call once on mount and on explicit "save" — not on every dropdown onChange | Not a performance problem today, but noisy in logs |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Logging API keys in Tauri tracing output | Keys appear in crash logs and telemetry | Never log `PiSidecarConfig` fields that contain credentials; audit `tracing::info!` calls in `agent.rs` |
| Storing Ollama base URL without validation | User inputs `javascript:` or file-system path; sidecar makes unexpected requests | Validate base URL format (must start with `http://` or `https://`) before passing to sidecar |
| Passing `WAVS_AGENT_BASE_URL` env var to sidecar without sanitization | Path traversal or SSRF if user controls the URL | Sanitize and validate in the Rust command handler before passing to `cmd.env()` |
| Showing raw API key in debug panel | Key exposed in screenshots | `AgentApiKeyField` already masks — ensure new provider credential fields follow the same masked-display pattern |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Showing "not configured" auth state for Ollama (which needs no key) | User is confused; enters dummy key; may break things | Detect base-URL-only providers and show a base URL field instead of an auth field |
| Provider dropdown changes apply without restart, but base URL changes don't | Inconsistent — some changes require restart, some don't | Unify: any provider config change sets `hasUnsavedChanges` and requires explicit restart |
| Thinking level selector visible and active for Groq/Together | User sets "high thinking", pays for it in latency/errors | Grey out and tooltip "Not supported by this provider" when provider doesn't support thinking |
| Sidebar anchor highlights wrong section when near boundaries | User loses orientation | Use `threshold: [0, 0.25]` with `rootMargin` to fire early enough for section highlight to lead the scroll |
| No feedback when `set_model` fails (provider not registered) | Agent silently continues on old model | Forward `set_model` failure responses to frontend as a toast error |

---

## "Looks Done But Isn't" Checklist

- [ ] **Provider dropdown:** Lists "Ollama" — verify `registerProvider` call exists in `entrypoint.ts` for it, not just a UI option with no backend registration
- [ ] **Ollama tool calling:** Test with a prompt that forces tool use (e.g., "list services") — not just a simple "hello" completion
- [ ] **Thinking level for new providers:** Check that selecting Groq/Together with thinking="high" does not cause 400 errors in Tauri logs
- [ ] **Sidecar restart after credential change:** Enter a new API key for a provider, switch to it without restarting — confirm whether it works or fails, and ensure failure is communicated
- [ ] **Scroll anchor navigation:** Verify clicking each sidebar item scrolls to the correct section in a Tauri build (not just browser dev), including the last section which may not reach the top of the viewport
- [ ] **IntersectionObserver root:** Confirm the active sidebar item updates correctly while scrolling — test all sections, not just the first two
- [ ] **Env var passthrough:** Add a new env var to the sidecar config, verify it appears in `process.env` inside `entrypoint.ts` by logging it at startup
- [ ] **Settings persistence:** After adding an Ollama base URL and restarting, confirm the URL is restored from `settings.json`

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Ollama tool calling broken in release | MEDIUM | Add provider warning banner in UI; document limitation in release notes; gate Ollama on non-tool-intensive tasks |
| `set_model` silent failure shipped | LOW | Add response forwarding in the relay (one-line fix in `agent.rs`); patch release |
| Scroll anchor using wrong container | LOW | Fix `scrollIntoView` → `scrollContainer.scrollTo`; pure frontend fix, no Rust changes |
| Provider registered in UI but not entrypoint | LOW | Add `registerProvider` call in `entrypoint.ts`; rebuild and replace sidecar bundle |
| Settings field `agent_base_url` not added to Rust struct | MEDIUM | Add field to `settings.rs`, add `#[serde(default)]`, bump settings; existing settings.json files will use default (empty) gracefully |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Tool calling drops on Ollama OpenAI compat | Provider registration & testing phase | Run "list services" prompt with Ollama selected; check for tool_execution_start events |
| `set_model` silent failure | Provider integration phase | Switch provider while sidecar running; confirm modelInfo updates or toast error appears |
| Scroll container height & anchor sync | Settings UX conversion phase | Click each sidebar anchor in Tauri build; confirm scroll and highlight |
| Thinking level breaks on unsupported providers | Provider capability modeling phase | Select Groq + thinking=high; confirm no 400 errors in Tauri logs |
| Ollama base-URL-only auth confusion | Provider type system phase | Open agent settings with Ollama selected; confirm no API key field shown |
| Sidecar restart required for new credentials | Settings save flow phase | Save new API key without restart; confirm restart prompt shown or agent switches cleanly |
| `registerProvider` with `models: []` wipes catalog | Provider registration implementation | After adding custom provider, verify built-in models for that provider still available |

---

## Sources

- pi-coding-agent ModelRegistry API: `/workspace/app/agent/node_modules/@mariozechner/pi-coding-agent/dist/core/model-registry.d.ts` (codebase, HIGH confidence)
- pi-ai env-api-keys provider map: `/workspace/app/agent/node_modules/@mariozechner/pi-ai/dist/env-api-keys.js` (codebase, HIGH confidence)
- pi-coding-agent changelog breaking changes: [pi-mono CHANGELOG](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/CHANGELOG.md) (MEDIUM confidence)
- pi-coding-agent models.md (compat flags, custom providers): [models.md](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/models.md) (MEDIUM confidence)
- pi-coding-agent custom-provider.md: [custom-provider.md](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/custom-provider.md) (MEDIUM confidence)
- Ollama OpenAI compat streaming + tool calling: [Ollama issue #10870](https://github.com/ollama/ollama/issues/10870) (MEDIUM confidence — open issue)
- Tauri scrollbar and overflow behavior: [Tauri discussion #8829](https://github.com/orgs/tauri-apps/discussions/8829), [Tauri issue #6067](https://github.com/tauri-apps/tauri/issues/6067) (MEDIUM confidence)
- IntersectionObserver root pitfall: [thomasledoux.be blog](https://www.thomasledoux.be/blog/highlighting-navigation-items-on-scroll), [DEV Community](https://dev.to/maciekgrzybek/create-section-navigation-with-react-and-intersection-observer-fg0) (MEDIUM confidence)
- Existing codebase: `agent.rs`, `AgentSection.tsx`, `Settings.tsx`, `SettingsSidebar.tsx`, `entrypoint.ts`, `settings.rs` (HIGH confidence)

---
*Pitfalls research for: open-source AI provider support and settings UX improvements in Tauri 2 + React + pi-coding-agent sidecar*
*Researched: 2026-04-08*
