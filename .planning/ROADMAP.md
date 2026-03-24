# Roadmap: WAVS

## Milestones

- ✅ **v1.0 Commonware P2P Migration** -- Phases 1-4 (shipped 2026-03-18)
- ✅ **v1.1 BLS Signatures** -- Phases 5-8 (shipped 2026-03-23)
- 🚧 **v1.2 Tauri App** -- Phases 9-12 (in progress)

## Phases

<details>
<summary>✅ v1.0 Commonware P2P Migration (Phases 1-4) -- SHIPPED 2026-03-18</summary>

- [x] Phase 1: Secure Peer Connectivity (3/3 plans) -- completed 2026-03-17
- [x] Phase 2: Broadcast and Routing (2/2 plans) -- completed 2026-03-17
- [x] Phase 3: Config and Observability (4/4 plans) -- completed 2026-03-17
- [x] Phase 4: Validation and Cleanup (2/2 plans) -- completed 2026-03-17

</details>

<details>
<summary>✅ v1.1 BLS Signatures (Phases 5-8) -- SHIPPED 2026-03-23</summary>

- [x] Phase 5: BLS Types and Key Derivation (3/3 plans) -- completed 2026-03-19
- [x] Phase 6: BLS Signing Pipeline (2/2 plans) -- completed 2026-03-20
- [x] Phase 7: BLS Aggregation (2/2 plans) -- completed 2026-03-20
- [x] Phase 8: Integration and Verification (2/2 plans) -- completed 2026-03-23

</details>

### 🚧 v1.2 Tauri App (In Progress)

**Milestone Goal:** Bring the Tauri desktop app up to date with v1.0/v1.1 backend features -- BLS service deployment with operator registration, full P2P/operator visibility, unified activity events, and settings UX overhaul.

- [ ] **Phase 9: Foundation Types and Settings Refactor** - Frontend type system updated for BLS/P2P, settings decomposed, Tauri command infrastructure ready
- [ ] **Phase 10: P2P Operator Dashboard** - Full P2P visibility page with Ed25519 identity, peers, services, and operator keys
- [ ] **Phase 11: BLS Service Builder and Registration** - BLS algorithm selection in service builder, post-deploy key display, one-click operator registration
- [ ] **Phase 12: Unified Activity Events** - Merged trigger+submission event cards with status progression and error display

## Phase Details

### Phase 9: Foundation Types and Settings Refactor
**Goal**: App type system and structural prerequisites are ready for all v1.2 features
**Depends on**: Phase 8 (v1.1 BLS backend complete)
**Requirements**: FND-01, FND-02, FND-03, FND-04, SET-01, SET-02
**Success Criteria** (what must be TRUE):
  1. `SignatureAlgorithm` type in frontend includes `'bls12381'` and is used by service builder store
  2. New Tauri commands (`cmd_get_p2p_status`, `cmd_get_service_signer`, `cmd_derive_bls_pubkey`) are callable from the frontend and return typed responses
  3. Settings page renders identically to before but is composed of section components (no 940-line monolith)
  4. Settings sections have clear visual hierarchy with consistent spacing and typography
  5. Existing app functionality is unchanged (no regressions from type widening or settings decomposition)
**Plans**: 2 plans

Plans:
- [x] 09-01-PLAN.md -- Foundation types and Tauri commands (FND-01, FND-02, FND-03)
- [x] 09-02-PLAN.md -- Settings decomposition with sidebar nav (FND-04, SET-01, SET-02)

### Phase 10: P2P Operator Dashboard
**Goal**: Operators can see their P2P network status, connected peers, and per-service keys from a dedicated page
**Depends on**: Phase 9 (P2P types and Tauri commands)
**Requirements**: P2P-01, P2P-02, P2P-03, P2P-04, P2P-05, P2P-06
**Success Criteria** (what must be TRUE):
  1. P2P page is accessible from header nav and displays the node's Ed25519 identity, discovery mode, and listen addresses
  2. Connected peers list shows peer IDs and connection status, updating on a regular interval
  3. Subscribed services list shows which services are active on P2P topics with human-readable names
  4. Each service displays its operator key (BLS G1 pubkey or ECDSA address) with a copy-to-clipboard button and on-chain registration status indicator
  5. *(Stretch)* Live quorum accumulation progress is visible per service when data is available
**Plans**: 2 plans

Plans:
- [x] 10-01-PLAN.md -- P2P page with identity, peers, and services cards (P2P-01, P2P-02, P2P-03, P2P-06)
- [ ] 10-02-PLAN.md -- Per-service operator key display and registration status (P2P-04, P2P-05)

### Phase 11: BLS Service Builder and Registration
**Goal**: Operators can deploy BLS services and register their BLS keys on-chain entirely from the app
**Depends on**: Phase 9 (BLS types and `cmd_derive_bls_pubkey` command)
**Requirements**: BLS-01, BLS-02, BLS-03, BLS-04
**Success Criteria** (what must be TRUE):
  1. Service builder submit step shows an algorithm selector (ECDSA / BLS) and the selection propagates through deployment
  2. After deploying a BLS service, the operator's BLS G1 pubkey is displayed with a copy-to-clipboard button
  3. Operator can register their BLS key on-chain with a single click (calls `updateOperatorSigningKey` on the BLS registry contract)
  4. Service detail page shows BLS registration status (registered/unregistered) read from on-chain state
**Plans**: TBD

Plans:
- [ ] 11-01: TBD
- [ ] 11-02: TBD

### Phase 12: Unified Activity Events
**Goal**: Operators see a clear, merged view of trigger and submission lifecycle in the activity feed
**Depends on**: Phase 9 (event types), Phase 10 and 11 not required
**Requirements**: ACT-01, ACT-02, ACT-03
**Success Criteria** (what must be TRUE):
  1. Trigger and submission events are merged into unified event cards (one card per workflow execution, not separate trigger and submission entries)
  2. Each event card shows status progression (pending, submitted, confirmed, or error) with visual indicators
  3. Submission errors are displayed inline on the event card with the error message visible
**Plans**: TBD

Plans:
- [ ] 12-01: TBD
- [ ] 12-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 9 → 10 → 11 → 12

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Secure Peer Connectivity | v1.0 | 3/3 | Complete | 2026-03-17 |
| 2. Broadcast and Routing | v1.0 | 2/2 | Complete | 2026-03-17 |
| 3. Config and Observability | v1.0 | 4/4 | Complete | 2026-03-17 |
| 4. Validation and Cleanup | v1.0 | 2/2 | Complete | 2026-03-17 |
| 5. BLS Types and Key Derivation | v1.1 | 3/3 | Complete | 2026-03-19 |
| 6. BLS Signing Pipeline | v1.1 | 2/2 | Complete | 2026-03-20 |
| 7. BLS Aggregation | v1.1 | 2/2 | Complete | 2026-03-20 |
| 8. Integration and Verification | v1.1 | 2/2 | Complete | 2026-03-23 |
| 9. Foundation Types and Settings Refactor | v1.2 | 2/2 | Complete | 2026-03-24 |
| 10. P2P Operator Dashboard | v1.2 | 1/2 | In Progress|  |
| 11. BLS Service Builder and Registration | v1.2 | 0/? | Not started | - |
| 12. Unified Activity Events | v1.2 | 0/? | Not started | - |

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for v1.0 phase details.
See [milestones/v1.1-ROADMAP.md](milestones/v1.1-ROADMAP.md) for v1.1 phase details.
