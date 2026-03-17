---
phase: 1
slug: secure-peer-connectivity
status: draft
nyquist_compliant: false
wave_0_complete: false
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

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01 | 1 | IDEN-01 | unit | `cargo test -p wavs test_ed25519_derivation` | ❌ W0 | ⬜ pending |
| 1-01-02 | 01 | 1 | IDEN-02 | unit | `cargo test -p wavs test_peer_id_deterministic` | ❌ W0 | ⬜ pending |
| 1-02-01 | 02 | 1 | NET-01, NET-04 | unit | `cargo test -p wavs test_runner_dedicated_thread` | ❌ W0 | ⬜ pending |
| 1-02-02 | 02 | 2 | NET-02 | integration | `cargo test -p wavs test_discovery_mode` | ❌ W0 | ⬜ pending |
| 1-02-03 | 02 | 2 | NET-03 | integration | `cargo test -p wavs test_lookup_mode` | ❌ W0 | ⬜ pending |
| 1-03-01 | 03 | 2 | SEC-01, SEC-02 | integration | `cargo test -p wavs test_oracle_authorization` | ❌ W0 | ⬜ pending |
| 1-03-02 | 03 | 2 | SEC-03 | integration | `cargo test -p wavs test_unauthorized_peer_rejected` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `packages/wavs/src/p2p/mod.rs` — p2p module stubs (empty impl for identity, network, oracle)
- [ ] `packages/wavs/tests/p2p_identity.rs` — unit test stubs for IDEN-01, IDEN-02
- [ ] `packages/wavs/tests/p2p_network.rs` — integration test stubs for NET-01 through NET-04
- [ ] `packages/wavs/tests/p2p_security.rs` — integration test stubs for SEC-01 through SEC-03

*Wave 0 establishes the test scaffolding so each implementation task has a failing test to make green.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Two live WAVS nodes discover each other | NET-02 | Requires two running processes with real network | Start two WAVS nodes with bootstrapper config; observe connection logs |
| Unauthorized peer connection rejected at network level | SEC-01 | Requires two processes with mismatched Oracle sets | Start two nodes; exclude peer B from node A's Oracle; verify B cannot connect |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
