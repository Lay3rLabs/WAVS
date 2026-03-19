---
phase: 05-bls-types-and-key-derivation
plan: 02
subsystem: types
tags: [bls12381, signing, enum-migration, serde, secp256k1, WavsCryptoSigner]

# Dependency graph
requires:
  - phase: 05-01
    provides: BLS ABI bindings (BlsServiceHandler::SignatureData), SignatureAlgorithm::Bls12381 variant, SignatureKind::bls_default()
provides:
  - SignatureData enum (Secp256k1/Bls12381 variants) replacing raw Alloy type at crate root
  - WavsSignature enum with tagged serde ("algorithm" discriminator)
  - WavsCryptoSigner enum (Secp256k1(PrivateKeySigner), Bls12381(bls12381::PrivateKey))
  - WavsSigner::sign() accepting &WavsCryptoSigner
  - All workspace call sites migrated to enum variants
affects: [06-bls-signing, 07-bls-aggregation, aggregator, submission, cli]

# Tech tracking
tech-stack:
  added: [commonware-cryptography optional dep in packages/types behind 'bls' feature]
  patterns: [enum-based signature dispatch, pub(crate) raw type with pub enum wrapper, cfg(feature=bls) gating]

key-files:
  created: []
  modified:
    - packages/types/Cargo.toml
    - packages/types/src/signing.rs
    - packages/types/src/signing/signer.rs
    - packages/types/src/lib.rs
    - packages/types/src/solidity_types/not_rpc.rs
    - packages/types/src/solidity_types/rpc.rs
    - packages/types/src/contracts/cosmwasm/service_handler.rs
    - packages/types/src/contracts/cosmwasm/service_manager.rs
    - packages/utils/src/evm_client/signing.rs
    - packages/wavs/src/subsystems/submission.rs
    - packages/wavs/src/subsystems/aggregator/p2p.rs
    - packages/wavs/tests/p2p_broadcast_tests.rs
    - packages/cli/src/main.rs
    - Cargo.toml
    - Cargo.lock

key-decisions:
  - "commonware-cryptography added as optional dep behind 'bls' feature flag to keep wavs-types lightweight for external consumers"
  - "SignatureData raw Alloy type made pub(crate) in solidity_types; enum version is the canonical crate-level export"
  - "Explicit pub use signing::SignatureData in lib.rs to disambiguate glob re-export conflict"
  - "WavsSignature uses #[serde(tag = 'algorithm')] tagged enum -- breaking serialization change documented in code and tested"
  - "WavsCryptoSigner::Bls12381 variant gated behind #[cfg(feature = 'bls')] for conditional compilation"
  - "commonware-cryptography promoted to workspace dependency for consistent versioning across packages"

patterns-established:
  - "Enum-based dispatch: signature types use match on enum variant to select algorithm-specific code paths"
  - "Inner type extraction: EVM contract calls extract Secp256k1(inner) before passing to Alloy-generated methods"
  - "Stub pattern: BLS variants return unimplemented!() with descriptive messages referencing implementation phase"

requirements-completed: [TYPES-02]

# Metrics
duration: 18min
completed: 2026-03-19
---

# Phase 5 Plan 2: Enum-Based Signing Types Summary

**SignatureData, WavsSignature, and WavsCryptoSigner converted to enums with full workspace migration -- secp256k1 path unchanged, BLS stubs ready for Phase 6**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-19T17:41:17Z
- **Completed:** 2026-03-19T17:59:00Z
- **Tasks:** 2
- **Files modified:** 15

## Accomplishments
- Converted SignatureData from raw Alloy re-export to enum with Secp256k1/Bls12381 variants wrapping respective ABI types
- Converted WavsSignature from struct to tagged enum with serde round-trip tests (3 tests: secp256k1, bls12381, old format rejection)
- Added WavsCryptoSigner enum with Secp256k1(PrivateKeySigner) and Bls12381(bls12381::PrivateKey) variants behind feature gate
- Migrated all 7 call sites across 6 packages (types, utils, wavs, cli) to use enum variants
- Full workspace compiles (including Tauri app), 74 lib tests pass (24 wavs-types + 50 utils), clippy clean on all changed crates

## Task Commits

Each task was committed atomically:

1. **Task 1: Convert SignatureData, WavsSignature, WavsCryptoSigner to enums** - `189fce5d` (feat)
2. **Task 2: Migrate all call sites across the workspace** - `89f0bcdc` (feat)
3. **Formatting cleanup** - `4444a949` (chore)

