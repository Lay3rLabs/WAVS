---
phase: 06-bls-signing-pipeline
verified: 2026-03-20T01:00:00Z
status: passed
score: 12/12 must-haves verified
re_verification: false
---

# Phase 6: BLS Signing Pipeline Verification Report

**Phase Goal:** An operator configured for BLS can sign a submission envelope with its BLS key and propagate the signed submission (BLS signature + G1 pubkey) over P2P, while secp256k1 services continue working unchanged
**Verified:** 2026-03-20T01:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP Success Criteria)

| #  | Truth | Status | Evidence |
|----|-------|--------|---------|
| 1  | BLS-configured service signs envelope digest with DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_` producing 256-byte G2 signature; blst runs on blocking thread pool | VERIFIED | `bls_sign_digest()` in `packages/utils/src/bls_signing.rs` uses exact DST; `WavsSigner::sign()` BLS arm calls `tokio::task::spawn_blocking` in `packages/types/src/signing/signer.rs:134` |
| 2  | `Submission` propagated over P2P carries operator G2 signature and G1 pubkey for BLS services | VERIFIED | `Submission.envelope_signature: WavsSignature` (submission.rs:12) carries `WavsSignature::Bls12381 { g2_signature, g1_pubkey, kind }`; `P2pMessage::from_submission()` JSON-serializes the full Submission including BLS fields |
| 3  | Service configured with `signature_algorithm: secp256k1` produces identical submissions to before | VERIFIED | `add_service_key()` defaults to `Secp256k1` when no Submit::Aggregator config found (`unwrap_or(SignatureAlgorithm::Secp256k1)` at dispatcher.rs:1079); unit test `submission_secp256k1_signer_unchanged` passes |

**Score:** 3/3 success criteria verified

### Must-Have Truths (from Plan 01 and Plan 02 frontmatter)

#### Plan 01 Truths (SIGN-01)

| #  | Truth | Status | Evidence |
|----|-------|--------|---------|
| 1  | `bls_sign_digest()` produces a 256-byte G2 signature from a PrivateKey and 32-byte digest | VERIFIED | Function exists at `packages/utils/src/bls_signing.rs:111`; test `bls_sign_digest_produces_256_bytes` passes |
| 2  | `bls_g2_signature_bytes()` converts blst Signature to 256-byte EIP-2537 format with correct 16-byte zero padding per Fp element | VERIFIED | Function at line 132; test `bls_g2_signature_eip2537_padding` verifies zero bytes at [0..16], [64..80], [128..144], [192..208] |
| 3  | PrivateKey bytes extracted via `commonware_codec::Encode` roundtrip correctly through `blst::SecretKey::from_bytes` | VERIFIED | Test `private_key_roundtrip_through_blst` passes; blst and commonware pubkeys match |
| 4  | `bls` feature is default-on in packages/types | VERIFIED | `packages/types/Cargo.toml:13: default = ["cosmwasm", "bls"]` |
| 5  | `WavsSigner::sign()` BLS arm produces `WavsSignature::Bls12381` with 256-byte g2_signature and 128-byte g1_pubkey via `spawn_blocking` | VERIFIED | `packages/types/src/signing/signer.rs:122-147`; `spawn_blocking` at line 134; returns `WavsSignature::Bls12381 { g2_signature, g1_pubkey, kind }` |

#### Plan 02 Truths (SIGN-02, SIGN-03)

| #  | Truth | Status | Evidence |
|----|-------|--------|---------|
| 6  | `add_service_key()` creates `WavsCryptoSigner::Bls12381` when `SignatureAlgorithm::Bls12381` is passed | VERIFIED | `packages/wavs/src/subsystems/submission.rs:275-289`; creates `WavsCryptoSigner::Bls12381(bls_key)` via `bls_private_key_from_mnemonic` |
| 7  | `add_service_key()` creates `WavsCryptoSigner::Secp256k1` when `SignatureAlgorithm::Secp256k1` is passed | VERIFIED | Lines 262-273; creates `WavsCryptoSigner::Secp256k1(pks)`; test `submission_secp256k1_signer_unchanged` confirms |
| 8  | The dispatcher reads `signature_kind.algorithm` from service workflows and passes it to `add_service_key` | VERIFIED | `packages/wavs/src/dispatcher.rs:1072-1081`; `find_map` over workflows extracting `signature_kind.algorithm`, passed as third arg to `add_service_key` |
| 9  | Submission propagated over P2P carries `WavsSignature::Bls12381 { g2_signature, g1_pubkey, kind }` for BLS services | VERIFIED | `Submission.envelope_signature` is `WavsSignature`; JSON-serialized in `P2pMessage::from_submission()`; BLS variant has `g2_signature: Vec<u8>` and `g1_pubkey: Vec<u8>` |
| 10 | Secp256k1 services produce identical submissions to before | VERIFIED | Algorithm defaults to `Secp256k1` when no Submit::Aggregator; unit test + unchanged code path |
| 11 | `get_service_signer()` returns a graceful response (not panic) for BLS services | VERIFIED | `packages/wavs/src/subsystems/submission.rs:330-338`; returns `SignerResponse::Bls12381 { hd_index, g1_pubkey_hex }` via `bls_g1_pubkey_bytes` |
| 12 | `bls` feature is default-on in packages/wavs/Cargo.toml | VERIFIED | `packages/wavs/Cargo.toml:13: default = ["bls"]` |

**Score:** 12/12 must-haves verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `packages/utils/src/bls_signing.rs` | `bls_sign_digest()`, `bls_g2_signature_bytes()`, `BLS_SIGNING_DST` | VERIFIED | All 3 present; `BLS_SIGNING_DST = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_"` at line 100; 5 new tests present (13 total in file pass) |
| `packages/utils/Cargo.toml` | `commonware-codec` dependency | VERIFIED | Line 69: `commonware-codec = "2026.3.0"` |
| `packages/types/Cargo.toml` | `bls` feature with `blst`, `commonware-codec`, `tokio` optional deps; `bls` in `default` | VERIFIED | Lines 13-15: `default = ["cosmwasm", "bls"]`; `bls = ["dep:commonware-cryptography", "dep:blst", "dep:commonware-codec", "dep:tokio"]`; `blst = { version = "0.3.16", optional = true }` at line 56; `commonware-codec` at line 57 |
| `packages/types/src/signing/signer.rs` | `WavsSigner::sign()` BLS arm with `bls_helpers` module, `bls_sign_digest_inner`, `bls_g1_pubkey_bytes_inner`, `BLS_SIGNING_DST` | VERIFIED | `bls_helpers` module at line 11; `bls_sign_digest_inner` at line 25; `bls_g1_pubkey_bytes_inner` at line 57; `BLS_SIGNING_DST = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_"` at line 21; `spawn_blocking` at line 134 |
| `packages/wavs/src/subsystems/submission.rs` | Algorithm-dispatched `add_service_key()`, graceful `get_service_signer()`, unit tests | VERIFIED | `add_service_key(service_id, hd_index, algorithm: SignatureAlgorithm)` at line 241; BLS arm at line 275; `get_service_signer` BLS arm at line 331; 2 unit tests present and passing |
| `packages/wavs/src/dispatcher.rs` | Algorithm detection from service config, passed to `add_service_key` | VERIFIED | `signature_kind.algorithm` extracted at lines 1072-1079; passed to `add_service_key` at line 1081 |
| `packages/wavs/Cargo.toml` | `bls` feature in `default` | VERIFIED | Line 13: `default = ["bls"]` |
| `packages/types/src/http.rs` | `SignerResponse::Bls12381` variant with `hd_index` and `g1_pubkey_hex` | VERIFIED | Lines 20-25: `Bls12381 { hd_index: u32, g1_pubkey_hex: String }` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `packages/types/src/signing/signer.rs` | `packages/utils/src/bls_signing.rs` | Mirrored `bls_helpers` module using same DST and blst API | VERIFIED | Both files contain identical `BLS_SIGNING_DST = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_"` and same EIP-2537 padding logic |
| `packages/types/src/signing/signer.rs` | `contracts/poa-middleware/contracts/src/bls/libs/HashToCurve.sol` | Matching DST for on-chain verification | VERIFIED | signer.rs uses `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`; Plan 01 confirmed this matches HashToCurve.sol line 20 |
| `packages/wavs/src/dispatcher.rs` | `packages/wavs/src/subsystems/submission.rs` | `add_service_key(service_id, hd_index, algorithm)` | VERIFIED | `dispatcher.rs:1081: submissions.add_service_key(service.id(), hd_index, algorithm)` with 3-arg signature matching `submission.rs:241` |
| `packages/wavs/src/subsystems/submission.rs` | `packages/types/src/signing/signer.rs` | `envelope.sign(&signer, signature_kind)` dispatches to BLS or secp256k1 | VERIFIED | `submission.rs:177-180: envelope.sign(&signer, signature_kind.clone()).await` |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| SIGN-01 | 06-01-PLAN.md | Operator signs envelope digest with BLS key producing 256-byte G2 signature with contract-matching DST | SATISFIED | `bls_sign_digest()` uses `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`; `WavsSigner::sign()` BLS arm via `spawn_blocking`; 13 BLS utility tests pass |
| SIGN-02 | 06-02-PLAN.md | BLS signature and operator G1 pubkey included in Submission propagated over P2P | SATISFIED | `Submission.envelope_signature: WavsSignature::Bls12381 { g2_signature, g1_pubkey, kind }`; JSON-serialized through `P2pMessage::from_submission()` |
| SIGN-03 | 06-02-PLAN.md | Existing secp256k1 signing path unchanged | SATISFIED | Algorithm defaults to `Secp256k1`; unit test `submission_secp256k1_signer_unchanged` passes; secp256k1 signing tests in wavs-types pass |

