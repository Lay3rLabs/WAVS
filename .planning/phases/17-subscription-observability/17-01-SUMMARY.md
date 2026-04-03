---
phase: 17-subscription-observability
plan: 01
subsystem: aggregator-p2p
tags: [observability, p2p, subscription, http-api]
dependency_graph:
  requires: [PeerSubscriptionMap, P2pStatus, ServiceRouter]
  provides: [peer_subscription_counts, OBS-01]
  affects: [/p2p/status endpoint, Tauri P2P dashboard]
tech_stack:
  added: []
  patterns: [hex-encoded service counts, serde(default) for backward compat]
key_files:
  created: []
  modified:
    - packages/wavs/src/subsystems/aggregator/p2p.rs
    - packages/types/src/http.rs
    - packages/wavs/src/subsystems/aggregator/p2p_status_tests.rs
    - app/src/types/index.ts
decisions:
  - const_hex::encode for service_id keys (consistent with existing P2pStatus hex encoding)
  - serde(default) on peer_subscriptions for backward compat with older nodes
metrics:
  duration_seconds: 593
  completed: "2026-04-03T16:43:09Z"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 4
---

# Phase 17 Plan 01: Subscription Observability Summary

Per-service peer subscription counts exposed via /p2p/status endpoint using peer_subscription_counts() on PeerSubscriptionMap, both bridge loop GetStatus handlers wired, serde(default) backward compat, TypeScript mirror updated.

## What Was Done

### Task 1: Add peer_subscription_counts() method with unit test (TDD)

**Commits:** 79f0b0b9 (RED), 294f98cd (GREEN)

Added `peer_subscription_counts()` method to `PeerSubscriptionMap` that returns `HashMap<String, usize>` where keys are hex-encoded service_id bytes and values are peer counts. Uses `const_hex::encode` consistent with existing hex encoding throughout the P2P module.

TDD flow:
- RED: Test called nonexistent method, compilation failed as expected
- GREEN: Implemented method with iterator over `service_to_peers`, test passes
- No refactor needed (implementation is minimal)

Unit test covers: empty map returns empty, correct counts after subscriptions, counts update after peer removal.

### Task 2: Wire peer_subscriptions into P2pStatus, GetStatus handlers, serialization test, TypeScript

**Commit:** b1edef0b

Five coordinated changes:
1. **P2pStatus struct** (`packages/types/src/http.rs`): Added `peer_subscriptions: HashMap<String, usize>` with `#[serde(default)]` for backward compat
2. **Lookup loop GetStatus** (`p2p.rs` line ~931): Added `peer_subscriptions: peer_subscriptions.peer_subscription_counts()`
3. **Discovery loop GetStatus** (`p2p.rs` line ~1412): Identical change
4. **Serialization test** (`p2p_status_tests.rs`): Asserts `peer_subscriptions` key exists, is an object, defaults to empty
5. **TypeScript** (`app/src/types/index.ts`): Added `peer_subscriptions: Record<string, number>` to P2pStatus interface

## Verification Results

- `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_counts` -- PASS
- `cargo test -p wavs --lib -- p2p_status_tests` -- PASS
- `cargo test -p wavs --lib -- p2p` -- 37 passed, 0 failed
- `grep peer_subscription_counts p2p.rs` -- exactly 2 matches in GetStatus handlers
- `serde(default)` confirmed before `peer_subscriptions` field

Note: Pre-existing `cargo fmt` and `clippy` warnings exist in unrelated files (submit.rs, commands.rs, queue.rs, etc.). These are out of scope for this plan. The `has_announced` dead_code warning was documented in Phase 15 as pre-existing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Worktree on wrong branch base**
- **Found during:** Pre-execution setup
- **Issue:** Worktree was checked out at `main` branch tip (e5e97f39), missing all phase 14-16 subscription protocol infrastructure (PeerSubscriptionMap, ServiceRouter, commonware P2P backend)
- **Fix:** Fast-forward merged `bls-commonware` branch into worktree to get correct base code
- **Files affected:** All (full branch merge)
- **Impact:** None -- fast-forward merge, no conflicts

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| `const_hex::encode` for service_id keys | Consistent with existing hex encoding in P2pStatus (peer IDs, service IDs) |
| `#[serde(default)]` on peer_subscriptions | Backward compat with older nodes that don't send this field; HashMap default is empty map |

## Known Stubs

None -- all data flows are fully wired. `peer_subscription_counts()` reads live state from `PeerSubscriptionMap` which is populated by subscription announcements in both bridge loops.
