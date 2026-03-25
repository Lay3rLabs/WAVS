---
phase: 9
slug: foundation-types-and-settings-refactor
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-23
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | vitest (existing) + Tauri command integration via dev server |
| **Config file** | `app/vitest.config.ts` (if exists, else create) |
| **Quick run command** | `cd app && npx vitest run --reporter=verbose` |
| **Full suite command** | `cd app && npx vitest run` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cd app && npx vitest run --reporter=verbose`
- **After every plan wave:** Run `cd app && npx vitest run`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | FND-01 | unit | `grep -q "bls12381" app/src/types/*.ts` | ❌ W0 | ⬜ pending |
| 09-01-02 | 01 | 1 | FND-02 | unit | `grep -q "cmd_get_p2p_status\|cmd_get_service_signer\|cmd_derive_bls_pubkey" app/src-tauri/src/commands.rs` | ❌ W0 | ⬜ pending |
| 09-02-01 | 02 | 1 | SET-01 | manual | Visual inspection of settings page sections | ❌ W0 | ⬜ pending |
| 09-02-02 | 02 | 1 | SET-02 | unit | `find app/src/pages/settings -name "*.tsx" \| wc -l` (expect >= 6) | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Verify vitest is configured for the app workspace
- [ ] Existing Tauri command patterns documented for reference

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Settings page visual parity | SET-01 | Layout/spacing requires visual inspection | Compare settings page before/after decomposition — sections should render identically |
| Settings visual hierarchy | SET-02 | Typography/spacing assessment | Verify consistent heading sizes, section spacing, and visual grouping |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