All 3 requirements SATISFIED. No orphaned requirements found.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `packages/types/src/signing/signer.rs` | 196 | `unimplemented!("BLS signature_data aggregation implemented in Phase 7")` | Info | Expected scope boundary — `signature_data()` aggregation is Phase 7's responsibility (AGG-01 through AGG-04). The `sign()` method (Phase 6's target) is fully implemented. |

No blockers or warnings found. The single `unimplemented!` is in the `signature_data()` aggregation method, which is explicitly scheduled for Phase 7. Phase 6's plan acceptance criteria specifically excluded aggregation.

### Human Verification Required

None. All observable behaviors could be verified through code inspection, test execution, and compilation checks.

### Test Results

| Test Suite | Result | Count |
|------------|--------|-------|
| `cargo test -p utils -- bls_signing` | PASS | 13 tests |
| `cargo test -p wavs-types --features full -- signing` | PASS | 4 tests |
| `cargo test -p wavs -- submission` (unit tests) | PASS | 2 tests (`submission_bls_signer_produces_correct_signature`, `submission_secp256k1_signer_unchanged`) |
| `cargo build -p wavs` | PASS | Compiles clean |
| `cargo build -p wavs-types --features full` | PASS | Compiles clean |

### Verified Commits

| Hash | Description |
|------|-------------|
| `1829ce5e` | test(06-01): add failing tests for BLS signing utilities (RED phase) |
| `f15478c9` | feat(06-01): implement bls_sign_digest and bls_g2_signature_bytes (GREEN phase) |
| `e88a555f` | feat(06-01): implement WavsSigner::sign() BLS arm with helper functions |
| `62b18ffd` | feat(06-02): algorithm-dispatched add_service_key with BLS signer creation and unit tests |
| `6b04aeec` | feat(06-02): add SignerResponse::Bls12381 variant, graceful get_service_signer |

All 5 commits confirmed present in repository.

---

_Verified: 2026-03-20T01:00:00Z_
_Verifier: Claude (gsd-verifier)_
