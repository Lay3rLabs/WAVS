---
phase: 01-secure-peer-connectivity
plan: 02
subsystem: p2p
tags: [commonware, lookup, oracle, runtime, ed25519, rate-limiting]

# Dependency graph
requires:
  - phase: 01-01
    provides: Ed25519 identity derivation, P2pConfig enum, commonware dependencies
provides:
  - Commonware runtime scaffold on dedicated OS thread via spawn_commonware_runtime()
  - Lookup-mode networking with Oracle peer authorization via run_lookup_network()
  - parse_peer_address() and parse_authorized_peers() helpers
  - P2pHandle::new wired to commonware backend for Local config
  - Rate limiting active via Config::local() defaults (SEC-02)
  - Integration tests proving lookup connectivity and Oracle authorization
affects: [01-03-PLAN, phase-02]

# Tech tracking
tech-stack:
  added: [commonware-utils 2026.3.0, commonware-codec 2026.3.0, rand_core 0.6]
  patterns: [dedicated OS thread for commonware Runner, Oracle peer authorization, lookup-mode networking]

key-files:
  created:
    - packages/wavs/tests/p2p_connectivity_tests.rs
  modified:
    - packages/wavs/Cargo.toml
    - packages/wavs/src/subsystems/aggregator/p2p.rs

key-decisions:
  - "Config::local() defaults provide sufficient rate limiting (SEC-02) -- no explicit builder calls needed"
  - "PublicKey deserialization via commonware_codec::ReadExt::read() with &[u8] Buf implementation"
  - "Map::from_iter_dedup() used for Oracle peer map to handle potential duplicate keys gracefully"
  - "context.stop(0, None) called on bridge loop shutdown for clean commonware runtime termination"

patterns-established:
  - "Runtime scaffold: spawn_commonware_runtime() on std::thread with Runner::new(Config::new())"
  - "Oracle setup: own pubkey + peer_addresses + authorized_peers -> Map -> oracle.track(0, map)"
  - "Bridge loop: command_rx.recv() -> match P2pCommand -> handle or break on None"
  - "Test pattern: test_port(offset) with TEST_PORT_BASE=19000 for isolated P2P tests"

requirements-completed: [NET-02, NET-03, SEC-01, SEC-02, SEC-03]

# Metrics
duration: 12min
completed: 2026-03-17
---

# Phase 1 Plan 02: Commonware Runtime Scaffold and Lookup-Mode Networking Summary

**Commonware runtime on dedicated OS thread with lookup-mode P2P networking, Oracle peer authorization, and rate limiting via Config::local() defaults**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-17T15:22:43Z
- **Completed:** 2026-03-17T15:34:43Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Implemented spawn_commonware_runtime() on a dedicated OS thread, validating the highest-risk pattern (no Tokio nesting panic)
- Wired up lookup-mode networking with Oracle peer authorization, parse_peer_address() and parse_authorized_peers() helpers
- Confirmed rate limiting is active via Config::local() defaults: connection_rate_per_peer=1/s, handshake_rate_per_ip=16/s, handshake_rate_per_subnet=128/s (SEC-02)
- P2pHandle::new now creates a working commonware backend for P2pConfig::Local
- Two integration tests prove lookup connectivity (NET-02) and Oracle authorization (SEC-01)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement commonware runtime scaffold and lookup-mode networking with rate limiting**
   - `4c92d060` (feat: commonware runtime scaffold + lookup networking)
2. **Task 2: Write connectivity and authorization integration tests (TDD)**
   - `998b9b2e` (test: P2P connectivity and authorization tests)

## Files Created/Modified
- `packages/wavs/tests/p2p_connectivity_tests.rs` - 2 integration tests for lookup mode connectivity and Oracle authorization
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - spawn_commonware_runtime(), run_lookup_network(), parse_peer_address(), parse_authorized_peers(), pubkey_from_bytes(), updated P2pHandle::new
- `packages/wavs/Cargo.toml` - Added commonware-utils, commonware-codec, rand_core dependencies

