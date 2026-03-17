---
phase: 2
slug: broadcast-and-routing
status: draft
nyquist_compliant: false
wave_0_complete: false
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
| 2-01-01 | 01 | 1 | BCAST-01 | unit | `cargo test -p wavs -- p2p::broadcast` | ❌ W0 | ⬜ pending |
| 2-01-02 | 01 | 1 | BCAST-02 | unit | `cargo test -p wavs -- p2p::service_router` | ❌ W0 | ⬜ pending |
| 2-01-03 | 01 | 1 | BCAST-03 | unit | `cargo test -p wavs -- p2p::codec` | ❌ W0 | ⬜ pending |
| 2-02-01 | 02 | 2 | BCAST-04 | unit | `cargo test -p wavs -- p2p::retry_queue` | ❌ W0 | ⬜ pending |
| 2-02-02 | 02 | 2 | BCAST-05 | unit | `cargo test -p wavs -- p2p::handle` | ❌ W0 | ⬜ pending |
| 2-03-01 | 03 | 2 | CATCH-01 | unit | `cargo test -p wavs -- p2p::catch_up` | ❌ W0 | ⬜ pending |
| 2-03-02 | 03 | 2 | CATCH-02 | unit | `cargo test -p wavs -- p2p::buffered_engine` | ❌ W0 | ⬜ pending |
| 2-03-03 | 03 | 3 | INT-01 | integration | `cargo test -p layer-tests` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `packages/wavs/src/subsystems/trigger/p2p/broadcast_tests.rs` — stubs for BCAST-01, BCAST-02
- [ ] `packages/wavs/src/subsystems/trigger/p2p/service_router_tests.rs` — stubs for BCAST-02
- [ ] `packages/wavs/src/subsystems/trigger/p2p/codec_tests.rs` — stubs for BCAST-03
- [ ] `packages/wavs/src/subsystems/trigger/p2p/retry_queue_tests.rs` — stubs for BCAST-04
- [ ] `packages/wavs/src/subsystems/trigger/p2p/catch_up_tests.rs` — stubs for CATCH-01, CATCH-02

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Broadcast delivery to all connected peers under network partition | BCAST-01 | Requires multi-process E2E setup with network manipulation | Start 3 WAVS nodes, partition one, broadcast, verify delivery count |
| Catch-up after real network reconnect | CATCH-01 | Requires actual disconnect/reconnect of libp2p swarm | Start 2 nodes, disconnect, broadcast 5 msgs, reconnect, verify retrieval |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
