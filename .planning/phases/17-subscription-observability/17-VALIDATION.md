---
phase: 17
slug: subscription-observability
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-03
---

# Phase 17 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test + cargo test |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p wavs --lib -- p2p` |
| **Full suite command** | `cargo test -p wavs --lib` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p wavs --lib -- p2p`
- **After every plan wave:** Run `just lint && cargo test -p wavs --lib`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 17-01-01 | 01 | 0 | OBS-01 | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_counts` | ❌ W0 | ⬜ pending |
| 17-01-02 | 01 | 1 | OBS-01 | unit | `cargo test -p wavs --lib -- p2p_status_tests` | ✅ (update) | ⬜ pending |
| 17-01-03 | 01 | 1 | OBS-01 | unit | `cargo test -p wavs --lib -- p2p` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `test_peer_subscription_counts` — new unit test in p2p_broadcast_tests module (packages/wavs/src/subsystems/aggregator/p2p.rs or test file)
- [ ] Update `p2p_status_format` test to verify `peer_subscriptions` field exists in serialized output

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `/p2p/status` HTTP response includes `peer_subscriptions` field | OBS-01 | Integration test requires live WAVS node | Start node, register service with BLS key, check `curl localhost:8041/p2p/status` includes `peer_subscriptions` key |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
