---
phase: 05-bls-types-and-key-derivation
plan: 01
subsystem: types
tags: [bls12381, alloy, solidity-abi, wit, signature-algorithm, serde]

# Dependency graph
requires: []
provides:
  - BLS ABI JSON files (IWavsServiceHandler, IPOAStakeRegistry, IWavsServiceManager) in packages/types
  - Alloy-generated Rust bindings (BlsServiceHandler, BlsStakeRegistry, BlsServiceManager)
  - SignatureAlgorithm::Bls12381 enum variant with serde support
  - SignatureKind::bls_default() factory method
  - WIT bls12381 variant in all three interface locations
affects: [05-02-bls-key-derivation, 05-03-bls-signing, aggregator, submission, engine]

# Tech tracking
tech-stack:
  added: [alloy_sol_macro BLS bindings]
  patterns: [namespaced BLS re-exports to avoid collision with secp256k1 bindings]

key-files:
  created:
    - packages/types/src/contracts/solidity/abi/bls/IWavsServiceHandler.json
    - packages/types/src/contracts/solidity/abi/bls/IPOAStakeRegistry.json
    - packages/types/src/contracts/solidity/abi/bls/IWavsServiceManager.json
    - packages/types/src/solidity_types/bls.rs
  modified:
    - packages/types/src/solidity_types/mod.rs
    - packages/types/src/service.rs
    - packages/types/src/signing/signer.rs
    - packages/engine/src/bindings/types/wavs_to_component.rs
    - packages/engine/src/bindings/types/component_to_wavs.rs
    - wit-definitions/types/wit/service.wit
    - wit-definitions/aggregator/wit/deps/wavs-types-2.7.0/package.wit
    - wit-definitions/operator/wit/deps/wavs-types-2.7.0/package.wit

key-decisions:
  - "BLS bindings use non-rpc pattern (no #[sol(rpc)]) since BLS contract interaction handled differently in Phase 7"
  - "BLS types re-exported with namespaced names (BlsServiceHandler, BlsStakeRegistry, BlsServiceManager) to avoid collision with existing secp256k1 types"
  - "SignatureKind::bls_default() uses prefix=None since BLS uses hash-to-curve with its own DST, not EIP-191"
  - "WavsSigner trait returns error for BLS algorithm -- dedicated BLS signer to be implemented in Plan 02"

patterns-established:
  - "Namespaced BLS re-exports: separate bls.rs module with 'as Bls*' re-exports to coexist with secp256k1"
  - "Guard arms in existing match blocks: new signature algorithm variants return descriptive errors until implementation lands"

requirements-completed: [TYPES-01, TYPES-03]

# Metrics
duration: 11min
completed: 2026-03-19
---

# Phase 5 Plan 1: BLS Types and Key Derivation Summary

**BLS12-381 ABI bindings generating Rust types via alloy_sol_macro, SignatureAlgorithm::Bls12381 variant compiling and serializing as "bls12381", WIT interfaces updated in all three locations**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-19T17:19:04Z
- **Completed:** 2026-03-19T17:30:10Z
- **Tasks:** 2
- **Files modified:** 12

## Accomplishments
- Copied BLS ABI JSON files from poa-middleware and created Alloy Rust bindings with namespaced re-exports
- Added SignatureAlgorithm::Bls12381 variant with correct serde serialization ("bls12381") and SignatureKind::bls_default() factory
- Updated WIT signature-algorithm variant in all three locations (types, aggregator, operator)
- All 21 wavs-types tests pass including 4 new BLS-specific tests, zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Copy BLS ABI JSON files and create Alloy bindings module** - `9fc6a5fd` (feat)
2. **Task 2: Add Bls12381 variant to SignatureAlgorithm and update WIT interfaces** - `deb321c3` (feat)

