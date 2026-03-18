---
phase: 4
slug: validation-and-cleanup
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-17
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust), layer-tests E2E suite |
| **Config file** | `packages/layer-tests/layer-tests.toml` |
| **Quick run command** | `cargo build 2>&1 | tail -5` |
| **Full suite command** | `just test-wavs-e2e` |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build 2>&1 | tail -5`
- **After every plan wave:** Run `just test-wavs-e2e`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 4-01-01 | 01 | 1 | INT-02 | compile | `cargo build 2>&1 \| grep -c "^error" \| xargs -I{} test {} -eq 0` | ✅ | ⬜ pending |
| 4-01-02 | 01 | 1 | INT-02 | grep | `grep -r "libp2p" Cargo.toml packages/wavs/Cargo.toml \| wc -l \| xargs -I{} test {} -eq 0` | ✅ | ⬜ pending |
| 4-02-01 | 02 | 1 | INT-03 | compile | `cargo build 2>&1 \| grep -c "^error" \| xargs -I{} test {} -eq 0` | ✅ | ⬜ pending |
| 4-02-02 | 02 | 1 | INT-03 | e2e | `just test-wavs-e2e 2>&1 \| tail -20` | ✅ | ⬜ pending |
| 4-03-01 | 03 | 2 | DOC-01 | file | `test -f docs/P2P.md && grep -c "commonware" docs/P2P.md \| xargs -I{} test {} -gt 0` | ✅ | ⬜ pending |
| 4-04-01 | 04 | 2 | DOC-02 | file | `ls docs/blog/*.md 2>/dev/null \| wc -l \| xargs -I{} test {} -ge 1` | ❌ W0 | ⬜ pending |
| 4-05-01 | 05 | 2 | DOC-03 | file | `test -f docs/OPERATOR_MIGRATION.md && grep -c "identity" docs/OPERATOR_MIGRATION.md \| xargs -I{} test {} -gt 0` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `docs/blog/` — create directory (for blog post)
- [ ] `docs/OPERATOR_MIGRATION.md` — create stub file

*Existing infrastructure (cargo, layer-tests, docs/P2P.md) covers all other phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Blog post quality and announcement tone | DOC-02 | Content quality is subjective | Read `docs/blog/*.md` — verify announcement style (not tutorial), covers commonware migration purpose, operator impact, and timeline |
| Operator migration guide completeness | DOC-03 | Content correctness requires domain knowledge | Read `docs/OPERATOR_MIGRATION.md` — verify identity change, config format change, and coordinated upgrade requirement are documented clearly |
| Multi-operator e2e actually tests multiple operators | INT-03 | Test configuration verification | Run `just test-wavs-e2e` and confirm `evm_multi_operator` test case passes specifically |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
