---
phase: 01-secure-peer-connectivity
plan: 01
subsystem: p2p
tags: [ed25519, commonware, bip39, identity, cryptography]

# Dependency graph
requires: []
provides:
  - Ed25519 identity derivation from BIP-39 mnemonic via ChaCha20Rng
  - P2pConfig enum with commonware-tailored fields (peer_addresses, bootstrappers, authorized_peers)
  - commonware-p2p, commonware-cryptography, commonware-runtime, commonware-math dependencies
affects: [01-02-PLAN, 01-03-PLAN, phase-02]

# Tech tracking
tech-stack:
  added: [commonware-p2p 2026.3.0, commonware-cryptography 2026.3.0, commonware-runtime 2026.3.0, commonware-math 2026.3.0, rand_chacha 0.3, bip39]
  patterns: [deterministic Ed25519 from BIP-39 via ChaCha20Rng, P2pConfig with authorized_peers]

key-files:
  created:
    - packages/wavs/tests/p2p_identity_tests.rs
  modified:
    - packages/wavs/Cargo.toml
    - packages/wavs/src/subsystems/aggregator/p2p.rs
    - packages/layer-tests/src/e2e/config.rs
    - packages/layer-tests/src/e2e/handles.rs

key-decisions:
  - "Used rand_chacha 0.3 (not 0.9) to match commonware-cryptography's rand_core 0.6 dependency"
  - "Added commonware-math as direct dependency for Random trait needed by PrivateKey::random()"

patterns-established:
  - "Ed25519 identity: ed25519_signer_from_mnemonic() using ChaCha20Rng from BIP-39 seed[..32]"
  - "P2pConfig authorized_peers: flat Vec<String> of hex-encoded Ed25519 pubkeys"
  - "commonware trait imports: Signer from commonware_cryptography, Random from commonware_math::algebra"

requirements-completed: [IDEN-01, IDEN-02]

# Metrics
duration: 13min
completed: 2026-03-17
---

# Phase 1 Plan 01: Ed25519 Identity and P2pConfig Foundation Summary

**Ed25519 identity derivation from BIP-39 mnemonic via commonware-cryptography with ChaCha20Rng seeding, and P2pConfig rewritten with authorized_peers, peer_addresses, and bootstrappers fields**

## Performance

- **Duration:** 13 min
- **Started:** 2026-03-17T15:05:37Z
- **Completed:** 2026-03-17T15:18:55Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Replaced libp2p secp256k1 identity derivation with Ed25519 via commonware-cryptography, producing deterministic keys from BIP-39 mnemonics (IDEN-01, IDEN-02)
- Deleted ~1,600 lines of libp2p networking code (build_swarm, run_event_loop, WavsBehaviour, CatchUpCodec, EventLoopState, GossipSub config methods, etc.)
- Rewrote P2pConfig enum with commonware-tailored fields: peer_addresses for lookup mode, bootstrappers for discovery mode, authorized_peers for Oracle peer authorization
- Added commonware-p2p, commonware-cryptography, commonware-runtime, commonware-math dependencies at version 2026.3.0
- 8 passing integration tests covering identity derivation and config structure

## Task Commits

Each task was committed atomically:

1. **Task 1: Add commonware deps, implement Ed25519 identity, write tests (TDD)**
   - `9c5229af` (test: add failing Ed25519 identity derivation tests)
   - `3e101d79` (feat: implement Ed25519 identity derivation via commonware-cryptography)
2. **Task 2: Rewrite P2pConfig enum with commonware-tailored fields (TDD)**
   - `abfd9e44` (test: add failing P2pConfig deserialization tests)
   - `8cc0446f` (feat: rewrite P2pConfig with commonware-tailored fields)

