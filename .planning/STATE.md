---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: BLS Signatures
status: in-progress
stopped_at: Plan 06-01 complete
last_updated: "2026-03-20T00:21:00Z"
last_activity: 2026-03-20 -- Plan 06-01 complete (BLS signing core with contract-matching DST)
progress:
  total_phases: 4
  completed_phases: 1
  total_plans: 3
  completed_plans: 4
  percent: 72
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-18)

**Core value:** Multi-operator signature aggregation over P2P must work reliably -- operators broadcast signed submissions, reach quorum, and submit on-chain
**Current focus:** Phase 6 - BLS Signing Pipeline

## Current Position

Phase: 6 of 8 (BLS Signing Pipeline)
Plan: 1 of 2 complete
Status: Phase 6 in progress
Last activity: 2026-03-20 -- Plan 06-01 complete (BLS signing core with contract-matching DST)

Progress: [##############......] 72% (plan 06-01 done, 06-02 next)

## Performance Metrics

**Velocity:**
- Total plans completed: 11 (v1.0)
- Average duration: see v1.0 retrospective
- Total execution time: see v1.0 retrospective

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 3 | - | - |
| 2 | 2 | - | - |
| 3 | 4 | - | - |
| 4 | 2 | - | - |
| 5-01 | 1 | 11min | 11min |
| 5-02 | 1 | 18min | 18min |
| 5-03 | 1 | 15min | 15min |
| 6-01 | 1 | 7min | 7min |
| 5-8 | TBD | - | - |

*Updated after each plan completion*

## Accumulated Context

### Decisions

- BLS coexists with secp256k1 as a per-service option -- no breaking changes
- Off-chain BLS aggregation in WAVS aggregator -- one aggregate sig per submission
- blst 0.3.16 already in Cargo.lock as transitive dep via commonware-cryptography
- Hash-to-curve DST must match contracts: BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_
- signerPubkeys sorted by keccak256(pubkey) ascending -- contract enforces this
- referenceBlock must be < current block at submission time
- blst signing is CPU-bound -- must use spawn_blocking in async context
- No MCP tooling for BLS in v1.1 -- defer to v1.2
- BLS bindings use non-rpc pattern (no #[sol(rpc)]) -- BLS contract interaction handled differently in Phase 7
- BLS types re-exported as BlsServiceHandler/BlsStakeRegistry/BlsServiceManager to avoid collision with secp256k1
- SignatureKind::bls_default() uses prefix=None -- BLS uses hash-to-curve with its own DST, not EIP-191
- WavsSigner trait returns error for BLS -- dedicated BLS signer to be implemented in Plan 02
- HKDF-SHA256 with domain separation (WAVS-BLS-KEY-v1 || hd_index LE) for BLS key derivation from mnemonic
- rand_chacha pinned to 0.3 in packages/utils to match commonware's rand_core 0.6
- bip39 made non-optional in packages/utils for BLS key derivation
- blst FFI used directly for G1 decompression (blst_p1_uncompress + blst_p1_affine_serialize)
- commonware-cryptography optional dep in wavs-types behind 'bls' feature; 'full' feature includes it
- SignatureData raw Alloy type pub(crate) in solidity_types; enum version is canonical crate-level export
- WavsSignature uses #[serde(tag = "algorithm")] -- breaking serialization change from old struct format
- WavsCryptoSigner::Bls12381 gated behind cfg(feature = "bls") for conditional compilation
- Use blst directly (not commonware Signer::sign) for contract-compatible DST (RO vs POP suffix)
- Mirror bls_helpers in wavs-types because circular dep prevents layer-utils import
- commonware_codec::Encode for PrivateKey byte extraction (Bytes deref to &[u8])
- tokio optional dep in wavs-types behind bls feature for spawn_blocking
- bls feature made default-on in wavs-types

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-20T00:21:00Z
Stopped at: Completed 06-01-PLAN.md
Resume file: .planning/phases/06-bls-signing-pipeline/06-01-SUMMARY.md
