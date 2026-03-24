---
phase: 10
slug: p2p-operator-dashboard
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Vite dev server + manual visual verification |
| **Config file** | None — no automated frontend test suite exists |
| **Quick run command** | `just app-build-frontend` |
| **Full suite command** | `just app-dev` (full Tauri dev) |
| **Estimated runtime** | ~15 seconds (Vite build) |

---

## Sampling Rate

- **After every task commit:** Run `just app-build-frontend`
- **After every plan wave:** Run `just app-dev` visual smoke test
- **Before `/gsd:verify-work`:** Full visual walkthrough of all 5 success criteria
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 10-01-01 | 01 | 1 | P2P-01 | manual-only | `just app-build-frontend` (TS compiles) | N/A | ⬜ pending |
| 10-01-02 | 01 | 1 | P2P-02 | manual-only | `just app-build-frontend` (TS compiles) | N/A | ⬜ pending |
| 10-01-03 | 01 | 1 | P2P-03 | manual-only | `just app-build-frontend` (TS compiles) | N/A | ⬜ pending |
| 10-01-04 | 01 | 1 | P2P-04 | manual-only | `just app-build-frontend` (TS compiles) | N/A | ⬜ pending |
| 10-01-05 | 01 | 1 | P2P-05 | manual-only | `just app-build-frontend` (TS compiles) | N/A | ⬜ pending |
| 10-01-06 | 01 | 1 | P2P-06 | manual-only | `just app-build-frontend` (TS compiles) | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. No additional test setup needed.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| P2P nav item visible and navigates to /p2p | P2P-01 | UI layout/nav — no test framework | Open app, verify "P2P" in header, click it, verify /p2p route renders |
| Node identity card shows peer ID, discovery mode, addresses | P2P-01 | Visual layout verification | Start WAVS with P2P enabled, navigate to /p2p, verify identity card |
| Peers list updates on interval | P2P-02 | Live polling behavior | Start multi-operator, wait 15s, verify peer count updates |
| Services show human-readable names | P2P-03 | Requires running services | Deploy a service, verify service name appears (not raw hash) |
| Operator key displays with copy button | P2P-04 | Clipboard interaction | Click copy button on operator key, paste, verify match |
| Registration badge shows correct status | P2P-05 | Requires on-chain state | Register operator, verify badge changes from unregistered to registered |
| Quorum placeholder visible | P2P-06 | Stretch — placeholder only | Verify "Quorum data not available" text shown |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
