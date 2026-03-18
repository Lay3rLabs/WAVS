# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — Commonware P2P Migration

**Shipped:** 2026-03-18
**Phases:** 4 | **Plans:** 11 | **Execution time:** ~2.8 hours (avg 15 min/plan)

### What Was Built
- Ed25519 P2P identity derived from BIP-39 mnemonic via ChaCha20Rng — deterministic across restarts
- Commonware-p2p lookup and discovery modes with Oracle authorization, rate limiting, and peer blocking
- commonware-broadcast buffered Engine with ServiceRouter filtering, SHA-256 dedup, retry queue, and catch-up
- New P2P config format (Disabled/Local/Remote) with configurable tuning fields; real connected-peer tracking in `/p2p/status`
- Full libp2p removal — zero workspace references after cleanup
- Complete documentation: P2P.md rewrite, announcement blog post, 231-line operator migration guide

### What Worked
- **TDD throughout**: Writing failing tests before implementation caught API assumption errors early (type mismatches, missing trait bounds)
- **Phase breakdown**: Separating identity/network, broadcast, config, and cleanup into discrete phases made progress predictable
- **Trait research upfront**: The commonware skill surfaced the `rand_chacha 0.3` / `rand_core 0.6` dependency constraint before it became a blocker
- **Dedicated OS thread pattern**: spawn_commonware_runtime() on std::thread with channel bridge worked cleanly — no Tokio nesting issues
- **SUMMARY.md discipline**: Capturing decisions and deviations in each plan made retrospective easy and will help future contributors

### What Was Inefficient
- **ROADMAP.md checkboxes not updated during execution** — all phases showed `[ ]` even after completion; the progress table diverged from reality (Phases 1-3 not checked off despite completion)
- **Audit needed manual gap investigation** — MISSING-01 (bootstrapper address format) was found in the audit rather than caught by e2e tests during Phase 4; earlier integration testing of the remote harness could have caught it sooner
- **CLI returned empty accomplishments** — `milestone complete` CLI couldn't extract accomplishments from SUMMARY.md files (fields returned as "None"); needed manual extraction

### Patterns Established
- `spawn_commonware_runtime()` on dedicated OS thread with `std::sync::mpsc` bridge to Tokio — canonical pattern for commonware-in-WAVS integration
- `rand_chacha = "0.3"` pinned for commonware compatibility (rand_core 0.6 boundary)
- `Config::local()` provides SEC-02 rate limiting by default — no explicit builder calls needed
- `Set::from_iter_dedup()` for discovery Oracle, `Map::from_iter_dedup()` for lookup Oracle — consistent dedup handling
- Two-channel broadcast (channel 0 = Engine cache, channel 1 = direct forward) for catch-up + live delivery

### Key Lessons
1. **Version boundaries matter for external crates**: commonware's `rand_core 0.6` transitive dep requires `rand_chacha 0.3`; pinning to exact compatible versions avoids trait incompatibility at compile time
2. **commonware-math is a direct dep**: `Random` trait for `PrivateKey::random()` lives in `commonware_math::algebra`, not re-exported — add it explicitly
3. **Milestone audits catch harness bugs**: The e2e test harness (handles.rs) had a silent bootstrapper address bug that unit/integration tests couldn't catch; milestone-level E2E audit is worth running before declaring done
4. **commonware channels are static**: Must be registered before `network.start()` — design for a fixed set of channels upfront, not dynamic per-service channels
5. **Oracle peer map format differs by mode**: Discovery uses `Set` (pubkeys only), Lookup uses `Map` (pubkey → address) — easy to mix up, document at the call site

### Cost Observations
- Model: claude-sonnet-4-6 (quality profile) throughout
- Sessions: ~5 sessions across 2 days (2026-03-17 to 2026-03-18)
- Notable: Most plans completed in 7-20 min; Phase 2 plan 02 was the longest (broadcast Engine wiring + 7 integration tests)

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 4 | 11 | First milestone — established WAVS+commonware patterns |

### Cumulative Quality

| Milestone | Tests Added | Requirements | libp2p Removed |
|-----------|-------------|--------------|----------------|
| v1.0 | 28 | 27/27 | ✓ |

### Top Lessons (Verified Across Milestones)

1. TDD (failing tests first) consistently catches API surface mismatches before implementation sinks too deep
2. External crate version pinning is critical for embedded runtimes with transitive rand/crypto deps
