# Roadmap: WAVS Improvements

## Milestones

- ✅ **v1.0 WAVS Improvements** — Phases 1-6 (shipped 2026-04-07)
- ✅ **v1.1 Open Source AI Providers & Settings UX** — Phases 7-9 (shipped 2026-04-08)
- 🚧 **v1.2 Components Explorer** — Phases 10-12 (in progress)

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

<details>
<summary>✅ v1.1 Open Source AI Providers & Settings UX (Phases 7-9) — SHIPPED 2026-04-08</summary>

- [x] Phase 7: Groq & OpenRouter Providers (1/1 plan) — completed 2026-04-08
- [x] Phase 8: Ollama Provider (1/1 plan) — completed 2026-04-08
- [x] Phase 9: Settings Scroll Refactor (1/1 plan) — completed 2026-04-08

Full details: `.planning/milestones/v1.1-ROADMAP.md`

</details>

### 🚧 v1.2 Components Explorer (In Progress)

**Milestone Goal:** Surface component interfaces, schemas, and metadata through an improved components list and a new component detail page.

- [x] **Phase 10: Backend Commands** - Tauri commands exposing wit-schema JSON Schema and component metadata to the frontend (completed 2026-04-08)
- [ ] **Phase 11: Component Detail Page** - New detail page surfacing a component's full interface profile, permissions, limits, and configuration
- [ ] **Phase 12: Components List Page** - Improved list page with richer cards, search, filter, and navigation to detail

## Phase Details

### Phase 10: Backend Commands
**Goal**: The frontend can retrieve full interface schema and metadata for any component via Tauri commands
**Depends on**: Nothing (first phase of v1.2)
**Requirements**: BACK-01, BACK-02
**Success Criteria** (what must be TRUE):
  1. Calling the Tauri command with a component digest returns a JSON Schema object covering all exported functions with their input/output types and doc comments
  2. Calling the Tauri command with a component digest returns a metadata object covering permissions, resource limits, config keys, and required env vars
  3. Both commands complete without error for components sourced via Registry, Download, and OCI digest
**Plans**: 1 plan
Plans:
- [x] 10-01-PLAN.md — Wire wit-schema and component metadata Tauri commands

### Phase 11: Component Detail Page
**Goal**: Users can navigate to a per-component detail page and read everything about its interface, permissions, and configuration
**Depends on**: Phase 10
**Requirements**: DETL-01, DETL-02, DETL-03, DETL-04, DETL-05, DETL-06
**Success Criteria** (what must be TRUE):
  1. User can navigate to `/components/:digest` and see a dedicated detail page for that component
  2. User can see the component's source info, digest, OCI URI (if applicable), and which services currently use it
  3. User can see all exported functions listed, and can expand each to view its input and output JSON Schema
  4. User can see the component's permission profile — HTTP hosts, file system access, sockets, and DNS resolution settings
  5. User can see resource limits (fuel limit, time limit) and the config keys and env vars the component expects
**Plans**: TBD
**UI hint**: yes

### Phase 12: Components List Page
**Goal**: Users can find components quickly through richer cards, search, and source-type filtering, and can reach a component's detail page in one click
**Depends on**: Phase 11
**Requirements**: LIST-01, LIST-02, LIST-03, LIST-04
**Success Criteria** (what must be TRUE):
  1. Each component card on the list page shows function count, a source-type badge, and a permissions summary
  2. User can type in a search box and the list filters to components matching by name or digest
  3. User can select a source-type filter (Registry / Download / Digest) and see only components of that type
  4. User can click a component card and land on that component's detail page
**Plans**: TBD
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
| 7. Groq & OpenRouter Providers | v1.1 | 1/1 | Complete | 2026-04-08 |
| 8. Ollama Provider | v1.1 | 1/1 | Complete | 2026-04-08 |
| 9. Settings Scroll Refactor | v1.1 | 1/1 | Complete | 2026-04-08 |
| 10. Backend Commands | v1.2 | 1/1 | Complete    | 2026-04-08 |
| 11. Component Detail Page | v1.2 | 0/? | Not started | - |
| 12. Components List Page | v1.2 | 0/? | Not started | - |
