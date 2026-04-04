---
phase: 18
slug: peer-state-correctness
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-04
---

# Phase 18 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[cfg(test)]` + cargo test |
| **Config file** | `packages/wavs/src/subsystems/aggregator/p2p.rs` (inline tests) |
| **Quick run command** | `cargo test -p wavs --lib aggregator::p2p` |
| **Full suite command** | `cargo test -p wavs` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p wavs --lib aggregator::p2p`
- **After every plan wave:** Run `cargo test -p wavs`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 18-01-01 | 01 | 1 | SUB-03 | — | Departed peers pruned from PeerSubscriptionMap | unit | `cargo test -p wavs --lib aggregator::p2p::tests::test_prune_departed_peers` | ❌ W0 | ⬜ pending |
| 18-01-02 | 01 | 1 | COMPAT-03 | — | Un-announced peers included in get_recipients | unit | `cargo test -p wavs --lib aggregator::p2p::tests::test_get_recipients_includes_unannounced` | ❌ W0 | ⬜ pending |
| 18-01-03 | 01 | 1 | COMPAT-03 | — | has_announced() called in production code | unit | `cargo test -p wavs --lib aggregator::p2p` | ✅ | ⬜ pending |
| 18-01-04 | 01 | 1 | SUB-03, COMPAT-03 | — | Existing tests pass with updated signatures | regression | `cargo test -p wavs --lib aggregator::p2p` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements. New tests are created inline alongside the implementation.*

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
