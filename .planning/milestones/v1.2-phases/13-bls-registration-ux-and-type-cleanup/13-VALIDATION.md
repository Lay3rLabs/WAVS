---
phase: 13
slug: bls-registration-ux-and-type-cleanup
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 13 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | None (no test framework configured for app/) |
| **Config file** | None |
| **Quick run command** | `cd app && npx tsc --noEmit` |
| **Full suite command** | `cd app && npx tsc --noEmit && npx vite build` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cd app && npx tsc --noEmit`
- **After every plan wave:** Run `cd app && npx tsc --noEmit && npx vite build`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 13-01-01 | 01 | 1 | FND-01 | unit (type check) | `cd app && npx tsc --noEmit` | N/A | ⬜ pending |
| 13-02-01 | 02 | 1 | BLS-03 | manual + type check | `cd app && npx tsc --noEmit` | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. TypeScript strict mode (`tsc --noEmit`) is already configured and serves as the automated validation layer.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Guidance banner renders when BLS service has no POA registry | BLS-03 | No test framework for React components in app/ | 1. Open a BLS service detail page where the service is not saved as a POA registry. 2. Verify an amber guidance banner appears explaining registry requirement. 3. Verify banner does NOT appear on ECDSA service pages. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
