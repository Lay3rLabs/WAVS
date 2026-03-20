---
phase: 8
slug: integration-and-verification
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust integration tests (cargo test) |
| **Config file** | `packages/layer-tests/layer-tests.toml` |
| **Quick run command** | `cargo test -p layer-tests -- bls` |
| **Full suite command** | `cargo test -p layer-tests` |
| **Estimated runtime** | ~120 seconds (full E2E with anvil) |

---

## Sampling Rate

- **After every task commit:** Run `cargo build -p layer-tests 2>&1 | grep error` (compile check)
- **After every plan wave:** Run `cargo test -p layer-tests -- bls`
- **Before `/gsd:verify-work`:** Full suite must be green (`cargo test -p layer-tests`)
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 8-01-01 | 01 | 1 | INT-01 | compile | `cargo build -p layer-tests` | ❌ W0 | ⬜ pending |
| 8-01-02 | 01 | 1 | INT-01 | compile | `cargo build -p layer-tests` | ❌ W0 | ⬜ pending |
| 8-01-03 | 01 | 1 | INT-01 | integration | `cargo test -p layer-tests -- bls_e2e` | ❌ W0 | ⬜ pending |
| 8-02-01 | 02 | 2 | INT-02 | integration | `cargo test -p layer-tests -- evm_multi_operator` | ✅ | ⬜ pending |
| 8-02-02 | 02 | 2 | INT-02 | integration | `cargo test -p layer-tests` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `packages/layer-tests/src/e2e/tests/bls_e2e.rs` — BLS E2E test stub
- [ ] `packages/layer-tests/src/e2e/services.rs` — `EvmService::BlsMultiOperator` variant added
- [ ] `examples/contracts/SimpleBlsSubmit.sol` — BLS-compatible submit contract

*Wave 0 covers new test infrastructure; existing secp256k1 tests already exist.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Pairing check passes on-chain | INT-01 | Requires live EIP-2537 precompile call and on-chain verification | Run `cargo test -p layer-tests -- bls_e2e -- --nocapture` and confirm "pairing check passed" in output |
| No secp256k1 regressions | INT-02 | Full suite run with human review of output | Run `cargo test -p layer-tests` and confirm all existing tests pass |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
