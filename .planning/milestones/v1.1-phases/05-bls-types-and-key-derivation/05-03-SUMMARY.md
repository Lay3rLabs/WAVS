---
phase: 05-bls-types-and-key-derivation
plan: 03
subsystem: crypto
tags: [bls12381, hkdf, key-derivation, eip-2537, commonware, blst]

# Dependency graph
requires:
  - phase: 05-bls-types-and-key-derivation (plan 01)
    provides: BLS ABI bindings and SignatureAlgorithm::Bls12381 variant
provides:
  - bls_private_key_from_mnemonic() for deterministic BLS key derivation from mnemonic + HD index
  - bls_g1_pubkey_bytes() for 128-byte EIP-2537 G1 public key conversion
  - HKDF-SHA256 domain-separated key derivation pattern for BLS
affects: [06-bls-key-lifecycle, 07-bls-signing-aggregation, 08-on-chain-bls-registration]

# Tech tracking
tech-stack:
  added: [commonware-cryptography 2026.3.0, commonware-math 2026.3.0, hkdf 0.12, blst 0.3.16, rand_chacha 0.3, rand_core 0.6, sha2]
  patterns: [HKDF-SHA256 domain separation for HD key derivation, blst FFI for G1 decompression, EIP-2537 128-byte padding]

key-files:
  created: [packages/utils/src/bls_signing.rs]
  modified: [packages/utils/Cargo.toml, packages/utils/src/lib.rs]

key-decisions:
  - "Used HKDF-SHA256 with domain-separated info label (WAVS-BLS-KEY-v1 || hd_index LE) instead of raw seed slicing for HD index incorporation"
  - "Used blst FFI (blst_p1_uncompress + blst_p1_affine_serialize) for compressed-to-uncompressed G1 conversion"
  - "Pinned rand_chacha to 0.3 (not 0.9) to match commonware's rand_core 0.6 requirement"
  - "Made bip39 non-optional in packages/utils since BLS key derivation requires it unconditionally"
  - "Used Deref<Target=[u8]> for PublicKey byte access to avoid ambiguous AsRef resolution"

patterns-established:
  - "BLS key derivation: mnemonic -> BIP-39 seed -> HKDF-SHA256(info=domain||index) -> ChaCha20Rng -> PrivateKey::random()"
  - "EIP-2537 G1 padding: 16 zero bytes + 48-byte coordinate, repeated for x and y = 128 bytes total"

requirements-completed: [KEYS-01, KEYS-02]

# Metrics
duration: 15min
completed: 2026-03-19
---

# Phase 5 Plan 3: BLS Key Derivation Summary

**Deterministic BLS12-381 key derivation from mnemonic+HD index using HKDF-SHA256, with G1 pubkey conversion to 128-byte EIP-2537 format via blst FFI**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-19T17:19:10Z
- **Completed:** 2026-03-19T17:35:06Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Deterministic BLS private key derivation from any BIP-39 mnemonic with HD index separation via HKDF-SHA256
- G1 public key conversion from 48-byte ZCash compressed to 128-byte EIP-2537 uncompressed format
- 8 unit tests covering determinism, index uniqueness, input validation, and output format
- Clean dependency integration with no version conflicts (rand_chacha 0.3, commonware 2026.3.0)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add BLS dependencies** - `ef826f4b` (chore)
2. **Task 2 RED: Failing tests** - `b7123eb1` (test)
3. **Task 2 GREEN: Implementation** - `6320b429` (feat)

_Note: TDD task has separate RED and GREEN commits_

## Files Created/Modified
- `packages/utils/src/bls_signing.rs` - BLS key derivation module with bls_private_key_from_mnemonic() and bls_g1_pubkey_bytes()
- `packages/utils/Cargo.toml` - Added BLS dependencies (commonware-cryptography, hkdf, blst, etc.)
- `packages/utils/src/lib.rs` - Registered bls_signing module
- `Cargo.lock` - Updated with new dependency resolutions

## Decisions Made
- Used HKDF-SHA256 with domain-separated info (WAVS-BLS-KEY-v1 || hd_index.to_le_bytes()) instead of raw seed slicing, providing cryptographic guarantees against cross-domain key reuse
- Pinned rand_chacha to 0.3 to match commonware's rand_core 0.6 -- using 0.9 would cause CryptoRngCore trait incompatibility
- Made bip39 a non-optional dependency (was optional under test-utils feature) since BLS key derivation is a production codepath
- Used blst FFI directly for G1 decompression rather than implementing curve math, matching the transitive blst 0.3.16 already in Cargo.lock via commonware

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed ambiguous AsRef resolution on PublicKey**
- **Found during:** Task 2 (TDD RED phase)
- **Issue:** `public_key().as_ref()` in tests was ambiguous because commonware's PublicKey implements both `AsRef<[u8]>` and `AsRef<MinPk::Public>`
- **Fix:** Created `pubkey_bytes()` helper using `Deref<Target=[u8]>` coercion instead of `as_ref()`
- **Files modified:** packages/utils/src/bls_signing.rs
- **Verification:** All 8 tests compile and pass
- **Committed in:** 6320b429 (Task 2 GREEN commit)

**2. [Rule 3 - Blocking] Fixed clippy explicit_auto_deref warning**
- **Found during:** Task 2 (post-implementation lint check)
- **Issue:** `&*pubkey` explicit deref flagged by clippy with -D warnings
- **Fix:** Changed to `&pubkey` (auto-deref via Deref trait)
- **Files modified:** packages/utils/src/bls_signing.rs
- **Verification:** `cargo clippy -p utils -- -D warnings` passes cleanly
- **Committed in:** 6320b429 (Task 2 GREEN commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes were necessary for compilation and lint compliance. No scope creep.

## Issues Encountered
- The `commonware_cryptography::Signer` trait must be imported to call `public_key()` on PrivateKey -- not immediately obvious from the plan's interface docs but documented in commonware's own examples
- Pre-existing uncommitted changes from Plan 05-01 were in the working tree but did not interfere with Plan 05-03 execution

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- BLS key derivation foundation complete for Phase 6 (BLS Key Lifecycle) to build per-service key management
- The `bls_g1_pubkey_bytes()` output format matches `BLS12381.G1_POINT_SIZE = 128` for Phase 8 on-chain registration
- HKDF domain separation pattern is established for consistent key derivation across the codebase

## Self-Check: PASSED

All created files verified present. All commit hashes verified in git log.

---
*Phase: 05-bls-types-and-key-derivation*
*Completed: 2026-03-19*