## Files Created/Modified
- `packages/types/src/contracts/solidity/abi/bls/IWavsServiceHandler.json` - BLS service handler ABI with signerPubkeys/aggregateSignature fields
- `packages/types/src/contracts/solidity/abi/bls/IPOAStakeRegistry.json` - BLS stake registry ABI
- `packages/types/src/contracts/solidity/abi/bls/IWavsServiceManager.json` - BLS service manager ABI
- `packages/types/src/solidity_types/bls.rs` - Alloy-generated BLS bindings with BlsServiceHandler, BlsStakeRegistry, BlsServiceManager exports and inline tests
- `packages/types/src/solidity_types/mod.rs` - Added `mod bls; pub use bls::*;`
- `packages/types/src/service.rs` - Added Bls12381 variant, bls_default(), serde tests
- `packages/types/src/signing/signer.rs` - Added BLS guard arms returning descriptive errors
- `packages/engine/src/bindings/types/wavs_to_component.rs` - Added Bls12381 conversion arms for component_service and aggregator_service
- `packages/engine/src/bindings/types/component_to_wavs.rs` - Added Bls12381 reverse conversion arm
- `wit-definitions/types/wit/service.wit` - Added bls12381 to signature-algorithm variant
- `wit-definitions/aggregator/wit/deps/wavs-types-2.7.0/package.wit` - Added bls12381 to signature-algorithm variant
- `wit-definitions/operator/wit/deps/wavs-types-2.7.0/package.wit` - Added bls12381 to signature-algorithm variant

## Decisions Made
- BLS bindings use non-rpc pattern (no `#[sol(rpc)]`) since BLS contract interaction will be handled differently in Phase 7
- Re-exported with `Bls*` prefix to avoid collision with existing secp256k1 bindings in the same namespace
- `SignatureKind::bls_default()` uses `prefix: None` because BLS uses hash-to-curve with its own DST, not EIP-191
- WavsSigner trait's `sign()` and `evm_signer_address()` return clear errors for BLS -- dedicated BLS signing is Plan 02's responsibility

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated operator WIT file not listed in plan**
- **Found during:** Task 2 (WIT interface updates)
- **Issue:** Plan only listed 2 WIT files to update (service.wit and aggregator package.wit), but a third copy exists at `wit-definitions/operator/wit/deps/wavs-types-2.7.0/package.wit` -- engine build failed with missing `Bls12381` variant
- **Fix:** Updated the operator package.wit to include `bls12381` in signature-algorithm variant
- **Files modified:** wit-definitions/operator/wit/deps/wavs-types-2.7.0/package.wit
- **Verification:** `cargo build -p wavs-engine` succeeds
- **Committed in:** deb321c3 (Task 2 commit)

**2. [Rule 3 - Blocking] Updated engine binding conversion match arms**
- **Found during:** Task 2 (adding Bls12381 variant)
- **Issue:** Non-exhaustive match patterns in wavs_to_component.rs (2 blocks) and component_to_wavs.rs (1 block) would fail to compile
- **Fix:** Added Bls12381 conversion arms mapping between wavs_types and WIT-generated SignatureAlgorithm types
- **Files modified:** packages/engine/src/bindings/types/wavs_to_component.rs, packages/engine/src/bindings/types/component_to_wavs.rs
- **Verification:** `cargo build -p wavs-engine` succeeds
- **Committed in:** deb321c3 (Task 2 commit)

**3. [Rule 3 - Blocking] Updated signer.rs match arms for new variant**
- **Found during:** Task 2 (adding Bls12381 variant)
- **Issue:** Two match blocks in signing/signer.rs on SignatureAlgorithm would become non-exhaustive
- **Fix:** Added BLS guard arms that return descriptive errors (BLS signing not yet implemented via this trait)
- **Files modified:** packages/types/src/signing/signer.rs
- **Verification:** `cargo build -p wavs-types` and all tests pass
- **Committed in:** deb321c3 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes were necessary to maintain compilation after adding the new enum variant. No scope creep -- these are direct consequences of adding a new variant to an exhaustively-matched enum.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- BLS type foundation is complete: ABI bindings, enum variant, WIT interfaces all in place
- Plan 02 (BLS Key Derivation) can proceed -- BlsServiceHandler::SignatureData type and SignatureAlgorithm::Bls12381 are available
- Plan 03 (BLS Signing) can proceed -- SignatureKind::bls_default() is ready

## Self-Check: PASSED

---
*Phase: 05-bls-types-and-key-derivation*
*Completed: 2026-03-19*
