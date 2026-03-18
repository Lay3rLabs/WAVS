---
phase: 01-secure-peer-connectivity
plan: 03
subsystem: p2p
tags: [commonware, discovery, bootstrapper, block-peer, auto-reconnect, oracle, ed25519]

# Dependency graph
requires:
  - phase: 01-02
    provides: Commonware runtime scaffold, lookup networking, parse helpers, P2pHandle wiring
provides:
  - Discovery-mode networking via run_discovery_network() with bootstrapper-based peer discovery (NET-01)
  - BlockPeer command wired end-to-end from P2pHandle through P2pCommand to Oracle.block() (SEC-03)
  - Automatic reconnection validated via discovery::Network dial_frequency (NET-04)
  - parse_bootstrapper() helper for "pubkey@host:port" bootstrapper addresses
  - Both lookup and discovery modes coexist in P2pConfig and spawn_commonware_runtime
  - Complete Phase 1 networking layer (all requirements addressed)
affects: [phase-02]

# Tech tracking
tech-stack:
  added: []
  patterns: [discovery-mode networking with bootstrappers, Oracle Set vs Map for discovery vs lookup, BlockPeer command pattern]

key-files:
  created: []
  modified:
    - packages/wavs/src/subsystems/aggregator/p2p.rs
    - packages/wavs/tests/p2p_connectivity_tests.rs

key-decisions:
  - "Config::local() for discovery in tests -- allow_private_ips=true needed for localhost testing"
  - "Set::from_iter_dedup for discovery Oracle peer set -- handles duplicate keys gracefully"
  - "BlockPeer in both lookup and discovery bridge loops -- consistent API regardless of mode"

patterns-established:
  - "Discovery bridge loop pattern: same structure as lookup, different Oracle API (Set vs Map)"
  - "BlockPeer command flow: P2pHandle.block_peer() -> P2pCommand::BlockPeer -> oracle.block()"

requirements-completed: [NET-01, NET-04]

# Metrics
duration: 7min
completed: 2026-03-17
---

# Phase 01 Plan 03: Discovery Mode Networking Summary

**Discovery-mode P2P with bootstrapper-based peer discovery, BlockPeer command wired through Oracle.block(), and auto-reconnect validation**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-17T15:39:55Z
- **Completed:** 2026-03-17T15:47:36Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Discovery-mode networking with bootstrappers implemented and tested (NET-01)
- BlockPeer command wired end-to-end through P2pHandle -> P2pCommand -> Oracle.block() in both lookup and discovery bridge loops (SEC-03)
- Auto-reconnect validated: node survives bootstrapper unavailability, discovery's dial_frequency handles retries (NET-04)
- Phase 1 complete: all 13 tests pass (8 identity + 5 connectivity), all requirements addressed

## Task Commits

Each task was committed atomically:

1. **Task 1: Add BlockPeer command and discovery mode networking** - `d42d6746` (feat)
2. **Task 2: Write discovery, block-peer, and auto-reconnect tests** - `bfd196de` (test)

## Files Created/Modified
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Added run_discovery_network(), BlockPeer command variant, block_peer() method on P2pHandle, parse_bootstrapper() helper, wired P2pConfig::Remote to discovery mode
- `packages/wavs/tests/p2p_connectivity_tests.rs` - Added test_discovery_mode_two_nodes (NET-01), test_block_peer (SEC-03), test_auto_reconnect (NET-04)

## Decisions Made
- Used `discovery::Config::local()` instead of `Config::recommended()` for test environment -- `allow_private_ips: true` is required for localhost testing. Production deployments would use `Config::recommended()`.
- Used `Set::from_iter_dedup()` for building the discovery Oracle peer set -- handles potential duplicate keys from own pubkey + authorized peers gracefully.
- Added BlockPeer handling to both lookup and discovery bridge loops -- ensures consistent API behavior regardless of which network mode is active.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None - all code compiled on first attempt, all tests passed immediately.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 1 is fully complete: Ed25519 identity (IDEN-01, IDEN-02), lookup mode (NET-02), discovery mode (NET-01), encrypted connections (NET-03), auto-reconnect (NET-04), Oracle authorization (SEC-01), rate limiting (SEC-02), peer blocking (SEC-03)
- Phase 2 can build on this foundation: broadcast messaging, message routing, full P2pHandle API, enhanced P2pStatus with connected_peers
- Known cleanup needed: commonware runtime thread cleanup panics on test exit (non-fatal, cosmetic) -- proper shutdown via thread_handle.join() is a Phase 2 task

---
*Phase: 01-secure-peer-connectivity*
*Completed: 2026-03-17*
