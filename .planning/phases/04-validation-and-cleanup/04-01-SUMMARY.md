---
phase: 04-validation-and-cleanup
plan: 01
subsystem: testing
tags: [rust, p2p, layer-tests, cargo, commonware, libp2p]

# Dependency graph
requires:
  - phase: 03-config-and-observability
    provides: P2pConfig enum with Local/Remote variants and max_message_size/deque_size fields
provides:
  - TestP2pMode enum with Local/Remote variants aligned to P2pConfig
  - layer-tests.toml with updated p2p = "remote" config
  - Complete P2pConfig constructions in e2e test harness (all fields specified)
  - libp2p removed from workspace and wavs package dependencies
affects: [future-test-runs, cargo-dependency-tree]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Use .. in P2pConfig destructuring to be resilient to future field additions"
    - "Always specify all P2pConfig fields explicitly in construction to avoid missing-field compile errors"

key-files:
  created: []
  modified:
    - packages/layer-tests/src/config.rs
    - packages/layer-tests/layer-tests.toml
    - packages/layer-tests/src/e2e/config.rs
    - packages/layer-tests/src/e2e/handles.rs
    - Cargo.toml
    - packages/wavs/Cargo.toml

key-decisions:
  - "TestP2pMode::Local replaces Mdns, TestP2pMode::Remote replaces Kademlia -- naming now matches P2pConfig variants exactly"
  - "libp2p workspace dep removed entirely -- zero Rust source references confirmed, no functionality lost"

patterns-established:
  - "P2pConfig construction: always include max_message_size: None and deque_size: None for forward-compat"
  - "P2pConfig destructuring: use .. to ignore optional fields added in Phase 3"

requirements-completed: [INT-02, INT-03]

# Metrics
duration: 10min
completed: 2026-03-17
---

# Phase 4 Plan 1: Test Harness Naming Fix and libp2p Removal Summary

**TestP2pMode enum renamed Local/Remote to match P2pConfig variants, P2pConfig constructions in e2e harness completed with max_message_size/deque_size, and libp2p workspace dependency fully eliminated**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-03-17T19:40:00Z
- **Completed:** 2026-03-17T19:50:54Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Renamed TestP2pMode::Mdns/Kademlia to Local/Remote, aligning test harness enum with production P2pConfig variants
- Added max_message_size: None and deque_size: None to all P2pConfig::Remote and P2pConfig::Local constructions in the test harness
- Added `..` pattern to P2pConfig::Remote destructuring in handles.rs for resilience against future field additions
- Removed libp2p 0.56 workspace dep block from root Cargo.toml and `libp2p = { workspace = true }` from packages/wavs/Cargo.toml
- Confirmed cargo check -p wavs -p layer-tests exits 0 after all changes
- Preserved hypercore/hypercore-protocol/hyperswarm (trigger subsystem deps)

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix test harness naming and P2pConfig field completeness** - `f3179b00` (feat)
2. **Task 2: Remove libp2p from workspace and package dependencies** - `b2a5a2a5` (chore)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `packages/layer-tests/src/config.rs` - TestP2pMode::Mdns -> Local, Kademlia -> Remote
- `packages/layer-tests/layer-tests.toml` - p2p = "remote" (was "kademlia"), updated comment
- `packages/layer-tests/src/e2e/config.rs` - Match arms renamed, max_message_size/deque_size added
- `packages/layer-tests/src/e2e/handles.rs` - TestP2pMode::Remote, .. in destructuring, new fields in reconstruction
- `Cargo.toml` - Removed libp2p 0.56 workspace dep block (17 lines)
- `packages/wavs/Cargo.toml` - Removed libp2p = { workspace = true } line

## Decisions Made
- TestP2pMode enum now exactly mirrors P2pConfig variant names (Local/Remote) for clarity and consistency
- libp2p removed without compatibility shim -- no source references existed so clean removal was safe

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Test harness compiles cleanly against updated P2pConfig with all fields
- libp2p no longer in the dependency tree for wavs package
- Ready for Phase 4 Plan 02 (further validation/cleanup work)

---
*Phase: 04-validation-and-cleanup*
*Completed: 2026-03-17*

## Self-Check: PASSED
- FOUND: .planning/phases/04-validation-and-cleanup/04-01-SUMMARY.md
- FOUND: commit f3179b00 (Task 1)
- FOUND: commit b2a5a2a5 (Task 2)
