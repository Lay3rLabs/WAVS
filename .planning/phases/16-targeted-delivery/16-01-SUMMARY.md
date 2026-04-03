---
phase: 16-targeted-delivery
plan: 01
subsystem: p2p
tags: [commonware, broadcast, recipients, targeted-delivery, peer-subscriptions]

# Dependency graph
requires:
  - phase: 15-subscription-protocol
    provides: "PeerSubscriptionMap with get_recipients(), set_peer_subscriptions(), handle_announcement() + subscription wire protocol in bridge loops"
provides:
  - "Targeted delivery in run_lookup_network: 3 direct_sender.send() submission sites use peer_subscriptions.get_recipients()"
  - "test_retry_re_resolution unit test proving TGT-04 re-resolution behavior"
affects: [16-02-PLAN, discovery-network-targeting]

# Tech tracking
tech-stack:
  added: []
  patterns: ["get_recipients() call at send site for per-service targeting", "Re-resolution at drain time instead of cached recipients"]

key-files:
  created: []
  modified: ["packages/wavs/src/subsystems/aggregator/p2p.rs"]

key-decisions:
  - "Only run_lookup_network modified; run_discovery_network is Plan 02 scope"
  - "mailbox.broadcast() calls (Engine channel) remain Recipients::All per TGT-03"
  - "Control messages (subscribe/unsubscribe/heartbeat probe/heartbeat subscription) remain Recipients::All"

patterns-established:
  - "TGT-01 pattern: direct_recipients = peer_subscriptions.get_recipients(&service_id.inner()) before direct_sender.send()"
  - "TGT-04 pattern: retry_recipients = peer_subscriptions.get_recipients(&queued_msg.service_id_bytes) at drain time"

requirements-completed: [TGT-01, TGT-02, TGT-03, TGT-04, COMPAT-01]

# Metrics
duration: 8min
completed: 2026-04-03
---

# Phase 16 Plan 01: Targeted Delivery Summary

**Targeted delivery wired into run_lookup_network bridge loop: 3 direct_sender.send() submission sites use peer_subscriptions.get_recipients() with Recipients::All fallback**

## Performance

- **Duration:** 8 min
- **Started:** 2026-04-03T15:44:08Z
- **Completed:** 2026-04-03T15:52:08Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added test_retry_re_resolution unit test proving get_recipients returns different results before and after subscription state arrives (TGT-04)
- Wired targeted delivery into all 3 direct_sender.send() submission call sites in run_lookup_network bridge loop
- Preserved all mailbox.broadcast(Recipients::All) calls for Engine channel (TGT-03)
- Preserved all control message sends (subscribe/unsubscribe announcements, heartbeat probes) as Recipients::All
- All 33 p2p_broadcast_tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add test_retry_re_resolution unit test** - `7e19916a` (test)
2. **Task 2: Wire targeted delivery into run_lookup_network bridge loop** - `252e4c2e` (feat)

## Files Created/Modified
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Added test_retry_re_resolution test + replaced Recipients::All with targeted recipients at 3 direct_sender.send() submission call sites in run_lookup_network

## Decisions Made
None - followed plan as specified. All 3 change sites and the test matched the plan exactly.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Worktree was based on main branch which lacked commonware P2P code; resolved by fast-forward merging bls-commonware branch to get Phase 14/15 code before starting work.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- run_lookup_network bridge loop now has targeted delivery for all submission sends
- run_discovery_network bridge loop still uses Recipients::All for submission sends -- this is Plan 02 scope
- All existing tests pass, ready for Plan 02 to apply identical changes to discovery network

## Self-Check: PASSED

- p2p.rs: FOUND
- 16-01-SUMMARY.md: FOUND
- Commit 7e19916a: FOUND
- Commit 252e4c2e: FOUND

---
*Phase: 16-targeted-delivery*
*Completed: 2026-04-03*
