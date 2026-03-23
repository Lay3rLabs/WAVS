---
phase: 07-bls-aggregation
plan: 01
subsystem: aggregator
tags: [bls, blst, eip-2537, signature-aggregation, keccak256, solidity-rpc]

# Dependency graph
requires:
  - phase: 06-bls-signing-pipeline
    provides: BLS signer, WavsSignature::Bls12381 variant, bls_helpers module
provides:
  - BLS signature_data() aggregation via blst point addition
  - BLS queue dedup via keccak256(g1_pubkey) identity
  - BLS RPC contract bindings (BlsServiceHandlerInstance, BlsServiceManagerInstance)
  - EIP-2537 G2 deserialization/serialization helpers
affects: [07-02-bls-submission, aggregator, submission]

# Tech tracking
tech-stack:
  added: []
  patterns: [algorithm-generic signer_identity for queue dedup, cfg_if! inline RPC gating]

key-files:
  created: []
  modified:
    - packages/types/src/signing/signer.rs
    - packages/types/src/solidity_types/bls.rs
    - packages/types/Cargo.toml
    - packages/wavs/src/subsystems/aggregator/queue.rs

key-decisions:
  - "BLS G2 aggregate via blst point addition (AggregateSignature::aggregate) with EIP-2537 roundtrip"
  - "Queue dedup uses signer_identity() abstraction (EVM address for secp256k1, keccak256(g1_pubkey) for BLS)"
  - "BLS RPC bindings use cfg_if! inline in bls.rs rather than separate files"
  - "cfg(not(feature = bls)) fallback returns error instead of unimplemented!() panic"

patterns-established:
  - "signer_identity() pattern: algorithm-generic identity extraction for queue/dedup logic"
  - "cfg_if! for inline feature-gated sol(rpc) bindings in existing module files"

requirements-completed: [AGG-01, AGG-02]

# Metrics
duration: 12min
completed: 2026-03-20
---

# Phase 7 Plan 01: BLS Aggregation and Queue Dedup Summary

**BLS G2 signature aggregation via blst point addition with keccak256-sorted G1 pubkeys, plus algorithm-generic queue dedup replacing evm_signer_address**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-20T01:20:35Z
- **Completed:** 2026-03-20T01:33:26Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- BLS signature_data() produces valid SignatureData::Bls12381 with aggregated G2 sig and sorted G1 pubkeys
- BLS submissions enter quorum queue via keccak256(g1_pubkey) dedup -- no more evm_signer_address errors
- BLS RPC bindings compile under solidity-rpc feature (BlsServiceHandlerInstance, BlsServiceManagerInstance)
- No unimplemented!() panics remain for BLS in signature_data()
- secp256k1 path fully unchanged (existing tests pass)

## Task Commits

Each task was committed atomically (TDD: test -> feat):

1. **Task 1: BLS RPC bindings and signature_data() aggregation arm**
   - `59fffd47` (test: add failing BLS signature_data tests -- RED)
   - `496390e6` (feat: implement BLS signature aggregation and RPC bindings -- GREEN)
2. **Task 2: BLS queue deduplication in append_submission_to_queue**
   - `c88829bf` (test: add failing BLS queue deduplication tests -- RED)
   - `64afe5bd` (feat: implement BLS queue dedup with signer_identity -- GREEN)
3. **Cargo.lock update**
   - `5a92686a` (chore: update Cargo.lock for BLS test dependencies)

## Files Created/Modified
- `packages/types/src/signing/signer.rs` - BLS signature_data() aggregation arm, EIP-2537 G2 deser/ser helpers, tests
- `packages/types/src/solidity_types/bls.rs` - #[sol(rpc)] BLS contract bindings, type aliases
- `packages/types/Cargo.toml` - dev-deps for BLS tests (rand_chacha, commonware-math, commonware-cryptography)
- `packages/wavs/src/subsystems/aggregator/queue.rs` - signer_identity() function, generic queue dedup, tests

## Decisions Made
- BLS G2 aggregate via blst `AggregateSignature::aggregate` with EIP-2537 roundtrip (deser -> aggregate -> ser)
- Queue dedup uses `signer_identity()` abstraction rather than algorithm-specific branches in the main function
- BLS RPC bindings placed inline in bls.rs with cfg_if! (not separate bls_rpc.rs / bls_not_rpc.rs files)
- `cfg(not(feature = "bls"))` arm returns Err instead of unimplemented!() to avoid panics

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Initial test compilation: needed `rand_chacha::rand_core::SeedableRng` import instead of `rand_core::SeedableRng` due to re-export chain with rand_core 0.6
- QuorumQueueId test construction needed `std::str::FromStr` import for ChainKey and string-based EvmAddr parse

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- BLS aggregation and queue dedup complete -- Plan 02 (BLS submission to contract) can proceed
- BlsServiceHandlerInstance and BlsServiceManagerInstance type aliases ready for on-chain submission
- signer_identity() pattern available for any future algorithm-generic queue logic

## Self-Check: PASSED

All 4 modified files verified on disk. All 5 commit hashes verified in git log.

---
*Phase: 07-bls-aggregation*
*Completed: 2026-03-20*
