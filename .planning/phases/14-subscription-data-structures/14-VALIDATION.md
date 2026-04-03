---
phase: 14
slug: subscription-data-structures
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-03
---

# Phase 14 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test framework (cargo test) |
| **Config file** | `Cargo.toml` workspace |
| **Quick run command** | `cargo test -p wavs --lib -- p2p_broadcast_tests` |
| **Full suite command** | `cargo test -p wavs` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p wavs --lib -- p2p_broadcast_tests`
- **After every plan wave:** Run `cargo test -p wavs`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 14-01-01 | 01 | 1 | SUB-01 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_map_forward_index` | ❌ W0 | ⬜ pending |
| 14-01-02 | 01 | 1 | SUB-02 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_map_remove_peer` | ❌ W0 | ⬜ pending |
| 14-01-03 | 01 | 1 | SUB-03 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_map_disconnect_cleanup` | ❌ W0 | ⬜ pending |
| 14-01-04 | 01 | 1 | ANN-05 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_subscription_announcement_roundtrip` | ❌ W0 | ⬜ pending |
| 14-01-05 | 01 | 1 | ANN-05 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_subscription_sentinel_distinguishable` | ❌ W0 | ⬜ pending |
| 14-01-06 | 01 | 1 | SUB-01 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_get_recipients_empty_fallback` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- All tests are new and will be written alongside the data structures in `packages/wavs/src/subsystems/aggregator/p2p.rs`
- Tests extend the existing `#[cfg(test)] mod p2p_broadcast_tests` module
- No new test files or framework installs needed

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
