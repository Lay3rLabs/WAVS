---
phase: 02-broadcast-and-routing
plan: 02
subsystem: p2p-networking
tags: [commonware-broadcast, p2p, broadcast-engine, service-routing, deduplication, catch-up]

# Dependency graph
requires:
  - phase: 02-broadcast-and-routing/01
    provides: "P2pMessage (Codec + Digestible), ServiceRouter, RetryQueue, single-channel bridge loops"
provides:
  - "Two-channel broadcast architecture (Engine + direct forwarding)"
  - "All P2pCommand handlers (Publish, Subscribe, Unsubscribe, GetStatus, BlockPeer)"
  - "Application-level message deduplication via seen_digests HashSet"
  - "Inbound message forwarding to Aggregator as AggregatorCommand::Receive"
  - "Push-based catch-up via Engine cache (CATCH-01)"
  - "Bounded message storage per peer via deque_size (CATCH-02)"
  - "7 integration tests covering broadcast, filtering, dedup, retry, catch-up, API"
affects: [phase-03-consensus-and-verification]

# Tech tracking
tech-stack:
  added: [commonware-broadcast buffered Engine, commonware-broadcast Broadcaster trait]
  patterns: [two-channel P2P architecture, tokio mpsc bridge for commonware-to-tokio, application-level digest dedup]

key-files:
  created:
    - packages/wavs/tests/p2p_broadcast_tests.rs
  modified:
    - packages/wavs/src/subsystems/aggregator/p2p.rs

key-decisions:
  - "Two-channel architecture (Engine on channel 0, direct on channel 1) for simultaneous caching and real-time forwarding"
  - "Tokio mpsc bridge task to forward commonware Receiver to tokio::select!-compatible channel"
  - "Encode::encode() returns Bytes (Into<IoBufs>) for direct_sender.send() -- not Vec<u8>"
  - "ack_rx.await works in commonware runtime because commonware_utils::channel::oneshot re-exports tokio::sync::oneshot"
  - "Spawner::spawn takes FnOnce(Self) -> Fut, not (label, future) -- fixed from plan's context.spawn() pattern"

patterns-established:
  - "Two-channel broadcast: channel 0 for Engine caching, channel 1 for direct application forwarding"
  - "Inbound bridge: context.clone().spawn(|_ctx| async { receiver.recv() -> tokio_tx.send() })"
  - "P2pCommand handlers inline in bridge loop (not separate function) for access to mailbox, direct_sender, retry_queue"
  - "seen_digests HashSet with clear-on-capacity for bounded dedup memory"

requirements-completed: [BCAST-01, BCAST-02, BCAST-04, BCAST-05, CATCH-01, CATCH-02, INT-01]

# Metrics
duration: 17min
completed: 2026-03-17
---

# Phase 02 Plan 02: Broadcast Engine Integration Summary

**Two-channel broadcast architecture with Engine caching, direct forwarding, digest dedup, service filtering, retry queue, and 7 integration tests proving end-to-end P2P message delivery**

## Performance

- **Duration:** 17 min
- **Started:** 2026-03-17T17:21:46Z
- **Completed:** 2026-03-17T17:38:53Z
- **Tasks:** 3
- **Files modified:** 2 (1 created, 1 modified)

## Accomplishments
- Wired commonware-broadcast buffered Engine into both lookup and discovery bridge loops with two-channel architecture
- Implemented all 5 P2pCommand handlers (Publish, Subscribe, Unsubscribe, GetStatus, BlockPeer) with broadcast via Engine + direct channel
- Added application-level message deduplication via seen_digests HashSet ensuring exactly-once delivery to Aggregator
- Created 7 integration tests with real P2P connections verifying broadcast delivery, service filtering, dedup, retry, catch-up, bounded deque, and API preservation

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire broadcast Engine into bridge loops** - `fcf445d3` (feat)
2. **Task 2: Implement all P2pCommand handlers** - `3c242516` (feat)
3. **Task 3: Write integration tests** - `07e68ee8` (test)