## Decisions Made
- Used Config::local() defaults for rate limiting (SEC-02) rather than explicit builder calls -- verified source shows non-zero values for all three rate-limiting fields
- Used commonware_codec::ReadExt::read() with &[u8] as Buf for PublicKey deserialization from hex bytes
- Used Map::from_iter_dedup() instead of TryFrom to handle potential duplicate keys gracefully in Oracle peer map
- Called context.stop(0, None) in the bridge loop's None/shutdown branch to signal clean commonware runtime shutdown

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] P2pStatus struct fields differ from plan assumptions**
- **Found during:** Task 1 (run_lookup_network bridge loop)
- **Issue:** Plan assumed P2pStatus had fields `peer_id`, `connected_peers: Vec`, `mesh_peers`. Actual struct has `enabled`, `local_peer_id: Option<String>`, `connected_peers: usize`, `peer_ids`, `topic_peer_counts`, etc.
- **Fix:** Updated status construction to match the actual P2pStatus struct definition in packages/types/src/http.rs
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs
- **Verification:** cargo check -p wavs compiles cleanly
- **Committed in:** 4c92d060

**2. [Rule 3 - Blocking] Channel type is u64, not u32**
- **Found during:** Task 1 (network.register call)
- **Issue:** Plan used `0u32` for channel ID but commonware-p2p Channel type is u64
- **Fix:** Changed to `0u64`
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs
- **Verification:** cargo check -p wavs compiles cleanly
- **Committed in:** 4c92d060

**3. [Rule 3 - Blocking] CryptoRngCore not in commonware_runtime**
- **Found during:** Task 1 (trait bounds on context parameter)
- **Issue:** Plan assumed CryptoRngCore was in commonware_runtime. It is in rand_core 0.6.
- **Fix:** Used rand_core::CryptoRngCore directly, added rand_core = "0.6" dependency
- **Files modified:** packages/wavs/Cargo.toml, packages/wavs/src/subsystems/aggregator/p2p.rs
- **Verification:** cargo check -p wavs compiles cleanly
- **Committed in:** 4c92d060

**4. [Rule 3 - Blocking] Missing Resolver trait bound on context**
- **Found during:** Task 1 (Network::new requires Resolver)
- **Issue:** Network impl requires Spawner + BufferPooler + Clock + CryptoRngCore + Network + Resolver + Metrics, but plan only listed partial bounds
- **Fix:** Added commonware_runtime::Resolver to the trait bound list
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs
- **Verification:** cargo check -p wavs compiles cleanly
- **Committed in:** 4c92d060

---

**Total deviations:** 4 auto-fixed (4 blocking)
**Impact on plan:** All auto-fixes necessary for compilation. No scope creep. Plan's pseudocode was directionally correct but specific type details needed adjustment against actual API.

## Issues Encountered
- Tokio cleanup panic in tests: After tests complete, a "Cannot drop a runtime in a context where blocking is not allowed" panic occurs during background thread cleanup. This is cosmetic -- all test assertions pass before the panic, and the test binary exits with success. The panic occurs because the commonware runtime's internal Tokio runtime is dropped when the OS thread exits after the test framework has already started tearing down its own Tokio runtime. This will be addressed with proper shutdown handling in Phase 2.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Commonware runtime scaffold proven: spawn_commonware_runtime() works without nesting panics
- Lookup mode networking operational: two nodes connect on localhost
- Oracle authorization validated: unauthorized peers are excluded
- Ready for Plan 03 (discovery mode) and Phase 2 (broadcast, message routing)
- P2pHandle::new returns working handle for Local config; Remote config stub in place for Plan 03

## Self-Check: PASSED

All artifacts verified:
- packages/wavs/tests/p2p_connectivity_tests.rs: FOUND
- packages/wavs/src/subsystems/aggregator/p2p.rs: FOUND
- .planning/phases/01-secure-peer-connectivity/01-02-SUMMARY.md: FOUND
- Commit 4c92d060: FOUND
- Commit 998b9b2e: FOUND

---
*Phase: 01-secure-peer-connectivity*
*Completed: 2026-03-17*
