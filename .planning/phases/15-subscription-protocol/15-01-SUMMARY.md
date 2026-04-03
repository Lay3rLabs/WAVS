---
phase: 15-subscription-protocol
plan: 01
subsystem: p2p
tags: [commonware, subscription, serde, backward-compat, peer-tracking]

# Dependency graph
requires:
  - phase: 14-subscription-data-structures
    provides: SubscriptionAnnouncement, PeerSubscriptionMap, SUBSCRIPTION_SENTINEL, ServiceRouter
provides:
  - full_state field on SubscriptionAnnouncement with serde(default) backward compat
  - PeerSubscriptionMap::set_peer_subscriptions() for replace-not-merge heartbeat sync
  - PeerSubscriptionMap::has_announced() for COMPAT-03 unknown-peer fallback
affects: [15-subscription-protocol plan 02 bridge loop wiring]

# Tech tracking
tech-stack:
  added: []
  patterns: [replace-not-merge via remove+reinsert, serde(default) for wire-format backward compat]

key-files:
  created: []
  modified:
    - packages/wavs/src/subsystems/aggregator/p2p.rs

key-decisions:
  - "serde(default) on full_state ensures Phase 14 announcements deserialize as full_state=false"
  - "set_peer_subscriptions delegates to remove_peer + reinsert for clean replace semantics"
  - "has_announced uses peer_to_services.contains_key -- empty-set peers still count as announced"

patterns-established:
  - "Replace-not-merge: remove_peer() then reinsert avoids partial state"
  - "Backward compat via serde(default): new fields default to safe values for old messages"

requirements-completed: [ANN-03, COMPAT-03]

# Metrics
duration: 9min
completed: 2026-04-03
---

# Phase 15 Plan 01: Subscription Data Structure Extensions Summary

**Extended SubscriptionAnnouncement with full_state discriminator and PeerSubscriptionMap with replace-not-merge and has_announced tracking for bridge loop protocol**

## Performance

- **Duration:** 9 min
- **Started:** 2026-04-03T14:44:09Z
- **Completed:** 2026-04-03T14:53:28Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 1

## Accomplishments
- Added `full_state: bool` field to `SubscriptionAnnouncement` with `#[serde(default)]` for backward compatibility
- Implemented `PeerSubscriptionMap::set_peer_subscriptions()` with replace-not-merge semantics
- Implemented `PeerSubscriptionMap::has_announced()` for COMPAT-03 backward-compat peer tracking
- Updated all 9 existing test constructions to include `full_state: false`
- Added 10 new comprehensive unit tests covering serde default, replace semantics, has_announced lifecycle, incremental vs full_state processing, and P2P message roundtrips

## Task Commits

Each task was committed atomically (TDD RED/GREEN cycle):

1. **Task 1 RED: Failing tests for full_state, set_peer_subscriptions, has_announced** - `e53346dc` (test)
2. **Task 1 GREEN: Implement has_announced and set_peer_subscriptions methods** - `f9172506` (feat)

_TDD: RED committed compilation-failing tests, GREEN implemented methods to pass all 32 tests._

## Files Created/Modified
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Extended SubscriptionAnnouncement with full_state field, added has_announced() and set_peer_subscriptions() methods to PeerSubscriptionMap, 10 new tests

## Decisions Made
- Used `#[serde(default)]` on `full_state` field so Phase 14 announcements (without the field) deserialize correctly with `full_state=false`
- `set_peer_subscriptions` delegates to `remove_peer` then reinserts, ensuring clean replace-not-merge semantics without partial state
- `has_announced` checks `peer_to_services.contains_key` -- even empty-set peers would count as announced (though `set_peer_subscriptions([])` removes the entry)
- Fixed clippy `bool_assert_comparison` warnings by using `assert!`/`assert!(!...)` instead of `assert_eq!(val, true/false)`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed clippy bool_assert_comparison in new test code**
- **Found during:** Task 1 GREEN (verification)
- **Issue:** New test assertions used `assert_eq!(val, false)` which clippy flags as `bool_assert_comparison`
- **Fix:** Changed to `assert!(!val)` and `assert!(val)` patterns
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs
- **Verification:** `cargo clippy -p wavs --lib --tests -- -D clippy::bool-assert-comparison` exits 0
- **Committed in:** f9172506 (part of GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Minor style fix required by clippy. No scope creep.

## Issues Encountered
- Pre-existing clippy errors in wavs crate (dead_code for Phase 14 structures not yet wired, too_many_arguments in unrelated code) -- these are out of scope and not caused by this plan's changes

## User Setup Required
None - no external service configuration required.

## Known Stubs
None -- all functionality is fully implemented and tested.

## Next Phase Readiness
- SubscriptionAnnouncement with full_state field ready for Plan 02 bridge loop dispatch
- set_peer_subscriptions() ready for heartbeat full-state sync in bridge loop
- has_announced() ready for COMPAT-03 unknown-peer-as-subscribed-to-all fallback
- All 32 tests pass, providing regression safety for Plan 02 wiring

---
*Phase: 15-subscription-protocol*
*Completed: 2026-04-03*
