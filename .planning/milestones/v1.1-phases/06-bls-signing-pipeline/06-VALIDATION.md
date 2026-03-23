---
phase: 6
slug: bls-signing-pipeline
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-20
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust unit tests) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p layer-utils -- bls_signing` |
| **Full suite command** | `cargo test -p layer-utils && cargo test -p wavs` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p layer-utils -- bls_signing`
- **After every plan wave:** Run `cargo test -p layer-utils && cargo test -p wavs`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 6-01-01 | 01 | 1 | SIGN-01 | unit | `cargo test -p layer-utils -- bls_sign_digest_produces_256_bytes` | ✅ created in plan | ⬜ pending |
| 6-01-02 | 01 | 1 | SIGN-01 | unit | `cargo test -p layer-utils -- private_key_roundtrip_through_blst` | ✅ created in plan | ⬜ pending |
| 6-01-03 | 01 | 1 | SIGN-01 | unit | `cargo test -p layer-utils -- bls_g2_signature_eip2537_padding` | ✅ created in plan | ⬜ pending |
| 6-02-01 | 02 | 2 | SIGN-02 | unit | `cargo test -p wavs -- submission_bls_signer_produces_correct_signature` | ✅ created in plan | ⬜ pending |
| 6-02-02 | 02 | 2 | SIGN-02 | unit | `cargo test -p wavs -- submission_secp256k1_signer_unchanged` | ✅ created in plan | ⬜ pending |
| 6-03-01 | 02 | 2 | SIGN-03 | unit | `cargo test -p wavs -- submission_secp256k1_signer_unchanged` | ✅ created in plan | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Tests are created inline by the TDD tasks in the plans (not as separate Wave 0 stubs):

- `packages/utils/src/bls_signing.rs` — tests `bls_sign_digest_produces_256_bytes`, `private_key_roundtrip_through_blst`, `bls_g2_signature_eip2537_padding` created by Plan 06-01 Task 1 (RED phase) before implementation (SIGN-01)
- `packages/wavs/src/subsystems/submission.rs` — tests `submission_bls_signer_produces_correct_signature`, `submission_secp256k1_signer_unchanged` created by Plan 06-02 Task 1 (SIGN-02, SIGN-03)

*Wave 0 pattern: TDD red-phase creates tests inline before GREEN phase implements the code.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| P2P propagation of BLS Submission message | SIGN-02 | Requires live multi-operator network setup | Run two operators, trigger BLS service, verify Submission message received by peer contains `bls_signature` and `bls_pubkey` fields |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (TDD inline creation pattern)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** 2026-03-20
