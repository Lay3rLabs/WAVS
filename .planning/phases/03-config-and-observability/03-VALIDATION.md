---
phase: 3
slug: config-and-observability
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-17
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in + tokio::test) |
| **Config file** | Cargo.toml test configuration |
| **Quick run command** | `cargo test -p wavs -- p2p --test-threads=1` |
| **Full suite command** | `cargo test -p wavs --test p2p_connectivity_tests --test p2p_broadcast_tests -- --test-threads=1` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p wavs -- p2p --test-threads=1`
- **After every plan wave:** Run `cargo test -p wavs --test p2p_connectivity_tests --test p2p_broadcast_tests -- --test-threads=1`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 3-00-01 | 00 | 0 | CFG-01, CFG-02 | unit | `cargo test -p wavs p2p_config_serde p2p_config_defaults -- --test-threads=1` | W0 creates | ⬜ pending |
| 3-00-02 | 00 | 0 | OBS-02 | unit | `cargo test -p wavs p2p_status_format -- --test-threads=1` | W0 creates | ⬜ pending |
| 3-00-03 | 00 | 0 | OBS-01 | integration | `cargo test -p wavs --test p2p_broadcast_tests test_status_connected_peers_after_broadcast -- --test-threads=1` | W0 creates | ⬜ pending |
| 3-01-01 | 01 | 1 | CFG-01 | unit | `cargo test -p wavs p2p_config_serde -- --test-threads=1` | ✅ W0 | ⬜ pending |
| 3-01-02 | 01 | 1 | CFG-02 | unit | `cargo test -p wavs p2p_config_defaults -- --test-threads=1` | ✅ W0 | ⬜ pending |
| 3-02-01 | 02 | 1 | CFG-01 | unit | `cargo test -p wavs p2p_config_serde -- --test-threads=1` | ✅ W0 | ⬜ pending |
| 3-02-02 | 02 | 1 | CFG-02 | unit | `cargo test -p wavs p2p_config_defaults -- --test-threads=1` | ✅ W0 | ⬜ pending |
| 3-02-03 | 02 | 1 | OBS-02 | unit | `cargo test -p wavs p2p_status_format -- --test-threads=1` | ✅ W0 | ⬜ pending |
| 3-03-01 | 03 | 2 | OBS-01 | integration | `cargo test -p wavs --test p2p_broadcast_tests test_status_connected_peers_after_broadcast -- --test-threads=1` | ✅ W0 | ⬜ pending |
| 3-03-02 | 03 | 2 | CFG-03 | integration | `cargo test -p wavs --test p2p_connectivity_tests test_lookup_mode -- --test-threads=1` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Plan 03-00 creates these test stubs:

- [ ] `packages/wavs/src/subsystems/aggregator/p2p_config_tests.rs` — unit test stubs for CFG-01 (p2p_config_serde) and CFG-02 (p2p_config_defaults)
- [ ] `packages/wavs/src/subsystems/aggregator/p2p_status_tests.rs` — unit test stub for OBS-02 (p2p_status_format)
- [ ] `packages/wavs/tests/p2p_broadcast_tests.rs` — integration test stub for OBS-01 (test_status_connected_peers_after_broadcast)

*All test files must compile (even with stub bodies) before Wave 1 begins.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| wavs.toml comments are readable and correct | CFG-01, CFG-03 | Documentation quality review | Read wavs.toml P2P section, verify comments reference commonware (not libp2p), verify example config matches actual P2pConfig fields |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (Plan 03-00 creates all stubs)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending execution of Plan 03-00
