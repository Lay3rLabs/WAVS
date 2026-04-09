# Roadmap: WAVS Improvements

## Milestones

- ✅ **v1.0 WAVS Improvements** — Phases 1-6 (shipped 2026-04-07)
- ✅ **v1.1 Open Source AI Providers & Settings UX** — Phases 7-9 (shipped 2026-04-08)
- ✅ **v1.2 Components Explorer** — Phases 10-12 (shipped 2026-04-08)
- 🚧 **v1.3 Activity UX & Bug Fixes** — Phases 13-16 (in progress)

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

<details>
<summary>✅ v1.2 Components Explorer (Phases 10-12) — SHIPPED 2026-04-08</summary>

- [x] Phase 10: Backend Commands (1/1 plan) — completed 2026-04-08
- [x] Phase 11: Component Detail Page (2/2 plans) — completed 2026-04-08
- [x] Phase 12: Components List Page (1/1 plan) — completed 2026-04-08

Full details: `.planning/milestones/v1.2-ROADMAP.md`

</details>

### 🚧 v1.3 Activity UX & Bug Fixes (In Progress)

**Milestone Goal:** Richer activity cards with inline submission data, smart result decoding, service restart reliability, and wallet settings kebab menu.

- [x] **Phase 13: Activity Backend Pipeline** — Forward tx_hash and result_payload through the Rust submission event pipeline (completed 2026-04-09)
- [x] **Phase 14: Activity Frontend UX** — Inline submission cards and smart result decoding in the UI (completed 2026-04-09)
- [x] **Phase 15: Service Restart Reliability** — Fix trigger re-subscription race condition after process restart (completed 2026-04-09)
- [ ] **Phase 16: Wallet Kebab Menu** — Move uncommon wallet actions behind a kebab dropdown

## Phase Details

### Phase 13: Activity Backend Pipeline
**Goal**: Submission events carry tx_hash and execution result payload from aggregator to frontend
**Depends on**: Phase 12 (previous milestone complete)
**Requirements**: ACT-01, ACT-02
**Success Criteria** (what must be TRUE):
  1. A submission event received by the frontend includes a non-empty tx_hash field when a transaction was submitted
  2. A submission event received by the frontend includes the execution result payload (capped at 4 KB) for each confirmed submission
  3. The Rust SubmissionEvent struct, DispatcherCommand, and aggregator send site all carry tx_hash and result_payload with no compile errors
**Plans:** 1/1 plans complete
Plans:
- [x] 13-01-PLAN.md — Add tx_hash and result_payload to Rust pipeline and TypeScript types

### Phase 14: Activity Frontend UX
**Goal**: Users can see submission status, tx hash, and decoded result inline on activity cards without expanding
**Depends on**: Phase 13
**Requirements**: ACT-03, ACT-04
**Success Criteria** (what must be TRUE):
  1. Activity cards show submission status, tx hash, and result summary without requiring the user to expand the card
  2. Result payloads displayed as readable UTF-8 text when the bytes decode cleanly
  3. Result payloads displayed as pretty-printed JSON when the UTF-8 content is valid JSON
  4. Result payloads fall back to hex display when UTF-8 decoding fails
**Plans:** 1/1 plans complete
Plans:
- [x] 14-01-PLAN.md — Add decodeResultPayload utility, SubmissionRows to activity cards, update virtualizer height

### Phase 15: Service Restart Reliability
**Goal**: Services reliably restore trigger subscriptions after the WAVS process restarts
**Depends on**: Nothing (independent of Phases 13-14)
**Requirements**: SVC-01
**Success Criteria** (what must be TRUE):
  1. After a WAVS process restart, all previously registered services resume receiving trigger events without manual intervention
  2. No trigger events are silently dropped during the re-subscription window after restart
**Plans:** 1/1 plans complete
Plans:
- [x] 15-01-PLAN.md — Add pending EVM subscription queue to fix trigger re-subscription race condition

### Phase 16: Wallet Kebab Menu
**Goal**: Uncommon wallet actions are accessible via a kebab dropdown rather than inline buttons
**Depends on**: Nothing (independent pure frontend)
**Requirements**: SET-01
**Success Criteria** (what must be TRUE):
  1. The wallet settings section shows a kebab (three-dot) menu icon instead of inline Reset Wallet and Reveal Seed Phrase buttons
  2. Clicking the kebab menu reveals Reset Wallet and Reveal Seed Phrase as dropdown options
  3. The existing reset and reveal behaviors function identically after the menu change
**Plans:** 1 plan
Plans:
- [ ] 16-01-PLAN.md — Add kebab dropdown menu to wallet card header

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
| 10. Backend Commands | v1.2 | 1/1 | Complete | 2026-04-08 |
| 11. Component Detail Page | v1.2 | 2/2 | Complete | 2026-04-08 |
| 12. Components List Page | v1.2 | 1/1 | Complete | 2026-04-08 |
| 13. Activity Backend Pipeline | v1.3 | 1/1 | Complete    | 2026-04-09 |
| 14. Activity Frontend UX | v1.3 | 1/1 | Complete    | 2026-04-09 |
| 15. Service Restart Reliability | v1.3 | 1/1 | Complete    | 2026-04-09 |
| 16. Wallet Kebab Menu | v1.3 | 0/1 | Not started | - |
