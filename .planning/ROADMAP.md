# Roadmap: WAVS

## Milestones

- ✅ **v1.0 Commonware P2P Migration** -- Phases 1-4 (shipped 2026-03-18)
- [ ] **v1.1 BLS Signatures** -- Phases 5-8 (in progress)

## Phases

<details>
<summary>v1.0 Commonware P2P Migration (Phases 1-4) -- SHIPPED 2026-03-18</summary>

- [x] Phase 1: Secure Peer Connectivity (3/3 plans) -- completed 2026-03-17
- [x] Phase 2: Broadcast and Routing (2/2 plans) -- completed 2026-03-17
- [x] Phase 3: Config and Observability (4/4 plans) -- completed 2026-03-17
- [x] Phase 4: Validation and Cleanup (2/2 plans) -- completed 2026-03-17

</details>

### v1.1 BLS Signatures

- [ ] **Phase 5: BLS Types and Key Derivation** - Core BLS types, WIT interface, contract ABIs, and deterministic key derivation from mnemonic
- [ ] **Phase 6: BLS Signing Pipeline** - Operators sign submission envelopes with BLS keys and propagate over P2P
- [ ] **Phase 7: BLS Aggregation** - Aggregator collects BLS submissions, aggregates signatures, submits to BLS service manager contract
- [ ] **Phase 8: Integration and Verification** - End-to-end BLS flow on local anvil with poa-middleware contracts, secp256k1 regression

## Phase Details

### Phase 5: BLS Types and Key Derivation
**Goal**: BLS12-381 types exist in the codebase -- signature algorithm enum, submission data structures, contract ABIs, and deterministic key derivation -- so downstream signing and aggregation code has a foundation to build on
**Depends on**: Phase 4 (v1.0 complete)
**Requirements**: TYPES-01, TYPES-02, TYPES-03, KEYS-01, KEYS-02
**Success Criteria** (what must be TRUE):
  1. `SignatureAlgorithm::Bls12381` variant compiles in Rust and is expressible in the WIT interface alongside `Secp256k1`
  2. `SignatureData` (or equivalent) struct can represent a BLS submission: G2 aggregate signature bytes, a Vec of G1 signer pubkeys, and a reference block number
  3. poa-middleware BLS contract ABIs (POAStakeRegistry BLS variant, BLS12381.sol) are importable from `packages/types` and generate Alloy bindings
  4. Given a signing mnemonic and an HD index, a BLS private key is deterministically derived via `blst` -- same mnemonic + index always produces the same key
  5. A 128-byte G1 public key can be derived from a BLS private key for use in operator registration
**Plans**: 3 plans

Plans:
- [ ] 05-01-PLAN.md -- BLS ABI bindings, SignatureAlgorithm variant, WIT updates
- [ ] 05-02-PLAN.md -- SignatureData/WavsSignature/WavsCryptoSigner enum migration
- [ ] 05-03-PLAN.md -- BLS key derivation and G1 pubkey conversion

### Phase 6: BLS Signing Pipeline
**Goal**: An operator configured for BLS can sign a submission envelope with its BLS key and propagate the signed submission (BLS signature + G1 pubkey) over P2P, while secp256k1 services continue working unchanged
**Depends on**: Phase 5
**Requirements**: SIGN-01, SIGN-02, SIGN-03
**Success Criteria** (what must be TRUE):
  1. When a BLS-configured service produces a submission, the operator signs the envelope digest using hash-to-curve with DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_` and produces a 256-byte G2 signature -- blst signing runs on a blocking thread pool (not Tokio async)
  2. The `Submission` message propagated over P2P includes the operator's G2 signature and G1 public key when the service uses BLS
  3. A service configured with `signature_algorithm: secp256k1` produces identical submissions to before this milestone -- no behavioral change in the secp256k1 path
**Plans**: TBD

### Phase 7: BLS Aggregation
**Goal**: The aggregator collects BLS-signed submissions from multiple operators, aggregates them into a single BLS aggregate (one G2 sig + sorted G1 pubkeys + reference block), and submits the result to the BLS service manager contract on-chain
**Depends on**: Phase 6
**Requirements**: AGG-01, AGG-02, AGG-03, AGG-04
**Success Criteria** (what must be TRUE):
  1. The aggregator accumulates BLS G2 signatures and G1 pubkeys from peer submissions until the configured quorum threshold is met
  2. At quorum, G2 signatures are aggregated via BLS point addition into a single aggregate signature; signer G1 pubkeys are sorted by `keccak256(pubkey)` ascending (matching the contract's expected ordering)
  3. A `referenceBlock` is captured that is strictly less than the current block number at submission time and greater than or equal to the block when operators registered their keys
  4. The aggregated `SignatureData { signerPubkeys, aggregateSignature, referenceBlock }` is submitted to the BLS service manager contract via the existing EVM submission path
**Plans**: TBD

### Phase 8: Integration and Verification
**Goal**: The full BLS pipeline is verified end-to-end on a local anvil chain with real poa-middleware BLS contracts, and existing secp256k1 tests confirm no regressions
**Depends on**: Phase 7
**Requirements**: INT-01, INT-02
**Success Criteria** (what must be TRUE):
  1. An E2E test deploys poa-middleware BLS contracts on local anvil, registers multiple operators with BLS keys, triggers a BLS service, and verifies that the aggregated BLS signature is accepted on-chain (pairing check passes via EIP-2537 precompiles)
  2. All existing secp256k1 E2E tests in `packages/layer-tests/` pass without modification -- the BLS addition causes zero regressions in the secp256k1 path
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 5 -> 6 -> 7 -> 8

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Secure Peer Connectivity | v1.0 | 3/3 | Complete | 2026-03-17 |
| 2. Broadcast and Routing | v1.0 | 2/2 | Complete | 2026-03-17 |
| 3. Config and Observability | v1.0 | 4/4 | Complete | 2026-03-17 |
| 4. Validation and Cleanup | v1.0 | 2/2 | Complete | 2026-03-17 |
| 5. BLS Types and Key Derivation | v1.1 | 0/3 | Planned | - |
| 6. BLS Signing Pipeline | v1.1 | 0/? | Not started | - |
| 7. BLS Aggregation | v1.1 | 0/? | Not started | - |
| 8. Integration and Verification | v1.1 | 0/? | Not started | - |

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for v1.0 phase details.
