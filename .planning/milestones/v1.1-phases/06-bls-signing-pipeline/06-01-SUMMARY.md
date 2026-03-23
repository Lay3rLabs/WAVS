---
phase: 06-bls-signing-pipeline
plan: 01
subsystem: crypto
tags: [bls12-381, blst, eip-2537, signing, commonware-codec, hash-to-curve]

# Dependency graph
requires:
  - phase: 05-bls-types-and-key-derivation
    provides: "BLS key derivation (bls_private_key_from_mnemonic), WavsCryptoSigner::Bls12381 variant, WavsSignature::Bls12381 enum"
provides:
  - "bls_sign_digest() - signs 32-byte digest with BLS private key using contract-matching DST"
  - "bls_g2_signature_bytes() - converts blst 192-byte G2 to 256-byte EIP-2537 format"
  - "BLS_SIGNING_DST constant matching HashToCurve.sol"
  - "WavsSigner::sign() BLS arm producing WavsSignature::Bls12381 via spawn_blocking"
  - "bls_helpers module in wavs-types mirroring utils (circular dep workaround)"
  - "bls feature default-on in wavs-types"
affects: [06-02-aggregation, phase-7-bls-submission]

# Tech tracking
tech-stack:
  added: [commonware-codec in packages/utils and packages/types]
  patterns: [mirrored helper functions for circular dep workaround, spawn_blocking for CPU-bound BLS crypto]

key-files:
  created: []
  modified:
    - packages/utils/src/bls_signing.rs
    - packages/utils/Cargo.toml
    - packages/types/Cargo.toml
    - packages/types/src/signing/signer.rs

key-decisions:
  - "Use blst directly (not commonware Signer::sign) for contract-compatible DST (RO vs POP suffix)"
  - "Mirror bls_helpers in wavs-types because circular dep prevents layer-utils import"
  - "commonware_codec::Encode for PrivateKey byte extraction (returns Bytes, deref to &[u8])"
  - "tokio::task::spawn_blocking for CPU-bound BLS signing in async context"
  - "bls feature made default-on in wavs-types so all builds include BLS support"

patterns-established:
  - "BLS helper mirroring: when wavs-types needs utils functionality, duplicate with clear docs linking both implementations"
  - "EIP-2537 padding: each 48-byte Fp element padded to 64 bytes (16 zero prefix + 48 data)"

requirements-completed: [SIGN-01]

# Metrics
duration: 7min
completed: 2026-03-20
---

# Phase 6 Plan 1: BLS Signing Core Summary

**BLS12-381 signing with contract-matching DST (BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_), producing 256-byte EIP-2537 G2 signatures via blst with WavsSigner::sign() BLS arm wired through spawn_blocking**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-20T00:13:47Z
- **Completed:** 2026-03-20T00:20:55Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Implemented bls_sign_digest() and bls_g2_signature_bytes() in packages/utils with contract-matching DST
- Verified PrivateKey byte roundtrip through blst SecretKey via commonware-codec Encode
- Wired WavsSigner::sign() BLS arm with bls_helpers module (mirrored due to circular dep) using spawn_blocking
- Made bls feature default-on in wavs-types; all 13 BLS utility tests and 4 signing tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: BLS signing utilities (RED)** - `1829ce5e` (test)
2. **Task 1: BLS signing utilities (GREEN)** - `f15478c9` (feat)
3. **Task 2: Feature flags + WavsSigner::sign() BLS arm** - `e88a555f` (feat)

_Note: Task 1 used TDD with RED/GREEN phases_

## Files Created/Modified
- `packages/utils/src/bls_signing.rs` - Added BLS_SIGNING_DST, bls_sign_digest(), bls_g2_signature_bytes(), and 5 new tests
- `packages/utils/Cargo.toml` - Added commonware-codec dependency
- `packages/types/Cargo.toml` - Added blst, commonware-codec, tokio optional deps; bls feature now default-on
- `packages/types/src/signing/signer.rs` - Added bls_helpers module; replaced unimplemented!() BLS arm with spawn_blocking signing

## Decisions Made
- Used blst directly rather than commonware's Signer::sign because commonware uses _POP_ DST suffix while the contract uses _RO_ suffix, and commonware wraps messages with union_unique
- Mirrored helper functions in wavs-types bls_helpers module because wavs-types cannot depend on layer-utils (circular dependency: layer-utils -> wavs-types)
- Used commonware_codec::Encode trait to extract PrivateKey bytes (returns Bytes which derefs to &[u8] for blst SecretKey::from_bytes)
- Made tokio an optional dependency of wavs-types behind bls feature for spawn_blocking support

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added tokio as optional dep in wavs-types**
- **Found during:** Task 2 (WavsSigner::sign() BLS arm)
- **Issue:** tokio::task::spawn_blocking requires tokio as a regular dependency, but it was only in dev-dependencies
- **Fix:** Added `tokio = { workspace = true, optional = true }` to [dependencies] and included `"dep:tokio"` in bls feature
- **Files modified:** packages/types/Cargo.toml
- **Verification:** cargo build -p wavs-types --features full succeeds
- **Committed in:** e88a555f (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary for spawn_blocking to compile outside test context. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- BLS signing core is complete; operators can now produce G2 signatures matching the poa-middleware contract's hash-to-curve DST
- Ready for Plan 06-02: BLS signature aggregation and submission
- bls_helpers pattern established for future wavs-types BLS work

## Self-Check: PASSED

All files exist, all commits verified (1829ce5e, f15478c9, e88a555f).

---
*Phase: 06-bls-signing-pipeline*
*Completed: 2026-03-20*
