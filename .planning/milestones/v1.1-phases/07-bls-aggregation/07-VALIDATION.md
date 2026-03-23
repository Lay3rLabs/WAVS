---
phase: 7
slug: bls-aggregation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p layer-wavs aggregator` |
| **Full suite command** | `cargo test -p layer-wavs` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p layer-wavs aggregator`
- **After every plan wave:** Run `cargo test -p layer-wavs`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 7-01-01 | 01 | 1 | AGG-01 | unit | `cargo test -p layer-wavs bls_accumulate` | ✅ | ⬜ pending |
| 7-01-02 | 01 | 1 | AGG-02 | unit | `cargo test -p layer-wavs bls_aggregate_sort` | ✅ | ⬜ pending |
| 7-01-03 | 01 | 1 | AGG-03 | unit | `cargo test -p layer-wavs bls_reference_block` | ✅ | ⬜ pending |
| 7-01-04 | 01 | 1 | AGG-04 | unit | `cargo test -p layer-wavs bls_submit` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `packages/wavs/src/subsystems/aggregator/` — BLS-specific unit test stubs
- [ ] Test fixtures for mock G2 signatures and G1 pubkeys using blst

*Existing cargo test infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| On-chain BLS sig validation via EIP-2537 | AGG-04 | Requires live anvil + deployed BLS contracts | Run `just test-wavs-e2e` after Phase 8 E2E tests added |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
