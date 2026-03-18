---
phase: 03-config-and-observability
plan: 03
subsystem: p2p
tags: [observability, peer-tracking, ed25519, commonware, p2p-status]

# Dependency graph
requires:
  - phase: 03-01
    provides: P2pStatus struct with connected_peers/peer_ids fields, GetStatus handler placeholders
provides:
  - Real connected peer tracking via broadcast ack and inbound message tracking
  - Integration test proving GetStatus returns real peer data after message exchange
affects: [http-api, monitoring, diagnostics]

# Tech tracking
tech-stack:
  added: []
  patterns: [Arc<RwLock<Vec<String>>> shared state for peer tracking across async bridge loop iterations]

key-files:
  created: []
  modified:
    - packages/wavs/src/subsystems/aggregator/p2p.rs
    - packages/wavs/tests/p2p_broadcast_tests.rs

key-decisions:
  - "Arc<RwLock<Vec<String>>> for connected_peers_tracker -- simple shared state sufficient for bridge loop scope"
  - "Broadcast ack replaces full tracker (not merge) -- latest broadcast recipients are the current connected set"
  - "Inbound message tracking uses contains-check dedup -- O(n) acceptable for small peer sets"

patterns-established:
  - "Peer tracking pattern: update from broadcast ack recipients (sender side) and inbound message senders (receiver side)"

requirements-completed: [OBS-01]

# Metrics
duration: 5min
completed: 2026-03-17
---

# Phase 03 Plan 03: Connected Peer Tracking Summary

**GetStatus returns real connected peer counts and hex-encoded Ed25519 peer IDs from broadcast ack and inbound message tracking**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-17T18:41:44Z
- **Completed:** 2026-03-17T18:47:18Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Wired connected peer tracking into both lookup and discovery bridge loops using Arc<RwLock<Vec<String>>>
- GetStatus now returns real connected_peers count and peer_ids from broadcast acknowledgment recipients and inbound message senders
- Replaced Wave 0 stub test with full integration test verifying peer tracking after broadcast exchange between two nodes

## Task Commits

Each task was committed atomically:

1. **Task 1: Add connected peer tracking to bridge loops** - `68976956` (feat)
2. **Task 2: Replace Wave 0 stub with full integration test** - `7685c8a1` (test)

## Files Created/Modified
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Added Arc<RwLock<Vec<String>>> connected_peers_tracker to both run_lookup_network and run_discovery_network, updated from broadcast ack recipients and inbound message senders, read in GetStatus handlers
- `packages/wavs/tests/p2p_broadcast_tests.rs` - Replaced OBS-01 stub with full test verifying connected_peers >= 1, hex pubkey format, and specific peer identity in peer_ids after broadcast exchange

## Decisions Made
- Arc<RwLock<Vec<String>>> chosen for connected_peers_tracker -- simple, sufficient for bridge loop scope, no cross-thread sharing needed beyond the bridge loop's own async iterations
- Broadcast ack handler replaces the full tracker contents (not additive merge) -- the latest broadcast recipients represent the current connected peer set most accurately
- Inbound message handler uses contains-check before push to avoid duplicates -- O(n) scan acceptable since peer sets are small (typically < 100 operators)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- OBS-01 (connected peer tracking) is complete and verified
- All 8 broadcast tests pass, all P2P tests pass
- Phase 03 (Config and Observability) is complete with all 4 plans executed

## Self-Check: PASSED

All files exist. All commits verified.

---
*Phase: 03-config-and-observability*
*Completed: 2026-03-17*
