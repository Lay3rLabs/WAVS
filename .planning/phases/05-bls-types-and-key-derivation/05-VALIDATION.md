---
phase: 5
slug: bls-types-and-key-derivation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 5 -- Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + cargo test |
| **Config file** | Workspace Cargo.toml |
| **Quick run command** | `cargo test -p utils --lib bls_signing` |
| **Full suite command** | `cargo test -p utils -p wavs-types` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build -p wavs-types -p utils` (type check) + `cargo test -p utils --lib bls_signing -p wavs-types --lib` (unit tests)
- **After every plan wave:** Run `cargo test -p utils -p wavs-types -p wavs`
- **Before `/gsd:verify-work`:** `just lint && cargo test -p utils -p wavs-types -p wavs`
- **Max feedback latency:** ~30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 5-01-01 | 01 | 1 | TYPES-01, TYPES-03 | unit | `cargo test -p wavs-types --lib -- bls` | N/A (inline) | pending |
| 5-01-02 | 01 | 1 | TYPES-01 | unit | `cargo test -p wavs-types --lib -- signature_algorithm` | N/A (inline) | pending |
| 5-02-01 | 02 | 2 | TYPES-02 | unit + build | `cargo build -p wavs-types && cargo test -p wavs-types --lib -- wavs_signature` | N/A (inline) | pending |
| 5-02-02 | 02 | 2 | TYPES-02 | build + lint | `cargo build && cargo test -p wavs-types -p utils --lib && just lint` | N/A | pending |
| 5-03-01 | 03 | 1 | KEYS-01, KEYS-02 | build | `cargo check -p utils && cargo tree -p utils -i rand_chacha 2>&1 \| head -5` | N/A | pending |
| 5-03-02 | 03 | 1 | KEYS-01, KEYS-02 | unit | `cargo test -p utils --lib bls_signing` | N/A (inline) | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `packages/utils/src/bls_signing.rs` -- stubs for KEYS-01, KEYS-02 (new file with `#[cfg(test)]` module)
- [ ] BLS binding compile test in `packages/types` -- covers TYPES-03 (inline `#[test]`)
- [ ] `SignatureData` enum serde test in `packages/types` -- covers TYPES-02 (inline `#[test]`)
- [ ] `WavsSignature` serde round-trip tests in `packages/types` -- covers TYPES-02 serialization change (inline `#[test]`)
- [ ] `SignatureAlgorithm` serde test including `Bls12381` -- covers TYPES-01 (inline `#[test]`)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WIT interface valid in wavs-wasi toolchain | TYPES-01 | Requires full WASM build toolchain | Run `just wasi-build-native echo` and check no WIT parse error |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
