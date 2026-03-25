---
phase: 12
slug: unified-activity-events
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Vite build (TypeScript type checking) + cargo check (Rust) |
| **Config file** | app/vite.config.ts, app/src-tauri/Cargo.toml |
| **Quick run command** | `npx --prefix app vite build --config app/vite.config.ts 2>&1 \| tail -5` |
| **Full suite command** | `cargo check -p wavs-app 2>&1 \| tail -5 && npx --prefix app vite build --config app/vite.config.ts 2>&1 \| tail -5` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `npx --prefix app vite build --config app/vite.config.ts 2>&1 | tail -5`
- **After every plan wave:** Run full suite (cargo check + vite build)
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 12-01-01 | 01 | 1 | ACT-01 | build | `npx --prefix app vite build` | ✅ | ⬜ pending |
| 12-01-02 | 01 | 1 | ACT-02, ACT-03 | build | `npx --prefix app vite build` | ✅ | ⬜ pending |
| 12-02-01 | 02 | 2 | ACT-01 | build+cargo | `cargo check -p wavs-app && npx --prefix app vite build` | ✅ | ⬜ pending |
| 12-02-02 | 02 | 2 | ACT-02, ACT-03 | build | `npx --prefix app vite build` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. Vite build provides TypeScript type checking; cargo check validates Rust backend changes.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Unified event cards merge trigger + submission | ACT-01 | Requires running WAVS node generating real events | Deploy service, trigger workflow, verify single card shows both trigger and submission |
| Status progression visual indicators | ACT-02 | Visual rendering requires human inspection | Watch event card progress through pending → submitted → confirmed states |
| Submission errors display inline | ACT-03 | Requires triggering a real submission failure | Configure invalid submission target, verify error message appears on event card |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
