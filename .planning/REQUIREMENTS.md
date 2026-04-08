# Requirements: WAVS Improvements

**Defined:** 2026-04-08
**Core Value:** AI agent developers can use WAVS components as MCP tools with the same ease as Wassette, but with cryptographic trust guarantees Wassette structurally cannot provide.

## v1.2 Requirements

Requirements for Components Explorer milestone. Each maps to roadmap phases.

### Backend

- [ ] **BACK-01**: User can retrieve JSON Schema (exported functions, input/output types, doc comments) for a component via Tauri command
- [ ] **BACK-02**: User can retrieve component metadata (permissions, resource limits, config keys, env vars) via Tauri command

### Detail Page

- [ ] **DETL-01**: User can navigate to a component detail page at `/components/:digest`
- [ ] **DETL-02**: User can see component identity — source info, digest, OCI URI, and which services use it
- [ ] **DETL-03**: User can see exported functions listed with expandable input/output JSON Schema viewers
- [ ] **DETL-04**: User can see component permissions (HTTP hosts, file system, sockets, DNS resolution)
- [ ] **DETL-05**: User can see component resource limits (fuel limit, time limit)
- [ ] **DETL-06**: User can see component config keys and required environment variables

### List Page

- [ ] **LIST-01**: User can see richer component cards showing function count, source type badge, and permissions summary
- [ ] **LIST-02**: User can search components by name or digest
- [ ] **LIST-03**: User can filter components by source type (Registry/Download/Digest)
- [ ] **LIST-04**: User can click a component card to navigate to its detail page

## Future Requirements

### Component Interaction

- **INTR-01**: User can test-invoke a component function from the detail page
- **INTR-02**: User can view execution history for a component

### Schema Visualization

- **SCHM-01**: User can see a visual type graph of component interfaces
- **SCHM-02**: User can export component schema as standalone JSON file

## Out of Scope

| Feature | Reason |
|---------|--------|
| Component publishing/upload from UI | OCI publish tooling deferred; pull-only shipped in v1.0 |
| Component comparison/diffing | Nice-to-have but not core to explorer |
| Schema editing | Components are immutable artifacts; editing schemas has no effect |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| BACK-01 | — | Pending |
| BACK-02 | — | Pending |
| DETL-01 | — | Pending |
| DETL-02 | — | Pending |
| DETL-03 | — | Pending |
| DETL-04 | — | Pending |
| DETL-05 | — | Pending |
| DETL-06 | — | Pending |
| LIST-01 | — | Pending |
| LIST-02 | — | Pending |
| LIST-03 | — | Pending |
| LIST-04 | — | Pending |

**Coverage:**
- v1.2 requirements: 12 total
- Mapped to phases: 0
- Unmapped: 12 ⚠️

---
*Requirements defined: 2026-04-08*
*Last updated: 2026-04-08 after initial definition*
