---
phase: 6
slug: bls-signing-pipeline
status: draft
nyquist_compliant: false
wave_0_complete: false
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
| **Quick run command** | `cargo test -p layer-utils bls` |
| **Full suite command** | `cargo test -p layer-utils && cargo test -p wavs` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p layer-utils bls`
- **After every plan wave:** Run `cargo test -p layer-utils && cargo test -p wavs`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 6-01-01 | 01 | 1 | SIGN-01 | unit | `cargo test -p layer-utils bls_sign_envelope` | ❌ W0 | ⬜ pending |
| 6-01-02 | 01 | 1 | SIGN-01 | unit | `cargo test -p layer-utils bls_private_key_roundtrip` | ❌ W0 | ⬜ pending |
| 6-01-03 | 01 | 1 | SIGN-01 | unit | `cargo test -p layer-utils bls_g2_signature_bytes` | ❌ W0 | ⬜ pending |
| 6-02-01 | 02 | 2 | SIGN-02 | unit | `cargo test -p wavs submission_bls` | ❌ W0 | ⬜ pending |
| 6-02-02 | 02 | 2 | SIGN-02 | unit | `cargo test -p wavs submission_secp256k1_unchanged` | ❌ W0 | ⬜ pending |
| 6-03-01 | 03 | 2 | SIGN-03 | unit | `cargo test -p wavs secp256k1_path_no_regression` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `packages/utils/src/bls.rs` — test stubs for `bls_sign_envelope`, `bls_private_key_roundtrip`, `bls_g2_signature_bytes` (SIGN-01)
- [ ] `packages/wavs/src/subsystems/submission/` — test stubs for `submission_bls`, `submission_secp256k1_unchanged` (SIGN-02)
- [ ] `packages/wavs/src/subsystems/submission/` — test stub for `secp256k1_path_no_regression` (SIGN-03)

*All Wave 0 items are test stubs — no framework install needed (cargo test already present).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| P2P propagation of BLS Submission message | SIGN-02 | Requires live multi-operator network setup | Run two operators, trigger BLS service, verify Submission message received by peer contains `bls_signature` and `bls_pubkey` fields |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
