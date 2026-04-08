# Roadmap: WAVS Improvements

## Milestones

- ✅ **v1.0 WAVS Improvements** — Phases 1-6 (shipped 2026-04-07)
- 🚧 **v1.1 Open Source AI Providers & Settings UX** — Phases 7-9 (in progress)

## Phases

<details>
<summary>✅ v1.0 WAVS Improvements (Phases 1-6) — SHIPPED 2026-04-07</summary>

- [x] Phase 1: OCI Component Pull (2/2 plans) — completed 2026-03-24
- [x] Phase 2: WIT-to-Schema Tooling (2/2 plans) — completed 2026-03-25
- [x] Phase 3: MCP Execution Interface (3/3 plans) — completed 2026-03-25
- [x] Phase 4: Rust Event Foundation (1/1 plan) — completed 2026-04-07
- [x] Phase 5: Settings Decomposition (2/2 plans) — completed 2026-04-07
- [x] Phase 6: Unified Activity Frontend (2/2 plans) — completed 2026-04-07

Full details: `.planning/milestones/v1.0-ROADMAP.md`

</details>

### 🚧 v1.1 Open Source AI Providers & Settings UX (In Progress)

**Milestone Goal:** Let users configure open-source AI models (Groq, OpenRouter, Ollama) as agent providers, with proper persistence through the Rust backend and sidecar, and convert the settings page to a scrollable single-page layout with sidebar anchor navigation.

- [ ] **Phase 7: Groq & OpenRouter Providers** - Backend schema + Groq/OpenRouter working end-to-end
- [ ] **Phase 8: Ollama Provider** - models.json plumbing, sidecar switch, Ollama working end-to-end
- [ ] **Phase 9: Settings Scroll Refactor** - Single-page scrollable settings with sidebar anchor navigation

## Phase Details

### Phase 7: Groq & OpenRouter Providers
**Goal**: Users can select and configure Groq and OpenRouter as agent providers, with credentials persisted and the agent using those providers immediately after a restart
**Depends on**: Phase 6 (v1.0 complete — settings decomposition exists)
**Requirements**: PROV-01, PROV-02, PROV-03
**Success Criteria** (what must be TRUE):
  1. User can select Groq from the provider dropdown in agent settings
  2. User can select OpenRouter from the provider dropdown in agent settings
  3. User can enter and save an API key for Groq and OpenRouter; credentials persist across app restarts
  4. After saving and restarting, the agent sidecar uses the selected provider for responses
**Plans**: 1 plan
Plans:
- [x] 07-01-PLAN.md — Add Groq & OpenRouter providers to UI dropdown and sidecar startup
**UI hint**: yes

### Phase 8: Ollama Provider
**Goal**: Users can configure Ollama as an agent provider with a custom base URL, and the agent works end-to-end with locally-hosted open-source models including tool-calling tasks
**Depends on**: Phase 7
**Requirements**: PROV-04, PROV-05, PROV-06, PROV-07
**Success Criteria** (what must be TRUE):
  1. User can select Ollama from the provider dropdown in agent settings
  2. User can set a base URL for Ollama (pre-filled with localhost:11434); the field appears only when Ollama is selected
  3. After saving and restarting, the agent sidecar loads Ollama from models.json and resolves the provider correctly
  4. User can complete a WAVS task (e.g., "list services") using an Ollama-hosted model — tool calling works end-to-end
**Plans**: 1 plan
Plans:
- [x] 07-01-PLAN.md — Add Groq & OpenRouter providers to UI dropdown and sidecar startup
**UI hint**: yes

### Phase 9: Settings Scroll Refactor
**Goal**: Users can navigate all settings sections on a single scrollable page with the sidebar tracking position and supporting click-to-scroll
**Depends on**: Phase 6 (v1.0 complete — Settings.tsx decomposed into sections)
**Requirements**: UX-01, UX-02, UX-03
**Success Criteria** (what must be TRUE):
  1. User can scroll through all settings sections without switching tabs or triggering navigation
  2. The sidebar highlights the section currently visible in the viewport as the user scrolls
  3. User can click any sidebar item and the page smoothly scrolls to that section
  4. OAuth listener and other page-level state survive scrolling and sidebar navigation without unmounting
**Plans**: 1 plan
Plans:
- [ ] 07-01-PLAN.md — Add Groq & OpenRouter providers to UI dropdown and sidecar startup
**UI hint**: yes

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. OCI Component Pull | v1.0 | 2/2 | Complete | 2026-03-24 |
| 2. WIT-to-Schema Tooling | v1.0 | 2/2 | Complete | 2026-03-25 |
| 3. MCP Execution Interface | v1.0 | 3/3 | Complete | 2026-03-25 |
| 4. Rust Event Foundation | v1.0 | 1/1 | Complete | 2026-04-07 |
| 5. Settings Decomposition | v1.0 | 2/2 | Complete | 2026-04-07 |
| 6. Unified Activity Frontend | v1.0 | 2/2 | Complete | 2026-04-07 |
| 7. Groq & OpenRouter Providers | v1.1 | 0/1 | In progress | - |
| 8. Ollama Provider | v1.1 | 0/? | Not started | - |
| 9. Settings Scroll Refactor | v1.1 | 0/? | Not started | - |