## Files Created/Modified
- `Cargo.toml` - Added commonware-cryptography to workspace dependencies
- `packages/types/Cargo.toml` - Added bls feature flag and optional commonware-cryptography dependency
- `packages/types/src/signing.rs` - SignatureData enum, WavsSignature enum with tagged serde, 3 round-trip tests
- `packages/types/src/signing/signer.rs` - WavsCryptoSigner enum, updated WavsSigner trait to accept enum
- `packages/types/src/lib.rs` - Explicit SignatureData re-export to disambiguate glob conflict
- `packages/types/src/solidity_types/not_rpc.rs` - Raw SignatureData made pub(crate) to avoid crate-level ambiguity
- `packages/types/src/solidity_types/rpc.rs` - Same pub(crate) change for solidity-rpc feature
- `packages/types/src/contracts/cosmwasm/service_handler.rs` - WavsSignatureData now accepts enum, From impls updated
- `packages/types/src/contracts/cosmwasm/service_manager.rs` - Test updated to wrap in SignatureData::Secp256k1
- `packages/utils/src/evm_client/signing.rs` - send_envelope_signatures extracts inner type; tests use WavsCryptoSigner
- `packages/wavs/src/subsystems/submission.rs` - SignerInfo holds WavsCryptoSigner, add_service_key wraps in Secp256k1
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Mock WavsSignature construction migrated to enum variant
- `packages/wavs/tests/p2p_broadcast_tests.rs` - Mock WavsSignature construction migrated to enum variant
- `packages/cli/src/main.rs` - sign() uses WavsCryptoSigner, handleSignedEnvelope extracts inner SignatureData
- `packages/utils/Cargo.toml` - commonware-cryptography updated to workspace = true
- `packages/wavs/Cargo.toml` - commonware-cryptography updated to workspace = true

## Decisions Made
- Used `#[serde(rename_all = "snake_case", tag = "algorithm")]` for WavsSignature enum -- this is a breaking serialization change from the old struct format. Documented in code comments and verified with a test that old format fails to deserialize.
- Added `commonware-cryptography` as a workspace dependency (version 2026.3.0) and updated packages/utils and packages/wavs to use `{ workspace = true }` for consistent versioning.
- WavsCryptoSigner derives Clone to support the existing `.clone()` pattern in submission.rs where SignerInfo is cloned from the signers map.
- The `full` feature in wavs-types includes `bls`, so all internal crates get both variants. External consumers without the feature get Secp256k1-only.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] wavs-cli sign() call site not listed in plan**
- **Found during:** Task 2 (workspace build)
- **Issue:** `packages/cli/src/main.rs` calls `envelope.sign(&signer, ...)` and `handleSignedEnvelope(envelope, signature_data)` -- not listed in plan's files_modified
- **Fix:** Updated CLI to use WavsCryptoSigner::Secp256k1 wrapper and extract inner SignatureData for contract call
- **Files modified:** packages/cli/src/main.rs
- **Verification:** `cargo clippy -p wavs-cli -- -D warnings` passes clean
- **Committed in:** 89f0bcdc (Task 2 commit)

**2. [Rule 3 - Blocking] SignatureData glob re-export conflict**
- **Found during:** Task 1 (packages/types build)
- **Issue:** Both `signing::*` and `solidity_types::*` export `SignatureData` causing ambiguous name error
- **Fix:** Made raw SignatureData `pub(crate)` in not_rpc.rs/rpc.rs and added explicit `pub use signing::SignatureData` in lib.rs
- **Files modified:** packages/types/src/solidity_types/not_rpc.rs, rpc.rs, lib.rs
- **Verification:** `cargo build -p wavs-types --features full` succeeds
- **Committed in:** 189fce5d (Task 1 commit)

**3. [Rule 3 - Blocking] WavsCryptoSigner missing Clone derive**
- **Found during:** Task 2 (wavs build)
- **Issue:** submission.rs clones SignerInfo.signer but WavsCryptoSigner had no Clone impl
- **Fix:** Added `#[derive(Clone)]` to WavsCryptoSigner enum
- **Files modified:** packages/types/src/signing/signer.rs
- **Verification:** `cargo build -p wavs` succeeds
- **Committed in:** 89f0bcdc (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes were necessary to achieve compilation. No scope creep -- these are direct consequences of the enum type migration.

## Issues Encountered
- Pre-existing clippy errors in `packages/wavs/src/subsystems/aggregator/p2p.rs` (10 errors at lines 475-1214) prevented `just lint` from passing on the full workspace. These are NOT caused by our changes -- all changed crates pass clippy individually. Logged to deferred-items.md.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All signing types are now enum-based, ready for Phase 6 (BLS signing) to fill in the `unimplemented!()` stubs
- WavsCryptoSigner::Bls12381 arm in sign() is the primary stub for Phase 6
- WavsSignature::Bls12381 variant in signature_data() is the primary stub for Phase 7 (aggregation)
- Plan 05-03 (BLS key derivation) is already complete -- combined with this plan, Phase 5 is done

## Self-Check: PASSED

---
*Phase: 05-bls-types-and-key-derivation*
*Completed: 2026-03-19*
