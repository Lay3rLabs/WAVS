---
phase: 2
slug: broadcast-and-routing
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-17
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust unit tests) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p wavs -- p2p` |
| **Full suite command** | `cargo test -p wavs && cargo test -p layer-tests` |
| **Estimated runtime** | ~30 seconds (unit), ~120 seconds (E2E) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p wavs -- p2p`
- **After every plan wave:** Run `cargo test -p wavs`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 2-01-00 | 01 | 0 | ALL | stub | `cargo test -p wavs -- p2p_broadcast_tests` (expect FAIL) | Created by Task 0 | ⬜ pending |
| 2-01-01 | 01 | 1 | BCAST-02, BCAST-03 | unit | `cargo test -p wavs -- p2p_broadcast_tests::test_p2p_message` | Wave 0 stub | ⬜ pending |
| 2-01-02 | 01 | 1 | BCAST-04, BCAST-05 | unit | `cargo test -p wavs -- p2p_broadcast_tests` (service_router + retry_queue) | Wave 0 stub | ⬜ pending |
| 2-02-01 | 02 | 2 | BCAST-01, CATCH-02 | compile | `cargo check -p wavs` | N/A | ⬜ pending |
| 2-02-02 | 02 | 2 | BCAST-01, BCAST-04 | compile+unit | `cargo check -p wavs && cargo test -p wavs -- p2p_broadcast_tests` | Wave 0 stub | ⬜ pending |
| 2-02-03 | 02 | 2 | ALL | integration | `cargo test -p wavs --test p2p_broadcast_tests` | Created by Task 3 | ⬜ pending |

*Status: ⬜ pending / ✅ green / ❌ red / ⚠️ flaky*

---

## Wave 0 Requirements

- [x] Plan 02-01 Task 0 creates `#[cfg(test)] mod p2p_broadcast_tests` in p2p.rs with 12 failing test stubs
- [x] Stubs cover: P2pMessage codec/digest (BCAST-02, BCAST-03), ServiceRouter (BCAST-05), RetryQueue (BCAST-04)
- [x] `commonware-broadcast` dependency added in Task 0 so stubs can reference traits
- [ ] Plan 02-02 Task 3 creates `packages/wavs/tests/p2p_broadcast_tests.rs` covering BCAST-01, BCAST-02, BCAST-04, BCAST-05, CATCH-01, CATCH-02, INT-01

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Broadcast delivery to all connected peers under network partition | BCAST-01 | Requires multi-process E2E setup with network manipulation | Start 3 WAVS nodes, partition one, broadcast, verify delivery count |
| Catch-up after real network reconnect | CATCH-01 | Requires actual disconnect/reconnect timing | Start 2 nodes, disconnect, broadcast 5 msgs, reconnect, verify retrieval |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending execution