## Files Created/Modified
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Two-channel broadcast Engine integration, all P2pCommand handlers, inbound message forwarding with dedup and service filtering
- `packages/wavs/tests/p2p_broadcast_tests.rs` - 7 integration tests for BCAST-01, BCAST-02, BCAST-04, BCAST-05, CATCH-01, CATCH-02, INT-01

## Decisions Made
- **Two-channel architecture:** Channel 0 for Engine (caching/catch-up), channel 1 for direct forwarding to Aggregator. This gives both real-time inbound delivery and Engine-based caching. Dual-send overhead is negligible at WAVS message rates.
- **Tokio mpsc bridge task:** Spawned via `context.clone().spawn()` to bridge commonware Receiver to tokio::select!-compatible channel, since commonware's async primitives may not compose with tokio::select! directly.
- **Encode::encode() for direct channel:** P2pMessage encoded to `Bytes` (not `Vec<u8>`) via `Encode::encode()` auto-derived from `Write + EncodeSize`. `Bytes` implements `Into<IoBufs>` required by `Sender::send()`.
- **Inline command handlers:** All P2pCommand handlers are implemented inline in the bridge loop (not in a separate function) because they need access to `mailbox`, `direct_sender`, `retry_queue`, and `service_router` which are local to the loop scope.
- **CATCH-01 scoped as push-based:** Engine's internal relay delivers cached messages to reconnecting peers. No application-level pull mechanism needed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed Spawner::spawn API mismatch**
- **Found during:** Task 1 (Wire broadcast Engine)
- **Issue:** Plan used `context.spawn("label", async { ... })` but Spawner::spawn takes `FnOnce(Self) -> Fut` (one argument, not label + future)
- **Fix:** Changed to `context.clone().spawn(move |_ctx| async move { ... })`
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs
- **Verification:** cargo check passes
- **Committed in:** fcf445d3 (Task 1 commit)

**2. [Rule 3 - Blocking] Added missing Sender trait import**
- **Found during:** Task 2 (Implement P2pCommand handlers)
- **Issue:** `direct_sender.send()` failed to compile because `commonware_p2p::Sender` trait was not in scope
- **Fix:** Added `use commonware_p2p::{Recipients, Sender as P2pSender};`
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs
- **Verification:** cargo check passes
- **Committed in:** 3c242516 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking issues)
**Impact on plan:** Both fixes required for compilation. No scope creep.

## Issues Encountered
- The `Encode::encode()` method returns `Bytes` (not `Vec<u8>` as the plan suggested). This is actually better since `Bytes` directly implements `Into<IoBufs>`, avoiding an intermediate `Vec<u8>` allocation.
- Runtime drop panics during test cleanup ("Cannot drop a runtime in a context where blocking is not allowed") are cosmetic teardown noise from the commonware Runner's internal Tokio runtime. All 7 tests pass despite these panics.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 2 broadcast and routing is complete. All requirements (BCAST-01/02/04/05, CATCH-01/02, INT-01) are verified by tests.
- 19 total P2P tests pass (12 unit + 7 integration) across identity, connectivity, and broadcast.
- Ready for Phase 3: Consensus and Verification.
- **Open concern:** Catch-up guarantee equivalence (buffered Engine is peer-scoped, previous protocol was service-scoped) should be validated in production-like scenarios.

## Self-Check: PASSED

- [x] packages/wavs/src/subsystems/aggregator/p2p.rs exists
- [x] packages/wavs/tests/p2p_broadcast_tests.rs exists
- [x] .planning/phases/02-broadcast-and-routing/02-02-SUMMARY.md exists
- [x] Commit fcf445d3 exists (Task 1)
- [x] Commit 3c242516 exists (Task 2)
- [x] Commit 07e68ee8 exists (Task 3)
- [x] cargo check -p wavs exits 0
- [x] All 19 P2P tests pass (12 unit + 7 integration)

---
*Phase: 02-broadcast-and-routing*
*Completed: 2026-03-17*
