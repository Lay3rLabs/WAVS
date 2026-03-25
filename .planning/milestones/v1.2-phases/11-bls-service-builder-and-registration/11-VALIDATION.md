---
phase: 11
slug: bls-service-builder-and-registration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | vitest (frontend) / cargo test (Rust) |
| **Config file** | `app/vite.config.ts` (vitest), `Cargo.toml` (cargo) |
| **Quick run command** | `npx --prefix app vite build --config app/vite.config.ts` |
| **Full suite command** | `cargo check -p wavs-app && npx --prefix app vite build --config app/vite.config.ts` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `npx --prefix app vite build --config app/vite.config.ts`
- **After every plan wave:** Run `cargo check -p wavs-app && npx --prefix app vite build --config app/vite.config.ts`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 1 | BLS-01 | build | `npx --prefix app vite build --config app/vite.config.ts` | ✅ | ⬜ pending |
| 11-01-02 | 01 | 1 | BLS-02 | build | `npx --prefix app vite build --config app/vite.config.ts` | ✅ | ⬜ pending |
| 11-02-01 | 02 | 2 | BLS-03, BLS-04 | build+check | `cargo check -p wavs-app && npx --prefix app vite build --config app/vite.config.ts` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. Vite build and cargo check provide type-level verification. No new test framework installation needed.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Algorithm selector shows ECDSA/BLS | BLS-01 | Visual UI element | Open service builder, verify selector renders with both options |
| BLS pubkey displayed after deploy | BLS-02 | Requires deployed BLS service | Deploy a BLS service, verify G1 pubkey shows with copy button |
| Register BLS key on-chain | BLS-03 | Requires wallet + chain interaction | Click register, sign tx, verify on-chain state updates |
| Registration status badge | BLS-04 | Requires on-chain state | After registration, verify badge shows "Registered" |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
