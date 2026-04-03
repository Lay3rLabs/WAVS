---
phase: 14-subscription-data-structures
plan: 01
subsystem: p2p
tags: [commonware, ed25519, subscription, broadcast, recipients, serde_json]

# Dependency graph
requires: []
provides:
  - PeerSubscriptionMap with bidirectional forward/reverse indexes for service-to-peer mapping
  - SubscriptionAnnouncement wire type with serde_json encoding/decoding
  - SUBSCRIPTION_SENTINEL constant ([0xFF; 32]) for message discrimination
  - is_subscription_announcement() predicate function
  - ServiceRouter::subscribed_services_raw() accessor returning Vec<[u8; 32]>
  - 11 comprehensive unit tests covering all subscription data structures
affects: [15-subscription-protocol, 16-targeted-delivery]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Bidirectional index map (forward + reverse) for O(1) lookup and O(services_per_peer) cleanup"
    - "Sentinel-based message discrimination on existing P2P channel (extends HEARTBEAT_SERVICE_ID pattern)"
    - "serde_json payload encoding for control messages (matches existing Submission encoding pattern)"

key-files:
  created: []
  modified:
    - packages/wavs/src/subsystems/aggregator/p2p.rs

key-decisions:
  - "Used HashMap/HashSet (not DashMap) since bridge loop is single-threaded"
  - "Used serde_json for SubscriptionAnnouncement (matches existing P2pMessage payload pattern)"
  - "SUBSCRIPTION_SENTINEL = [0xFF; 32] distinct from HEARTBEAT_SERVICE_ID = [0x00; 32] and SHA-256 outputs"
  - "get_recipients() returns Recipients::All as defensive fallback when subscriber set is empty"

patterns-established:
  - "PeerSubscriptionMap: bidirectional index with handle_announcement/remove_peer/get_recipients API"
  - "SubscriptionAnnouncement: to_p2p_message/from_payload for sentinel-tagged P2pMessage encoding"

requirements-completed: [SUB-01, SUB-02, SUB-03, ANN-05]

# Metrics
duration: 10min
completed: 2026-04-03
---

# Phase 14 Plan 01: Subscription Data Structures Summary

**Bidirectional PeerSubscriptionMap with serde_json SubscriptionAnnouncement wire format and [0xFF;32] sentinel for per-service P2P targeting**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-03T14:00:04Z
- **Completed:** 2026-04-03T14:10:14Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- PeerSubscriptionMap with forward (service_id -> peers) and reverse (peer -> services) indexes, supporting handle_announcement, remove_peer, and get_recipients operations
- SubscriptionAnnouncement struct with to_p2p_message/from_payload for encoding as P2pMessage with SUBSCRIPTION_SENTINEL service_id
- SUBSCRIPTION_SENTINEL constant ([0xFF; 32]) and is_subscription_announcement() predicate, extending the existing heartbeat sentinel pattern
- ServiceRouter::subscribed_services_raw() returning raw [u8; 32] bytes for building announcements
- 11 new unit tests (22 total in p2p_broadcast_tests), all passing with 0 regressions across 87 lib tests

## Task Commits

Each task was committed atomically:

1. **Task 1: Add subscription data structures, sentinel constant, and ServiceRouter accessor** - `fa7bbd85` (feat)
2. **Task 2: Add comprehensive unit tests for subscription data structures** - `c1f87962` (test)

## Files Created/Modified
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Added PeerSubscriptionMap, SubscriptionAnnouncement, SUBSCRIPTION_SENTINEL, is_subscription_announcement(), ServiceRouter::subscribed_services_raw(), and 11 unit tests

## Decisions Made
- Used HashMap/HashSet (not DashMap) -- bridge loop is single-threaded, no concurrent access needed
- Used serde_json for SubscriptionAnnouncement encoding -- consistent with existing Submission payload pattern in P2pMessage
- SUBSCRIPTION_SENTINEL = [0xFF; 32] -- distinct from HEARTBEAT_SERVICE_ID ([0x00; 32]) and astronomically unlikely as a SHA-256 output
- get_recipients() returns Recipients::All when subscriber set is empty -- defensive fallback prevents silent message drops for newly deployed services

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Non-exhaustive match on Recipients enum in tests**
- **Found during:** Task 2 (test compilation)
- **Issue:** The commonware-p2p Recipients enum has three variants (All, Some, One), but plan's test match expressions only covered All and Some
- **Fix:** Changed `Recipients::All => panic!(...)` to `other => panic!("..., got {:?}", other)` using wildcard pattern to cover the One variant
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs (test module)
- **Verification:** All 22 tests compile and pass
- **Committed in:** c1f87962 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minimal -- match exhaustiveness fix required by Rust compiler, no design impact.

## Issues Encountered
- Worktree was initially on main branch (libp2p version of p2p.rs), required merge of bls-commonware branch to get the correct commonware-based code. This was a setup issue, not a plan issue.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All subscription data structures are in place for Phase 15 (subscription protocol) to wire into the bridge loops
- PeerSubscriptionMap ready to be instantiated in run_lookup_network and run_discovery_network bridge loops
- SubscriptionAnnouncement ready to be encoded/decoded in heartbeat and direct message paths
- ServiceRouter::subscribed_services_raw() ready for building subscription announcement payloads

---
*Phase: 14-subscription-data-structures*
*Completed: 2026-04-03*
