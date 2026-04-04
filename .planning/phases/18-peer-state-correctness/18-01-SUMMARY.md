---
phase: 18-peer-state-correctness
plan: 01
subsystem: aggregator-p2p
tags: [sub-03, compat-03, peer-pruning, backward-compat, targeted-delivery]
dependency_graph:
  requires: [phase-14-subscription-data-structures, phase-15-subscription-protocol, phase-16-targeted-delivery]
  provides: [heartbeat-peer-pruning, compat03-recipient-resolution]
  affects: [packages/wavs/src/subsystems/aggregator/p2p.rs]
tech_stack:
  added: []
  patterns: [heartbeat-diff-pruning, connected-peer-set-state, backward-compat-recipient-inclusion]
key_files:
  created: []
  modified:
    - packages/wavs/src/subsystems/aggregator/p2p.rs
decisions:
  - "Heartbeat-only pruning (not broadcast ack) to keep prune trigger predictable and bounded"
  - "connected_peer_set updated from both heartbeat and broadcast acks for freshest COMPAT-03 resolution"
  - "Prune block placed BEFORE retry drain so get_recipients uses pruned subscription state"
  - "Empty connected set preserves old get_recipients behavior (backward-compatible signature change)"
metrics:
  duration_seconds: 928
  completed: "2026-04-04T16:21:00Z"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 1
  lines_added: 432
  lines_removed: 40
---

# Phase 18 Plan 01: Peer State Correctness Summary

Heartbeat-driven peer pruning (SUB-03) and backward-compatible recipient resolution for pre-v1.3 nodes (COMPAT-03) in both bridge loops.

## What Changed

### PeerSubscriptionMap API (Task 1)

- Added `tracked_peers()` method returning `HashSet<ed25519::PublicKey>` of all peers with subscription entries. Used by heartbeat prune to compute set difference against connected peers.

- Modified `get_recipients()` signature from `(&self, service_id: &[u8; 32])` to `(&self, service_id: &[u8; 32], connected_peers: &HashSet<ed25519::PublicKey>)`. The new parameter enables COMPAT-03: connected peers that have never sent a subscription announcement (pre-v1.3 nodes) are unconditionally included in the recipient set via `has_announced()` check.

- `has_announced()` is now called in production code (inside `get_recipients()`), resolving the dead_code clippy warning from Phase 15.

### Bridge Loop Wiring (Task 2)

- Added `connected_peer_set: HashSet<ed25519::PublicKey>` state variable to both `run_lookup_network` and `run_discovery_network`.

- `connected_peer_set` updated from broadcast ack recipients (Publish handler) and heartbeat ack recipients (heartbeat tick arm) -- 4 update sites total (2 per loop).

- SUB-03 prune block in each heartbeat tick arm: computes `tracked_peers().difference(&connected_peer_set)`, calls `remove_peer()` and `known_peers.remove()` for each departed peer. Placed before retry drain so `get_recipients()` in retry path uses pruned state.

- All 6 production `get_recipients()` call sites (3 per loop) now pass `&connected_peer_set` instead of `&HashSet::new()`.

### Test Coverage (Task 1)

8 new unit tests added:
- `test_tracked_peers_empty` -- empty map returns empty set
- `test_tracked_peers_returns_announced_peers` -- returns both after announcements
- `test_tracked_peers_after_remove` -- removed peer no longer tracked
- `test_heartbeat_prune_departed_peer` -- SUB-03: prune removes departed, keeps connected
- `test_prune_noop_all_connected` -- no-op when all tracked peers are connected
- `test_get_recipients_includes_unannounced_connected_peers` -- COMPAT-03: legacy peers included
- `test_get_recipients_all_announced_no_legacy` -- announced peers only get subscribed services
- `test_get_recipients_empty_connected_set_preserves_old_behavior` -- backward compat with empty set

All ~24 existing `get_recipients()` test calls updated to pass `&HashSet::new()` preserving previous behavior.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 237b10ac | Add tracked_peers() and COMPAT-03-aware get_recipients() with 8 new tests |
| 2 | 08ff32ff | Wire heartbeat prune and connected_peer_set into both bridge loops |

## Verification Results

- `cargo test -p wavs --lib -- aggregator::p2p`: 45 passed, 0 failed
- `cargo test -p wavs`: all tests pass
- `cargo clippy -p wavs -- -W dead_code`: no `has_announced` dead_code warning
- Pre-existing clippy warnings (non-binding let, too_many_arguments, cloned_ref_to_slice_refs) are unchanged -- out of scope for this plan
- `connected_peer_set` state variable: 2 occurrences (1 per loop)
- `connected_peer_set` update sites: 4 (2 per loop: broadcast ack + heartbeat ack)
- `tracked_peers()` calls in heartbeat: 2 (1 per loop)
- `remove_peer(departed)` in prune blocks: 2 (1 per loop)
- `known_peers.remove(departed)` in prune blocks: 2 (1 per loop)
- Production `get_recipients` with `connected_peer_set`: 6 (3 per loop)
- Both loops have character-for-character identical changes

## Requirements Satisfied

| Requirement | Status | Evidence |
|-------------|--------|----------|
| SUB-03 | Complete | `tracked_peers().difference(&connected_peer_set)` prune loop in both heartbeat tick arms |
| COMPAT-03 | Complete | `get_recipients()` includes un-announced peers via `has_announced()` check; 2 dedicated tests |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Production call sites needed temporary placeholder**
- **Found during:** Task 1
- **Issue:** Changing `get_recipients()` signature broke 6 production call sites that weren't being updated until Task 2
- **Fix:** Used `&HashSet::new()` as temporary placeholder in production code during Task 1 (preserves old behavior), replaced with `&connected_peer_set` in Task 2
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs
- **Commit:** 237b10ac (temporary), 08ff32ff (final)

## Known Stubs

None. All code paths are fully wired with no placeholder data.
