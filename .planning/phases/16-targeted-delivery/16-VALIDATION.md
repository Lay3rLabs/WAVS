---
phase: 16
slug: targeted-delivery
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-03
---

# Phase 16 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (workspace) |
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
| 16-01-01 | 01 | 1 | TGT-01, TGT-02, COMPAT-01 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests` | ✅ | ⬜ pending |
| 16-02-01 | 02 | 2 | TGT-01, TGT-02, COMPAT-01 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests` | ✅ | ⬜ pending |
| 16-02-02 | 02 | 2 | TGT-04 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests` | ✅ | ⬜ pending |
| 16-02-03 | 02 | 2 | COMPAT-02 | integration | `cargo test -p wavs` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. The `p2p_broadcast_tests` module already has 32 tests covering `PeerSubscriptionMap::get_recipients()`, `has_announced()`, and `set_peer_subscriptions()`.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Multi-operator targeted delivery in live cluster | TGT-03 | Requires multi-node P2P network | Deploy 3+ nodes, subscribe to different services, verify messages route correctly |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
