---
phase: 06-bls-signing-pipeline
plan: 02
subsystem: crypto
tags: [bls12-381, submission-pipeline, signer-dispatch, feature-flags, http-api]

# Dependency graph
requires:
  - phase: 06-bls-signing-pipeline-01
    provides: "BLS key derivation, bls_sign_digest(), WavsCryptoSigner::Bls12381, WavsSignature::Bls12381"
provides:
  - "Algorithm-dispatched add_service_key() creating secp256k1 or BLS signers based on service config"
  - "Dispatcher auto-detects signature algorithm from Submit::Aggregator { signature_kind }"
  - "SignerResponse::Bls12381 variant for graceful HTTP API responses"
  - "get_service_signer() returns G1 pubkey hex for BLS services instead of panicking"
  - "bls feature default-on in packages/wavs/Cargo.toml"
affects: [phase-7-bls-submission, phase-8-bls-aggregation]

# Tech tracking
tech-stack:
  added: []
  patterns: [algorithm-dispatched signer creation, graceful enum variant expansion across packages]

key-files:
  created: []
  modified:
    - packages/wavs/src/subsystems/submission.rs
    - packages/wavs/src/dispatcher.rs
    - packages/wavs/Cargo.toml
    - packages/types/src/http.rs
    - packages/wavs-mcp/src/server.rs
    - packages/layer-tests/src/e2e/service_managers.rs

key-decisions:
  - "bls feature in packages/wavs is purely a gating flag; wavs-types already provides BLS types via features = [\"full\"]"
  - "cfg(not(feature = \"bls\")) arm returns FailedToCreateEvmSigner error rather than compile error for graceful degradation"
  - "get_service_signer uses and_then instead of map to allow fallible BLS G1 pubkey extraction"
  - "BLS logging uses truncated G1 pubkey hex (first 16 chars) for readable display"

patterns-established:
  - "Algorithm dispatch: match on SignatureAlgorithm enum to select signer creation path"
  - "Enum variant expansion: when adding variants to shared types, update all match sites across workspace packages"

requirements-completed: [SIGN-02, SIGN-03]

# Metrics
duration: 12min
completed: 2026-03-20
---

# Phase 6 Plan 2: BLS Submission Pipeline Wiring Summary

**Algorithm-dispatched add_service_key() with BLS signer creation, dispatcher auto-detection from service config, SignerResponse::Bls12381 for graceful HTTP API, bls feature default-on in packages/wavs**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-20T00:24:55Z
- **Completed:** 2026-03-20T00:37:31Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Updated add_service_key() to dispatch on SignatureAlgorithm, creating either secp256k1 or BLS12-381 signers
- Dispatcher now auto-detects algorithm from service workflow Submit::Aggregator config with secp256k1 default for backward compatibility
- Replaced unimplemented!() in get_service_signer with SignerResponse::Bls12381 variant returning G1 pubkey hex
- Added bls feature (default-on) to packages/wavs/Cargo.toml ensuring all WAVS builds include BLS support
- Unit tests verify BLS signer produces 128-byte G1 pubkey and 256-byte G2 signature

## Task Commits

Each task was committed atomically:

1. **Task 1: Algorithm-dispatched add_service_key, dispatcher integration, and bls feature default-on** - `62b18ffd` (feat)
2. **Task 2: SignerResponse BLS variant and get_service_signer graceful handling** - `6b04aeec` (feat)

## Files Created/Modified
- `packages/wavs/Cargo.toml` - Added bls feature (default-on) to features section
- `packages/wavs/src/subsystems/submission.rs` - Algorithm-dispatched add_service_key(), graceful get_service_signer for BLS, unit tests
- `packages/wavs/src/dispatcher.rs` - Algorithm detection from service config, SignerResponse variant matching
- `packages/types/src/http.rs` - Added SignerResponse::Bls12381 { hd_index, g1_pubkey_hex } variant
- `packages/wavs-mcp/src/server.rs` - Updated 5 match sites for exhaustive SignerResponse pattern matching
- `packages/layer-tests/src/e2e/service_managers.rs` - Updated irrefutable let pattern to match both variants

## Decisions Made
- bls feature in packages/wavs is a gating flag (wavs-types already provides BLS types via features = ["full"])
- cfg(not(feature = "bls")) arm returns a descriptive error rather than compile-time error
- get_service_signer uses and_then instead of map to allow fallible BLS G1 pubkey extraction
- BLS display uses truncated G1 pubkey hex (first 16 chars) prefixed with "BLS:" for readable logging

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated wavs-mcp and layer-tests for exhaustive SignerResponse matching**
- **Found during:** Task 2 (SignerResponse::Bls12381 variant addition)
- **Issue:** Adding Bls12381 variant to SignerResponse made existing pattern matches in wavs-mcp (5 sites) and layer-tests (1 site) non-exhaustive, causing compilation failures
- **Fix:** Added Bls12381 match arms to all affected sites in packages/wavs-mcp/src/server.rs and packages/layer-tests/src/e2e/service_managers.rs
- **Files modified:** packages/wavs-mcp/src/server.rs, packages/layer-tests/src/e2e/service_managers.rs
- **Verification:** cargo build -p wavs -p wavs-mcp -p layer-tests succeeds
- **Committed in:** 6b04aeec (Task 2 commit)

**2. [Rule 3 - Blocking] Updated dispatcher change_service for refutable pattern**
- **Found during:** Task 2 (SignerResponse enum expansion)
- **Issue:** dispatcher.rs line 879 used irrefutable `let SignerResponse::Secp256k1 { hd_index, .. }` which became refutable with new variant
- **Fix:** Replaced with match expression extracting hd_index from either variant
- **Files modified:** packages/wavs/src/dispatcher.rs
- **Verification:** cargo build -p wavs succeeds
- **Committed in:** 6b04aeec (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes required for compilation after enum variant expansion. No scope creep.

## Issues Encountered
- Pre-existing clippy errors in packages/wavs/src/subsystems/aggregator/p2p.rs (10 warnings treated as errors) -- confirmed pre-existing on clean tree, not caused by this plan's changes. Out of scope per deviation rules.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- BLS submission pipeline is fully wired: services configured with BLS algorithm will create BLS signers and produce BLS-signed submissions
- Secp256k1 services are completely unchanged (backward compatible)
- Ready for Phase 7: BLS signature aggregation and on-chain submission
- All unimplemented!() for BLS in submission.rs removed

## Self-Check: PASSED

All files exist, all commits verified (62b18ffd, 6b04aeec). All 15 acceptance criteria pass.

---
*Phase: 06-bls-signing-pipeline*
*Completed: 2026-03-20*
