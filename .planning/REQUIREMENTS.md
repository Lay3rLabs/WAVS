# Requirements: WAVS

**Defined:** 2026-03-18
**Milestone:** v1.1 BLS Signatures
**Core Value:** Multi-operator signature aggregation over P2P must work reliably — operators broadcast signed submissions, reach quorum, and submit on-chain.

## v1.1 Requirements

### Types

- [x] **TYPES-01**: `SignatureAlgorithm::Bls12381` variant added to Rust enum and WIT interface
- [x] **TYPES-02**: BLS submission carries G2 aggregate signature + sorted G1 signer pubkeys + reference block
- [x] **TYPES-03**: poa-middleware BLS contract ABIs imported into `packages/types`

### Key Management

- [x] **KEYS-01**: BLS private key derived deterministically from signing mnemonic per service (HD index, using `blst` crate)
- [x] **KEYS-02**: BLS public key (G1 point, 128 bytes) derivable from private key for operator registration

### Signing

- [x] **SIGN-01**: Operator signs envelope digest with BLS key → G2 signature (256 bytes) using hash-to-curve consistent with `HashToCurve.sol` (RFC 9380, DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`)
- [x] **SIGN-02**: BLS signature and operator G1 pubkey included in `Submission` propagated over P2P
- [x] **SIGN-03**: Existing secp256k1 signing path unchanged — algorithm is per-service config

### Aggregation

- [ ] **AGG-01**: Aggregator collects BLS submissions from peers, accumulates G2 sigs and G1 pubkeys until quorum
- [ ] **AGG-02**: Aggregator aggregates G2 signatures into single aggregate sig via point addition; pubkeys sorted by keccak256 ascending (contract requirement)
- [ ] **AGG-03**: Aggregator captures `referenceBlock` at quorum time (must be < submission block)
- [ ] **AGG-04**: Aggregated `SignatureData { signerPubkeys[], aggregateSignature, referenceBlock }` submitted to BLS service manager contract

### Integration & Tests

- [ ] **INT-01**: E2E test: BLS service on local anvil with poa-middleware BLS contracts, multi-operator quorum reached and verified on-chain
- [ ] **INT-02**: Existing secp256k1 e2e tests unchanged and still passing

## v2 Requirements

### MCP Tooling

- **MCP-01**: `wavs_register_operator` derives BLS key and calls `updateOperatorSigningKey` on BLS registry
- **MCP-02**: `wavs_get_signing_address` supports BLS pubkey output mode

### Threshold Signatures

- **THRESH-01**: Commonware threshold-simplex integration for DKG-based threshold BLS
- **THRESH-02**: Cross-chain certificate production via threshold BLS

## Out of Scope

| Feature | Reason |
|---------|--------|
| MCP tooling for BLS operator registration | Defer to v1.2 — operators can register manually via CLI/direct contract calls |
| Tauri desktop app changes | Backend signature scheme transparent to frontend |
| Threshold/DKG signatures | Commonware threshold-simplex is future work — standard BLS first |
| Cosmos BLS submission | EVM only for this milestone |
| Trigger and engine changes | Unaffected by submission signature scheme |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TYPES-01 | Phase 5 | Complete |
| TYPES-02 | Phase 5 | Complete |
| TYPES-03 | Phase 5 | Complete |
| KEYS-01 | Phase 5 | Complete |
| KEYS-02 | Phase 5 | Complete |
| SIGN-01 | Phase 6 | Complete |
| SIGN-02 | Phase 6 | Complete |
| SIGN-03 | Phase 6 | Complete |
| AGG-01 | Phase 7 | Pending |
| AGG-02 | Phase 7 | Pending |
| AGG-03 | Phase 7 | Pending |
| AGG-04 | Phase 7 | Pending |
| INT-01 | Phase 8 | Pending |
| INT-02 | Phase 8 | Pending |

**Coverage:**
- v1.1 requirements: 14 total
- Mapped to phases: 14
- Unmapped: 0 ✓

---
*Requirements defined: 2026-03-18*
*Last updated: 2026-03-18 after initial definition*
