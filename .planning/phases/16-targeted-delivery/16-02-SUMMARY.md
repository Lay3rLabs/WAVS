---
phase: 16-targeted-delivery
plan: 02
subsystem: p2p
tags: [commonware, broadcast, recipients, targeted-delivery, peer-subscriptions, discovery-network]

# Dependency graph
requires:
  - phase: 16-targeted-delivery
    plan: 01
    provides: "Targeted delivery in run_lookup_network: 3 direct_sender.send() submission sites use peer_subscriptions.get_recipients()"
provides:
  - "Targeted delivery in run_discovery_network: 3 direct_sender.send() submission sites use peer_subscriptions.get_recipients()"
  - "Both bridge loops (lookup + discovery) now behave identically on targeted delivery"
  - "get_recipients dead_code warning resolved (now called from production code in both loops)"
affects: [p2p-performance, multi-operator-targeting]

# Tech tracking
tech-stack:
  added: []
  patterns: ["get_recipients() call at send site for per-service targeting in both bridge loops", "Re-resolution at drain time instead of cached recipients in both bridge loops"]

key-files:
  created: []
  modified: ["packages/wavs/src/subsystems/aggregator/p2p.rs"]

key-decisions:
  - "Discovery loop changes are character-for-character identical to lookup loop changes from Plan 01"
  - "has_announced dead_code warning is pre-existing from Phase 15 -- method is test-only, not called from production bridge loop code (known_peers HashSet used instead)"
  - "All pre-existing clippy warnings (too_many_arguments, clone_on_copy, let_underscore_future, cloned_ref_to_slice_refs) documented but not fixed per scope boundary rules"

patterns-established:
  - "Both bridge loops (lookup and discovery) now use identical targeted delivery patterns"
  - "All direct_sender.send() submission sites in both loops use get_recipients(); only control messages use Recipients::All"

requirements-completed: [TGT-01, TGT-02, TGT-03, TGT-04, COMPAT-01, COMPAT-02]

# Metrics
duration: 11min
completed: 2026-04-03
---

# Phase 16 Plan 02: Targeted Delivery Summary

**Targeted delivery wired into run_discovery_network bridge loop: both P2P modes now use get_recipients() for all submission sends with Recipients::All fallback**

## Performance

- **Duration:** 11 min
- **Started:** 2026-04-03T15:55:39Z
- **Completed:** 2026-04-03T16:06:39Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Wired targeted delivery into all 3 direct_sender.send() submission call sites in run_discovery_network bridge loop (mirrors Plan 01 lookup changes)
- Both bridge loops now behave identically: Publish handler, Publish retry drain, and Heartbeat retry drain all use get_recipients()
- Verified `get_recipients` dead_code clippy warning is resolved (now called from production code in both loops)
- Full crate test suite passes: 140 tests across all test targets, 0 failures
- All mailbox.broadcast() calls remain Recipients::All (TGT-03 maintained)
- All control messages (subscribe/unsubscribe announcements, heartbeat probes, heartbeat subscription piggybacks) remain Recipients::All

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire targeted delivery into run_discovery_network bridge loop** - `53b07a76` (feat)
2. **Task 2: Verify full test suite and clippy clean** - verification-only, no code changes needed

## Files Created/Modified
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Replaced Recipients::All with targeted recipients at 3 direct_sender.send() submission call sites in run_discovery_network (+10 lines, -3 lines)

## Decisions Made
- Discovery loop changes are character-for-character identical to lookup loop changes (Plan 01 pattern preserved exactly)
- `has_announced` dead_code warning documented as pre-existing from Phase 15 (method is only used in tests; bridge loops use `known_peers` HashSet directly for ANN-04 tracking)

## Deviations from Plan

None - plan executed exactly as written.

## Pre-existing Clippy Warnings (Not Phase 16 Scope)

The following clippy warnings exist but are NOT introduced by Phase 16 changes:

| Warning | Location | Notes |
|---------|----------|-------|
| `dead_code` for `has_announced` | p2p.rs:422 | Phase 15 -- test-only method, not called from production code |
| `too_many_arguments` | p2p.rs:631, p2p.rs:1129 | Pre-existing function signatures (run_lookup_network, run_discovery_network) |
| `let_underscore_future` | p2p.rs:859, 1080, 1338, 1557 | Pre-existing retry drain pattern (let _ = mailbox.broadcast) |
| `cloned_ref_to_slice_refs` | p2p.rs:692, 925, 1404 | Pre-existing clone calls |
| `doc list item` | p2p.rs:630 | Pre-existing doc formatting |
| `clone_on_copy` | submit.rs:172-173 | Pre-existing in submit.rs |

Phase 16 resolved: `get_recipients` dead_code warning (now used in production in both bridge loops).

## Issues Encountered
- Worktree was on main branch; fast-forward merge of bls-commonware branch required to get Phase 14/15/16-01 code before starting work (same as Plan 01).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Both bridge loops (lookup and discovery) now have identical targeted delivery for all submission sends
- Phase 16 targeted delivery implementation is complete across both P2P modes
- Pre-existing clippy warnings documented for future cleanup phase

## Acceptance Criteria Verification

| Criterion | Status |
|-----------|--------|
| 2x `get_recipients(&service_id.inner())` (one per loop) | PASS |
| 4x `get_recipients(&queued_msg.service_id_bytes)` (two per loop) | PASS |
| `direct_sender.send(Recipients::All)` only for control messages | PASS (8 remaining: 4 subscribe/unsubscribe + 4 heartbeat) |
| All mailbox.broadcast() still Recipients::All | PASS (8 total across both loops) |
| cargo test -p wavs exits 0 | PASS (140 tests, 0 failures) |
| No get_recipients dead_code warning | PASS |
| No new clippy warnings from Phase 16 | PASS |

## Self-Check: PASSED

- p2p.rs: FOUND
- 16-02-SUMMARY.md: FOUND
- Commit 53b07a76: FOUND

---
*Phase: 16-targeted-delivery*
*Completed: 2026-04-03*
