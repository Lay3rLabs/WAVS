# Requirements: WAVS

**Defined:** 2026-03-23
**Milestone:** v1.2 Tauri App
**Core Value:** Multi-operator signature aggregation over P2P must work reliably -- operators broadcast signed submissions, reach quorum, and submit on-chain.

## v1.2 Requirements

### Foundation

- [x] **FND-01**: `SignatureAlgorithm` type updated to include `'bls12381'` alongside `'secp256k1'` in frontend types
- [x] **FND-02**: New Tauri commands for P2P status (`cmd_get_p2p_status`), service signer info (`cmd_get_service_signer`), and BLS key derivation (`cmd_derive_bls_pubkey`)
- [x] **FND-03**: `P2pStatus` and `SignerResponse` TypeScript types matching backend Rust structs
- [x] **FND-04**: Settings.tsx decomposed from monolithic 940-line file into section components

### P2P Dashboard

- [x] **P2P-01**: P2P page accessible from header nav showing Ed25519 identity, discovery mode, and listen addresses
- [x] **P2P-02**: Connected peers list with peer IDs and connection status
- [x] **P2P-03**: Subscribed services list showing which services are active on P2P topics
- [ ] **P2P-04**: Per-service operator key display (BLS G1 pubkey or ECDSA address) with copy button
- [ ] **P2P-05**: Operator key registration status indicator (registered/unregistered on-chain)
- [x] **P2P-06**: *(Stretch)* Live quorum accumulation progress per service

### BLS Service Deployment

- [ ] **BLS-01**: Algorithm selector (ECDSA/BLS) in service builder submit step
- [ ] **BLS-02**: Post-deploy BLS G1 pubkey display with copy-to-clipboard
- [ ] **BLS-03**: One-click BLS key registration on-chain (calls `updateOperatorSigningKey` on BLS registry)
- [ ] **BLS-04**: BLS registration status shown on service detail page

### Activity

- [ ] **ACT-01**: Trigger and submission events merged into unified event cards (trigger event with submission result inlined)
- [ ] **ACT-02**: Event status progression displayed (pending → submitted → confirmed/error)
- [ ] **ACT-03**: Submission errors displayed inline on event cards

### Settings UX

- [x] **SET-01**: Settings page reorganized into logical sections with clear visual hierarchy
- [x] **SET-02**: Visual polish -- consistent spacing, typography, and component styling across all settings sections

## v2 Requirements

### MCP Tooling

- **MCP-01**: `wavs_register_operator` derives BLS key and calls `updateOperatorSigningKey` on BLS registry
- **MCP-02**: `wavs_get_signing_address` supports BLS pubkey output mode

### Threshold Signatures

- **THRESH-01**: Commonware threshold-simplex integration for DKG-based threshold BLS
- **THRESH-02**: Cross-chain certificate production via threshold BLS

### Cosmos

- **COSMOS-01**: BLS submission path for Cosmos chains

## Out of Scope

| Feature | Reason |
|---------|--------|
| MCP tooling for BLS operator registration | Defer -- CLI/manual registration sufficient for now |
| Threshold/DKG signatures | Foundational BLS first, threshold later |
| Cosmos BLS submission | EVM only for now |
| Trigger/engine subsystem changes | Unaffected by frontend milestone |
| Mobile app | Desktop-first via Tauri |
| Real-time P2P message feed | Complexity vs value -- status polling is sufficient |
| Component library migration (shadcn/Radix) | Existing hand-rolled Tailwind components work -- not worth the churn |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| FND-01 | Phase 9 | Complete |
| FND-02 | Phase 9 | Complete |
| FND-03 | Phase 9 | Complete |
| FND-04 | Phase 9 | Complete |
| P2P-01 | Phase 10 | Complete |
| P2P-02 | Phase 10 | Complete |
| P2P-03 | Phase 10 | Complete |
| P2P-04 | Phase 10 | Pending |
| P2P-05 | Phase 10 | Pending |
| P2P-06 | Phase 10 | Pending (Stretch) |
| BLS-01 | Phase 11 | Pending |
| BLS-02 | Phase 11 | Pending |
| BLS-03 | Phase 11 | Pending |
| BLS-04 | Phase 11 | Pending |
| ACT-01 | Phase 12 | Pending |
| ACT-02 | Phase 12 | Pending |
| ACT-03 | Phase 12 | Pending |
| SET-01 | Phase 9 | Complete |
| SET-02 | Phase 9 | Complete |

**Coverage:**
- v1.2 requirements: 19 total (18 core + 1 stretch)
- Mapped to phases: 19/19
- Unmapped: 0

---
*Requirements defined: 2026-03-23*
*Last updated: 2026-03-23 after roadmap creation*
