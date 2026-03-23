---
phase: 07-bls-aggregation
plan: 02
subsystem: aggregator
tags: [bls, evm, submission, aggregator, alloy, solidity]

# Dependency graph
requires:
  - phase: 07-01
    provides: "BLS aggregation logic producing SignatureData::Bls12381 variant"
provides:
  - "send_bls_envelope_signatures() on EvmSigningClient with full retry + gas estimation"
  - "BLS contract instance helpers (bls_service_handler, bls_service_manager) on both signing and query clients"
  - "BLS dispatch in handle_action_submit_evm() routing Bls12381 to BLS contract path"
  - "BlsServiceManagerEnvelope and BlsServiceManagerSignatureData re-exports for validate() calls"
affects: [phase-08, bls-e2e-testing]

# Tech tracking
tech-stack:
  added: []
  patterns: ["BLS Envelope field-by-field copy pattern for Alloy type mismatch between handler/manager RPC modules", "SignatureData variant dispatch for dual-algorithm submission"]

key-files:
  created: []
  modified:
    - packages/utils/src/evm_client/contracts.rs
    - packages/utils/src/evm_client/signing.rs
    - packages/wavs/src/subsystems/aggregator/submit.rs
    - packages/types/src/solidity_types/bls.rs
    - packages/types/src/signing.rs

key-decisions:
  - "BLS service manager validate() requires its own Alloy-generated Envelope/SignatureData types, not the handler RPC types -- added re-exports"
  - "send_bls_envelope_signatures accepts non-rpc BlsServiceHandler::SignatureData and converts internally to rpc types"
  - "BLS dispatch bypasses From<SignatureData> for ServiceManagerSignatureData conversion entirely"

patterns-established:
  - "Alloy type conversion pattern: field-by-field copy between handler-rpc, manager-rpc, and non-rpc variants of same ABI struct"
  - "SignatureData variant dispatch: match on Secp256k1/Bls12381 to route submission logic"

requirements-completed: [AGG-03, AGG-04]

# Metrics
duration: 21min
completed: 2026-03-20
---

# Phase 7 Plan 02: BLS Submission Pipeline Summary

**BLS EVM submission path wired end-to-end: send_bls_envelope_signatures() with retry logic, BLS contract helpers, and SignatureData::Bls12381 dispatch in handle_action_submit_evm()**

## Performance

- **Duration:** 21 min
- **Started:** 2026-03-20T01:36:54Z
- **Completed:** 2026-03-20T01:58:48Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- BLS contract instance helpers (bls_service_handler, bls_service_manager) on both EvmSigningClient and EvmQueryClient
- send_bls_envelope_signatures() with full retry logic, gas estimation, nonce refresh, and Alloy type conversion
- handle_action_submit_evm() dispatches between secp256k1 and BLS paths based on SignatureData variant
- BLS service manager validate() called before submission, matching secp256k1 pattern
- referenceBlock captured as block_height - 1 (strictly less than submission block, satisfying AGG-03)

## Task Commits

Each task was committed atomically:

1. **Task 1: BLS contract helpers and send_bls_envelope_signatures** - `387fbd54` (feat)
2. **Task 2: BLS dispatch in handle_action_submit_evm** - `e733883a` (feat)

## Files Created/Modified
- `packages/utils/src/evm_client/contracts.rs` - Added bls_service_handler() and bls_service_manager() to EvmSigningClient and EvmQueryClient
- `packages/utils/src/evm_client/signing.rs` - Added send_bls_envelope_signatures() with full retry/gas logic; updated BLS error message in send_envelope_signatures()
- `packages/wavs/src/subsystems/aggregator/submit.rs` - Added SignatureData dispatch with BLS service manager validate and send_bls_envelope_signatures path
- `packages/types/src/solidity_types/bls.rs` - Added BlsServiceManagerEnvelope and BlsServiceManagerSignatureData re-exports for validate() type compatibility
- `packages/types/src/signing.rs` - Replaced unimplemented!() with descriptive panic for BLS -> ServiceManagerSignatureData conversion

## Decisions Made
- BLS service manager's validate() expects its own Alloy-generated types (BlsServiceManagerEnvelope/BlsServiceManagerSignatureData), not the handler RPC types. Added explicit re-exports in bls.rs following the secp256k1 pattern in rpc.rs.
- send_bls_envelope_signatures() accepts non-rpc BlsServiceHandler::SignatureData to match the canonical SignatureData::Bls12381 variant, and converts to RPC types internally.
- BLS dispatch completely bypasses the From<SignatureData> for ServiceManagerSignatureData conversion (which had an unimplemented!() panic). The secp256k1 path now constructs ServiceManagerSignatureData directly from destructured inner fields.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added BlsServiceManagerEnvelope/BlsServiceManagerSignatureData re-exports**
- **Found during:** Task 2 (BLS dispatch in submit.rs)
- **Issue:** BLS service manager's validate() expects types from its own sol! macro module (bls_service_manager_rpc::IWavsServiceHandler::Envelope/SignatureData), which are private. Cannot use BlsServiceHandlerRpc types.
- **Fix:** Added re-exports in bls.rs: BlsServiceManagerEnvelope and BlsServiceManagerSignatureData, following the same pattern as rpc.rs ServiceManagerEnvelope/ServiceManagerSignatureData.
- **Files modified:** packages/types/src/solidity_types/bls.rs
- **Verification:** cargo check -p wavs exits 0
- **Committed in:** e733883a (Task 2 commit)

**2. [Rule 1 - Bug] Fixed send_bls_envelope_signatures to accept non-rpc SignatureData**
- **Found during:** Task 1 (send_bls_envelope_signatures implementation)
- **Issue:** Plan specified wavs_types::solidity_types::BlsServiceHandler::SignatureData as parameter type, but solidity_types module is private. Also, handleSignedEnvelope on the RPC instance expects BlsServiceHandlerRpc::SignatureData (different Alloy type).
- **Fix:** Accept BlsServiceHandler::SignatureData via the public re-export (wavs_types::BlsServiceHandler::SignatureData), convert internally to BlsServiceHandlerRpc::SignatureData with field-by-field copy.
- **Files modified:** packages/utils/src/evm_client/signing.rs
- **Verification:** cargo build -p utils exits 0, all 55 utils tests pass
- **Committed in:** 387fbd54 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary due to Alloy sol! macro generating distinct types per module. No scope creep.

## Issues Encountered
- Disk space exhaustion during initial build (94.5GB of build artifacts). Resolved with cargo clean.
- utoipa-swagger-ui build failure due to network unavailability (cannot resolve github.com for Swagger UI download). Pre-existing issue unrelated to changes; cargo check succeeds.
- 6 pre-existing wasm_engine test failures (component type mismatch). Verified as pre-existing by testing against base commit.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- BLS submission pipeline complete: aggregation (Plan 01) through on-chain submission (Plan 02)
- Phase 7 complete -- BLS12-381 signature aggregation and EVM submission fully wired
- Ready for Phase 8 or end-to-end testing of BLS flow

## Self-Check: PASSED

All files exist. All commits verified (387fbd54, e733883a).

---
*Phase: 07-bls-aggregation*
*Completed: 2026-03-20*