## Files Created/Modified
- `packages/wavs/tests/p2p_identity_tests.rs` - 8 integration tests for identity derivation (IDEN-01, IDEN-02) and P2pConfig structure
- `packages/wavs/Cargo.toml` - Added commonware-p2p, commonware-cryptography, commonware-runtime, commonware-math, rand_chacha, bip39 dependencies
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Replaced from 1,839 lines to ~213 lines: new Ed25519 identity functions, new P2pConfig enum, placeholder P2pHandle
- `packages/layer-tests/src/e2e/config.rs` - Updated P2pConfig construction to use new field names
- `packages/layer-tests/src/e2e/handles.rs` - Updated P2pConfig destructuring to use new field names

## Decisions Made
- Used `rand_chacha` 0.3 instead of 0.9 because commonware-cryptography depends on `rand_core` 0.6, and `rand_chacha` 0.9 uses `rand_core` 0.9 -- the CryptoRngCore trait mismatch causes compilation failure with two different versions of the same trait
- Added `commonware-math` as a direct dependency because the `Random` trait (required for `ed25519::PrivateKey::random()`) lives in `commonware_math::algebra`, not re-exported by commonware-cryptography
- Imported `Signer` trait from commonware-cryptography for `public_key()` method access, and `Random` trait from commonware-math for `random()` method access -- these are required by commonware's trait-based API design

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] rand_chacha version mismatch with commonware**
- **Found during:** Task 1 (Ed25519 identity implementation)
- **Issue:** Plan specified `rand_chacha = "0.9"` but commonware-cryptography uses `rand_core` 0.6. The `CryptoRngCore` trait from 0.6 and 0.9 are different types, causing "two types from two different versions" compilation error.
- **Fix:** Changed to `rand_chacha = "0.3"` which depends on `rand_core` 0.6, matching commonware's expected trait version. Used `rand_chacha::rand_core::SeedableRng` instead of `rand::SeedableRng`.
- **Files modified:** `packages/wavs/Cargo.toml`, `packages/wavs/src/subsystems/aggregator/p2p.rs`
- **Verification:** `cargo check -p wavs` compiles, all tests pass
- **Committed in:** 3e101d79

**2. [Rule 3 - Blocking] Missing commonware-math dependency for Random trait**
- **Found during:** Task 1 (Ed25519 identity implementation)
- **Issue:** `ed25519::PrivateKey::random()` requires `Random` trait from `commonware_math::algebra`, which is not re-exported by commonware-cryptography. Compilation error: "function or associated item named `random` found but trait not in scope".
- **Fix:** Added `commonware-math = "2026.3.0"` as direct dependency and imported the trait.
- **Files modified:** `packages/wavs/Cargo.toml`, `packages/wavs/src/subsystems/aggregator/p2p.rs`
- **Verification:** `cargo check -p wavs` compiles, all tests pass
- **Committed in:** 3e101d79

**3. [Rule 3 - Blocking] Updated layer-tests to match new P2pConfig fields**
- **Found during:** Task 2 (P2pConfig rewrite)
- **Issue:** `packages/layer-tests/src/e2e/config.rs` and `handles.rs` destructure P2pConfig with old field names (bootstrap_nodes, max_retry_duration_secs, etc.), causing compilation errors.
- **Fix:** Updated both files to use new field names (bootstrappers, peer_addresses, authorized_peers).
- **Files modified:** `packages/layer-tests/src/e2e/config.rs`, `packages/layer-tests/src/e2e/handles.rs`
- **Verification:** `cargo check -p wavs` compiles
- **Committed in:** 8cc0446f

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes necessary for compilation. No scope creep.

## Issues Encountered
None beyond the deviations documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Ed25519 identity foundation complete, ready for Plans 02 and 03
- P2pConfig enum defines the contract for commonware-p2p networking configuration
- P2pHandle::new returns placeholder error for non-Disabled configs -- Plans 02 and 03 will implement the actual networking
- All commonware dependencies are available in the workspace

---
*Phase: 01-secure-peer-connectivity*
*Completed: 2026-03-17*
