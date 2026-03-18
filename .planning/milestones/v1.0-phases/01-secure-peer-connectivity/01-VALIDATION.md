---
phase: 1
slug: secure-peer-connectivity
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-17
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust unit/integration tests) |
| **Config file** | `packages/layer-tests/layer-tests.toml` |
| **Quick run command** | `cargo test -p wavs 2>&1 | tail -20` |
| **Full suite command** | `cargo test -p wavs && cargo test -p layer-tests` |
| **Estimated runtime** | ~30 seconds (unit) / ~120 seconds (e2e) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p wavs 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test -p wavs && cargo test -p layer-tests`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 1-01-01 | 01 | 1 | IDEN-01, IDEN-02 | integration | `cargo test -p wavs --test p2p_identity_tests -- --nocapture` | pending |
| 1-01-02 | 01 | 1 | (config rewrite) | compile | `cargo check -p wavs` (P2pConfig rewrite) | pending |
| 1-02-01 | 02 | 2 | NET-02, NET-03, SEC-02 | compile | `cargo check -p wavs` (runtime scaffold + lookup + rate limiting) | pending |
| 1-02-02 | 02 | 2 | NET-02, SEC-01 | integration | `cargo test -p wavs --features dev --test p2p_connectivity_tests -- --nocapture` | pending |
| 1-03-01 | 03 | 3 | NET-01, NET-04, SEC-03 | compile | `cargo check -p wavs` (BlockPeer + discovery mode) | pending |
| 1-03-02 | 03 | 3 | NET-01, SEC-03, NET-04 | integration | `cargo test -p wavs --features dev --test p2p_connectivity_tests -- --nocapture` | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

Plans use inline TDD (tests created within the same task as implementation, `tdd="true"` on task elements). This satisfies the Nyquist sampling contract because:

1. Plan 01-01 Task 1 creates `packages/wavs/tests/p2p_identity_tests.rs` as part of TDD RED->GREEN cycle
2. Plan 01-02 Task 2 creates `packages/wavs/tests/p2p_connectivity_tests.rs` as part of TDD
3. Plan 01-03 Task 2 adds tests to `p2p_connectivity_tests.rs`

No separate Wave 0 stub-creation step is needed. Each test file is created by the first task that needs it, and all subsequent tasks that modify the same file add to it.

- [x] Test files created inline by TDD tasks (no separate Wave 0 stubs needed)
- [x] `packages/wavs/tests/p2p_identity_tests.rs` — created by Plan 01-01 Task 1
- [x] `packages/wavs/tests/p2p_connectivity_tests.rs` — created by Plan 01-02 Task 2

---

## Test File Map

| Test File | Created By | Tests |
|-----------|------------|-------|
| `packages/wavs/tests/p2p_identity_tests.rs` | Plan 01-01 Task 1 | `test_deterministic_derivation`, `test_consistent_across_restarts`, `test_different_mnemonics_produce_different_keys`, `test_invalid_mnemonic_returns_error`, `test_p2p_config_default_is_disabled` |
| `packages/wavs/tests/p2p_connectivity_tests.rs` | Plan 01-02 Task 2 | `test_lookup_mode_two_nodes_connect`, `test_unauthorized_peer_rejected` |
| `packages/wavs/tests/p2p_connectivity_tests.rs` | Plan 01-03 Task 2 (adds to) | `test_discovery_mode_two_nodes`, `test_block_peer`, `test_auto_reconnect` |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Two live WAVS nodes discover each other | NET-02 | Full stack (Dispatcher + Aggregator) multi-process test is Phase 4 | Start two WAVS nodes with bootstrapper config; observe connection logs |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Inline TDD satisfies Wave 0 sampling contract (test files created by first task using them)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter
- [x] NET-04 (auto-reconnect) has automated test: `test_auto_reconnect` in Plan 01-03 Task 2

**Approval:** approved
