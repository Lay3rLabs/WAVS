---
phase: 15
slug: subscription-protocol
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-03
---

# Phase 15 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test framework (cargo test) |
| **Config file** | `Cargo.toml` workspace |
| **Quick run command** | `cargo test -p wavs --lib -- p2p_broadcast_tests` |
| **Full suite command** | `cargo test -p wavs` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p wavs --lib -- p2p_broadcast_tests`
- **After every plan wave:** Run `cargo test -p wavs`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 15-01-01 | 01 | 1 | ANN-01 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_subscribe_builds_announcement` | ❌ W0 | ⬜ pending |
| 15-01-01 | 01 | 1 | ANN-02 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_unsubscribe_builds_announcement` | ❌ W0 | ⬜ pending |
| 15-01-01 | 01 | 1 | ANN-03 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_heartbeat_subscription_announcement` | ❌ W0 | ⬜ pending |
| 15-01-01 | 01 | 1 | ANN-04 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_hello_on_first_contact` | ❌ W0 | ⬜ pending |
| 15-01-01 | 01 | 1 | ANN-03 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_set_peer_subscriptions_replaces` | ❌ W0 | ⬜ pending |
| 15-01-01 | 01 | 1 | COMPAT-03 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_has_announced_compat03` | ❌ W0 | ⬜ pending |
| 15-01-01 | 01 | 1 | ANN-03 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_full_state_serde_default` | ❌ W0 | ⬜ pending |
| 15-01-01 | 01 | 1 | ANN-01 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_incremental_vs_full_state_processing` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- All new tests extend the existing `#[cfg(test)] mod p2p_broadcast_tests` module in p2p.rs
- No new test files or framework installs needed
- Existing 22 tests in `p2p_broadcast_tests` must continue passing (regression check)

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end subscription announcement exchange between nodes | ANN-01..04 | Requires running multi-node P2P network | Run E2E tests via `cargo test -p layer-tests` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
