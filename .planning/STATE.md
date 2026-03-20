---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: BLS Signatures
status: completed
stopped_at: Completed 06-02-PLAN.md (Phase 6 complete)
last_updated: "2026-03-20T00:48:16.755Z"
last_activity: 2026-03-20 -- Plan 06-02 complete (BLS submission pipeline wiring)
progress:
  total_phases: 4
  completed_phases: 2
  total_plans: 5
  completed_plans: 5
  percent: 85
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-18)

**Core value:** Multi-operator signature aggregation over P2P must work reliably -- operators broadcast signed submissions, reach quorum, and submit on-chain
**Current focus:** Phase 6 - BLS Signing Pipeline

## Current Position

Phase: 6 of 8 (BLS Signing Pipeline) -- COMPLETE
Plan: 2 of 2 complete
Status: Phase 6 complete
Last activity: 2026-03-20 -- Plan 06-02 complete (BLS submission pipeline wiring)

Progress: [#################...] 85% (phase 6 complete, phase 7 next)

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
| 6-02 | 1 | 12min | 12min |
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
- bls feature added and default-on in packages/wavs/Cargo.toml
- add_service_key dispatches on SignatureAlgorithm to create correct signer type
- Dispatcher auto-detects algorithm from Submit::Aggregator { signature_kind } with secp256k1 default
- SignerResponse::Bls12381 variant with hd_index and g1_pubkey_hex for graceful HTTP API
- get_service_signer uses and_then (not map) for fallible BLS G1 pubkey extraction

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-20T00:37:31Z
Stopped at: Completed 06-02-PLAN.md (Phase 6 complete)
Resume file: .planning/phases/06-bls-signing-pipeline/06-02-SUMMARY.md
