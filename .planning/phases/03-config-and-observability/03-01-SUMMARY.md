---
phase: 03-config-and-observability
plan: 01
subsystem: p2p
tags: [commonware, p2p-status, p2p-config, broadcast-engine, struct-migration]

# Dependency graph
requires:
  - phase: 02-broadcast-and-routing
    provides: P2pStatus struct, P2pConfig enum, ServiceRouter, run_lookup_network, run_discovery_network
provides:
  - Clean P2pStatus struct without libp2p fields (6 fields: enabled, local_peer_id, listen_addresses, connected_peers, peer_ids, subscribed_services)
  - P2pConfig with optional max_message_size and deque_size tuning fields
  - Configurable broadcast Engine parameters (no more hardcoded 65536/128)
  - ServiceRouter method renamed from subscribed_topics to subscribed_services
affects: [03-03, p2p-config, observability]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Optional<T> with serde(default) for backward-compatible config extension"
    - "Accessor methods with unwrap_or(default) for config values with defaults"

key-files:
  created: []
  modified:
    - packages/types/src/http.rs
    - packages/wavs/src/subsystems/aggregator/p2p.rs
    - packages/layer-tests/src/e2e/handles.rs
    - packages/wavs/tests/p2p_broadcast_tests.rs
    - packages/wavs/tests/p2p_connectivity_tests.rs
    - packages/wavs/tests/p2p_identity_tests.rs

key-decisions:
  - "Keep HashMap import in http.rs -- used by DevTriggerStreamsInfo and DevTriggerStreamInfo"
  - "Use Option<u32> and Option<usize> with serde(default) for backward-compatible config extension"
  - "Pass max_message_size and deque_size as function parameters to run_lookup/discovery_network rather than passing full P2pConfig"

patterns-established:
  - "Config extension via Option<T> with serde(default): add optional fields to enum variants without breaking existing configs"
  - "Accessor methods with defaults: P2pConfig::max_message_size() -> u32 returns unwrap_or(65536)"

requirements-completed: [CFG-01, CFG-02, OBS-02]

# Metrics
duration: 19min
completed: 2026-03-17
---

# Phase 03 Plan 01: P2pStatus and P2pConfig Update Summary

**Clean P2pStatus struct (6 fields, no libp2p remnants), configurable P2pConfig with max_message_size/deque_size tuning, all consumers updated across 6 files**

## Performance

- **Duration:** 19 min
- **Started:** 2026-03-17T18:19:34Z
- **Completed:** 2026-03-17T18:38:23Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Removed libp2p-era fields (external_addresses, topic_peer_counts) from P2pStatus and renamed subscribed_topics to subscribed_services
- Added configurable max_message_size (Option<u32>) and deque_size (Option<usize>) to P2pConfig::Local and P2pConfig::Remote variants with accessor methods
- Replaced all hardcoded 65536 and 128 values in run_lookup_network and run_discovery_network with configurable parameters
- Updated all consumers: layer-tests handles.rs, CLI clients.rs (implicit via shared type), broadcast tests, connectivity tests, identity tests

## Task Commits

Each task was committed atomically:

1. **Task 1: Update P2pStatus struct and P2pConfig enum** - `b96e7332` (feat)
2. **Task 2: Update all P2pStatus consumers and fix tests** - Consumer changes were already committed as part of 03-00 research plan (commits `e58636ad`, `d769267e`, `421060cd`). Assertion message updates verified in-place.

**Plan metadata:** (pending final commit)

## Files Created/Modified
- `packages/types/src/http.rs` - P2pStatus reduced to 6 fields, subscribed_topics renamed to subscribed_services
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - P2pConfig tuning fields, accessor methods, ServiceRouter rename, configurable network parameters, updated GetStatus handlers
- `packages/layer-tests/src/e2e/handles.rs` - Removed external_addresses fallback, use listen_addresses directly
- `packages/wavs/tests/p2p_broadcast_tests.rs` - subscribed_topics -> subscribed_services, assertion messages updated, P2pConfig fields added
- `packages/wavs/tests/p2p_connectivity_tests.rs` - P2pConfig::Local/Remote constructors updated with max_message_size/deque_size
- `packages/wavs/tests/p2p_identity_tests.rs` - P2pConfig constructors updated with new fields

## Decisions Made
- Kept HashMap import in http.rs since it's used by DevTriggerStreamsInfo and other types (not just P2pStatus)
- Used Option<T> with serde(default) for max_message_size and deque_size to maintain backward compatibility with existing configs
- Passed tuning values as function parameters (not full P2pConfig) to run_lookup_network/run_discovery_network for cleaner separation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed P2pConfig constructors in p2p_connectivity_tests.rs**
- **Found during:** Task 2 (consumer updates)
- **Issue:** p2p_connectivity_tests.rs (not listed in plan) constructs P2pConfig::Local and P2pConfig::Remote without the new max_message_size/deque_size fields, causing compilation failure
- **Fix:** Added max_message_size: None, deque_size: None to all 8 P2pConfig constructors in the file
- **Files modified:** packages/wavs/tests/p2p_connectivity_tests.rs
- **Verification:** cargo test -p wavs --test p2p_connectivity_tests passes all 5 tests
- **Committed in:** Already committed as part of 03-00 plan (421060cd)

**2. [Rule 3 - Blocking] Fixed P2pConfig constructors in p2p_identity_tests.rs**
- **Found during:** Task 2 (consumer updates)
- **Issue:** p2p_identity_tests.rs constructs P2pConfig without new fields
- **Fix:** Added max_message_size: None, deque_size: None to both constructors
- **Files modified:** packages/wavs/tests/p2p_identity_tests.rs
- **Verification:** cargo test -p wavs --test p2p_identity_tests passes all 4 tests
- **Committed in:** Already committed as part of 03-00 plan (421060cd)

---

**Total deviations:** 2 auto-fixed (2 blocking - additional test files not listed in plan)
**Impact on plan:** Both auto-fixes necessary for compilation. No scope creep -- these files were direct consumers of the changed types.

## Issues Encountered
- Linter reverted some file changes during editing, requiring re-application of edits. This was a tooling issue, not a code issue.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- P2pStatus struct is clean and ready for Plan 03-03 to fill connected_peers and peer_ids from network state
- P2pConfig tuning fields are ready for operator use in wavs.toml
- All 20+ P2P tests passing (15 unit + 8 broadcast integration + 5 connectivity + 4 identity)

---
*Phase: 03-config-and-observability*
*Completed: 2026-03-17*
