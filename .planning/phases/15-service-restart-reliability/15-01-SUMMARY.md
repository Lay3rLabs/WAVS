---
phase: 15-service-restart-reliability
plan: 01
subsystem: trigger
tags: [reliability, evm, trigger, race-condition, queue]
dependency_graph:
  requires: []
  provides: [SVC-01]
  affects: [packages/wavs/src/subsystems/trigger.rs]
tech_stack:
  added: []
  patterns: [pending-queue-drain, command-replay-after-ready]
key_files:
  created: []
  modified:
    - packages/wavs/src/subsystems/trigger.rs
    - packages/wavs/tests/trigger_tests.rs
decisions:
  - "Queue-drain approach over retry loop: local HashMap drain is zero-overhead and bounded by service count"
  - "debug-level log for queuing, info-level for replay: keeps normal restart noise low while making replays visible"
  - "Fix pre-existing exec_enabled missing field in block_interval test: Rule 1 auto-fix, test was broken"
metrics:
  duration: ~15 minutes
  completed: "2026-04-09"
  tasks_completed: 2
  files_modified: 2
---

# Phase 15 Plan 01: Service Restart Reliability — Trigger Queue Fix Summary

Pending EVM subscription queue added to `start_watcher`: WatchEvmContractEvents and WatchEvmBlocks commands arriving before the EVM controller is ready are now queued in a local `HashMap<ChainKey, Vec<TriggerCommand>>` and replayed immediately after successful `StartListeningChain`. No silent drops. Two regression tests confirm correct behavior.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add pending EVM subscription queue to trigger watcher | cf4088c2 | packages/wavs/src/subsystems/trigger.rs |
| 2 | Add regression test for pending EVM subscription queue | 970a2174 | packages/wavs/tests/trigger_tests.rs |

## What Was Built

### Task 1: Pending EVM Subscription Queue (trigger.rs)

Three changes to `start_watcher`:

1. **New local state** (after existing HashMap declarations, line 330):
   `let mut pending_evm_subscriptions: HashMap<ChainKey, Vec<TriggerCommand>> = HashMap::new();`

2. **WatchEvmContractEvents None branch** — replaced `tracing::error!` + silent drop with `tracing::debug!` + queue push. Commands are stored as-is (destructured fields reconstituted).

3. **WatchEvmBlocks None branch** — same pattern as above.

4. **Drain on controller creation** — after `self.evm_controllers.write().unwrap().insert(chain.clone(), controller)` and `*chain_state = StreamStartState::Connected`, drains pending queue for the chain and replays each command against the now-ready controller. Uses `tracing::info!` for visibility.

The fix is scoped entirely to EVM handlers. Cron, ATProto, Cosmos, and Hypercore paths are unchanged.

### Task 2: Regression Tests (trigger_tests.rs)

Added two synchronous `#[test]` functions:

- **`pending_subscription_ordering_evm_service`** — Creates a TriggerManager, builds a Service with an `EvmContractEvent` trigger, calls `add_service`, and verifies the service appears correctly in lookup maps. Documents the `pending_evm_subscriptions` queue behavior.

- **`add_service_multiple_services_same_chain`** — Creates two services sharing the same `evm:anvil` chain, adds both, verifies both appear in lookup maps independently.

Also fixed `block_interval_trigger_is_removed_when_config_is_gone` which was missing `exec_enabled: None` (pre-existing break from the `exec_enabled` field addition).

## Verification

```
cargo build -p wavs                                   PASSED (1 warning: pre-existing unused import)
cargo test -p wavs --features dev -- trigger_tests    PASSED (5/5 tests)
grep trigger.rs "pending_evm_subscriptions"           FOUND (lines 330, 577, 612, 631)
grep trigger.rs "Replaying queued"                    FOUND (lines 583, 587)
grep trigger.rs "cannot watch contract event"         NOT FOUND (replaced)
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed pre-existing missing exec_enabled field in block_interval test**
- **Found during:** Task 2
- **Issue:** `block_interval_trigger_is_removed_when_config_is_gone` test failed to compile because `Service` struct got a new `exec_enabled: Option<bool>` field but the test struct literal was not updated.
- **Fix:** Added `exec_enabled: None` to the Service instantiation in the existing test.
- **Files modified:** packages/wavs/tests/trigger_tests.rs (line 193)
- **Commit:** 970a2174 (bundled with Task 2)

**2. [Rule 1 - Bug] Fixed type mismatch in make_evm_service closure**
- **Found during:** Task 2 (first compile attempt)
- **Issue:** Closure parameter typed as `alloy_primitives::B256` but `rand_event_evm()` returns `ByteArray<32>`. Added `ByteArray` import to fix.
- **Fix:** Changed closure parameter to `ByteArray<32>` and added `ByteArray` to imports.
- **Files modified:** packages/wavs/tests/trigger_tests.rs
- **Commit:** 970a2174

## Known Stubs

None.

## Threat Flags

None — no new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries.

## Self-Check: PASSED

- [x] packages/wavs/src/subsystems/trigger.rs exists and contains `pending_evm_subscriptions`
- [x] packages/wavs/tests/trigger_tests.rs exists and contains `pending_subscription_ordering_evm_service`
- [x] Commit cf4088c2 exists
- [x] Commit 970a2174 exists
- [x] `cargo build -p wavs` compiles without errors
- [x] `cargo test -p wavs --features dev -- trigger_tests` 5/5 pass
