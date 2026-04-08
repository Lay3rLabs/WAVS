# Requirements: WAVS Improvements

**Defined:** 2026-04-08
**Core Value:** AI agent developers can use WAVS components as MCP tools with the same ease as Wassette, but with cryptographic trust guarantees Wassette structurally cannot provide.

## v1.1 Requirements

Requirements for milestone v1.1: Open Source AI Providers & Settings UX.

### Agent Providers

- [ ] **PROV-01**: User can select Groq as an agent provider from the settings dropdown
- [ ] **PROV-02**: User can select OpenRouter as an agent provider from the settings dropdown
- [ ] **PROV-03**: User can configure API keys for Groq and OpenRouter providers
- [ ] **PROV-04**: User can select Ollama as an agent provider from the settings dropdown
- [ ] **PROV-05**: User can configure a base URL for Ollama (defaults to localhost:11434)
- [ ] **PROV-06**: Agent sidecar loads custom provider config from models.json at startup
- [ ] **PROV-07**: User can use the agent with Ollama-hosted open-source models for WAVS tasks

### Settings Layout

- [ ] **UX-01**: User can scroll through all settings sections on a single page
- [ ] **UX-02**: Sidebar highlights the currently visible section as user scrolls
- [ ] **UX-03**: User can click a sidebar item to scroll to that section

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Agent Providers

- **PROV-08**: User can select Together AI as an agent provider
- **PROV-09**: Thinking level selector is hidden for providers that don't support it
- **PROV-10**: User can add fully custom OpenAI-compatible providers with arbitrary base URLs

### Environment Variables

- **ENV-01**: Environment variable suggestions grouped by category (AI, Blockchain, Storage)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Dynamic model list fetch from providers | Anti-feature per research — adds async failure states for marginal benefit |
| Connection test button in settings | Same as above — runtime errors are sufficient feedback |
| Together AI provider | Deferred to v2 — not a KnownProvider, needs same models.json plumbing as Ollama |
| Env var category grouping | Current flat chip list works fine per user preference |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| PROV-01 | Phase 7 | Pending |
| PROV-02 | Phase 7 | Pending |
| PROV-03 | Phase 7 | Pending |
| PROV-04 | Phase 8 | Pending |
| PROV-05 | Phase 8 | Pending |
| PROV-06 | Phase 8 | Pending |
| PROV-07 | Phase 8 | Pending |
| UX-01 | Phase 9 | Pending |
| UX-02 | Phase 9 | Pending |
| UX-03 | Phase 9 | Pending |

**Coverage:**
- v1.1 requirements: 10 total
- Mapped to phases: 10
- Unmapped: 0

---
*Requirements defined: 2026-04-08*
*Last updated: 2026-04-08 after roadmap creation*
