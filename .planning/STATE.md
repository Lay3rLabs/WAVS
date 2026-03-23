---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: BLS Signatures
status: unknown
stopped_at: Completed 08-01-PLAN.md
last_updated: "2026-03-23T13:31:27.900Z"
progress:
  total_phases: 4
  completed_phases: 4
  total_plans: 9
  completed_plans: 9
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-18)

**Core value:** Multi-operator signature aggregation over P2P must work reliably -- operators broadcast signed submissions, reach quorum, and submit on-chain
**Current focus:** Phase 08 — integration-and-verification

## Current Position

Phase: 08
Plan: Not started

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
| 7-01 | 1 | 12min | 12min |
| 7-02 | 1 | 21min | 21min |
| 8-01 | 1 | 6min | 6min |
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
- BLS G2 aggregate via blst AggregateSignature::aggregate with EIP-2537 roundtrip
- Queue dedup uses signer_identity() abstraction (EVM addr for secp256k1, keccak256(g1_pubkey) for BLS)
- BLS RPC bindings (#[sol(rpc)]) use cfg_if! inline in bls.rs
- cfg(not(feature = bls)) fallback returns error instead of unimplemented!() panic
- BLS service manager validate() needs its own re-exported Envelope/SignatureData types (BlsServiceManagerEnvelope/BlsServiceManagerSignatureData)
- send_bls_envelope_signatures accepts non-rpc BlsServiceHandler::SignatureData, converts to rpc types internally
- BLS dispatch bypasses From<SignatureData> for ServiceManagerSignatureData -- constructs types directly
- PoaBlsMiddleware uses local forge/cast instead of Docker image -- avoids uncertainty about BLS artifact inclusion
- FOUNDRY_PROFILE=bls env var (not --profile flag) for forge commands matching poa-middleware shell scripts
- SimpleBlsSubmit does not implement ISimpleSubmit -- BLS and ECDSA SignatureData are incompatible types
- hardfork_prague parameter on EvmInstance::spawn with false default for backward compatibility

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-20T12:45:31Z
Stopped at: Completed 08-01-PLAN.md
Resume file: .planning/phases/08-integration-and-verification/08-01-SUMMARY.md
