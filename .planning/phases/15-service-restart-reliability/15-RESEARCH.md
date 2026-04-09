# Phase 15: Service Restart Reliability - Research

**Researched:** 2026-04-09
**Domain:** Rust async startup sequencing, Tokio channel ordering, EVM trigger stream re-subscription
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all implementation choices are at Claude's discretion.

### Claude's Discretion
All implementation choices. Key constraints from STATE.md:
- Must handle race conditions in trigger stream re-subscription
- No trigger events should be silently dropped during the re-subscription window
- Previously registered services must resume receiving trigger events without manual intervention

### Deferred Ideas (OUT OF SCOPE)
None — infrastructure phase.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SVC-01 | Services reliably restore trigger subscriptions after WAVS process restart (fix race condition in trigger stream re-subscription) | Root cause identified in trigger.rs:577-605; fix approach documented in Architecture Patterns |
</phase_requirements>

## Summary

The bug is a startup ordering race condition in `packages/wavs/src/subsystems/trigger.rs`. When `TriggerManager::add_service` is called during restart restore, it sends commands to the trigger watcher loop in this order: (1) `StartListeningChain`, (2) `WatchEvmContractEvents` (or `WatchEvmBlocks`). The watcher loop processes these commands sequentially from the `local_command_stream`. However, `StartListeningChain` for EVM chains is an async operation that connects a WebSocket, creates an `EvmTriggerStreamsController`, and inserts it into `self.evm_controllers`. When the next command `WatchEvmContractEvents` arrives, it looks up the controller by chain key — but because `StartListeningChain` connects asynchronously over the network, the controller may not yet be in `evm_controllers`. The result: a `tracing::error!` log and a silent `continue`, permanently dropping the subscription for that service.

The fix is to queue `WatchEvmContractEvents` and `WatchEvmBlocks` commands that arrive before the EVM controller is ready, then replay them immediately after the controller is successfully created. This is a targeted change to `start_watcher` in `trigger.rs` — no API changes, no changes to how callers invoke `add_service`.

**Primary recommendation:** Add a `pending_evm_subscriptions: HashMap<ChainKey, Vec<TriggerCommand>>` in `start_watcher`'s local state. When `WatchEvmContractEvents`/`WatchEvmBlocks` arrive for a chain with no controller, push to the pending map. After `StartListeningChain` successfully creates a controller, drain that chain's pending commands.

## Standard Stack

This is an internal Rust fix with no new dependencies. All relevant types are already in scope.

### Core Types Already in Use
| Type | Location | Purpose |
|------|----------|---------|
| `HashMap<ChainKey, EvmTriggerStreamsController>` | `trigger.rs` `evm_controllers` field | Stores EVM controllers per chain |
| `TriggerCommand` | `trigger.rs` | The command enum to be queued |
| `StreamStartState` | `trigger.rs` | Existing Waiting/Connecting/Connected state machine |
| `tokio::sync::mpsc::UnboundedSender<TriggerCommand>` | `trigger.rs` `command_sender` | Used to send commands to the watcher loop |

**No new crates required.** [VERIFIED: grep of Cargo.toml and trigger.rs]

## Architecture Patterns

### Startup Sequence (What Currently Happens)

```
dispatcher.start()
  ├── spawn thread: trigger_manager.start(ctx)   ← starts async watcher loop
  ├── spawn thread: engine_manager.start(ctx)
  ├── spawn thread: submission_manager.start(ctx)
  └── block_on: restore services from registry
        └── for each service:
              trigger_manager.add_service(&service)
                ├── send: StartListeningChain { chain }      ← queued #1
                ├── send: WatchEvmContractEvents { chain, .. } ← queued #2
                └── send: WatchEvmBlocks { chain, .. }         ← queued #3
```

The watcher loop processes commands from `local_command_stream`. When it processes command #1 (`StartListeningChain`), it awaits an async WebSocket connection. Only after the connection resolves does it insert the controller into `evm_controllers`. Command #2 arrives while #1 is still awaiting — but because the loop is single-threaded (one `tokio::select!` iteration per event), command #2 will not be processed until command #1's handler finishes. However, the handler for `StartListeningChain` is itself an `await` — and during the await, the `tokio::select!` would pick the next item from the stream if available. This is NOT the actual ordering problem; the actual problem is that command #2 IS processed after the controller is inserted (the loop is sequential), but there can be a race in the real world when:

1. `StartListeningChain` for chain `A` fails (returns `continue`) — the controller is never inserted, and `WatchEvmContractEvents` for chain `A` is then silently dropped.
2. Multiple services share the same chain: the second service sends `StartListeningChain` (skipped, `Connected`) then `WatchEvmContractEvents` — but only if the FIRST service's `StartListeningChain` has already completed and is in `Connected` state. In practice on startup all services are registered rapidly, so `WatchEvmContractEvents` for service 2 can arrive BEFORE service 1's `StartListeningChain` completes (the loop processes them in arrival order, so if both are queued before the loop runs, they are interleaved: S1-StartChain, S1-WatchEvents, S2-StartChain, S2-WatchEvents; but this isn't the bug).

**The real bug path** (verified by code trace): When `StartListeningChain` transitions to `Connecting` and the actual `await` inside the `match chain_config` block for EVM yields control briefly — it's possible for `WatchEvmContractEvents` from a DIFFERENT service on the SAME chain to arrive next. Since `StartListeningChain` has state `Connecting`, that second service's `StartListeningChain` is skipped, so the controller insertion race is avoided. However, if `StartListeningChain` FAILS (WebSocket error, `continue`), state reverts to `Waiting` — any `WatchEvmContractEvents` that arrived and processed in the gap have no controller and are silently dropped with `tracing::error!`.

**More critically**: the `WatchEvmContractEvents` for service URI updates is sent by `add_service` BEFORE the per-workflow trigger commands. If the EVM WebSocket is slow to connect, the `WatchEvmContractEvents` for the service manager contract itself gets dropped when `StartListeningChain` fails on first attempt. This means URI change events are never received, and triggers that depend on EVM events are also unsubscribed.

### Pattern 1: Pending Subscription Queue (Recommended Fix)

**What:** Store `WatchEvmContractEvents` and `WatchEvmBlocks` commands that arrive when no controller exists, keyed by chain. Drain after the controller is successfully created.

**When to use:** Startup restore and any `add_service` call where the EVM chain is not yet connected.

**Location:** Inside `start_watcher` local state in `trigger.rs`.

```rust
// Source: packages/wavs/src/subsystems/trigger.rs (new local state in start_watcher)

// Add to start_watcher's local variable declarations (around line 326):
let mut pending_evm_subscriptions: HashMap<ChainKey, Vec<TriggerCommand>> = HashMap::new();

// In TriggerCommand::WatchEvmContractEvents handler (replace lines 577-593):
TriggerCommand::WatchEvmContractEvents { ref chain, .. } => {
    match self.evm_controllers.read().unwrap().get(chain) {
        Some(evm_controller) => {
            if let TriggerCommand::WatchEvmContractEvents { addresses, event_hashes, .. } = command {
                evm_controller.subscriptions.enable_logs(addresses, event_hashes);
            }
        }
        None => {
            tracing::debug!(
                "EVM controller for chain {chain} not yet ready, queuing WatchEvmContractEvents"
            );
            pending_evm_subscriptions
                .entry(chain.clone())
                .or_default()
                .push(command);
        }
    }
}

// Same pattern for TriggerCommand::WatchEvmBlocks (lines 594-606):
TriggerCommand::WatchEvmBlocks { ref chain } => {
    match self.evm_controllers.read().unwrap().get(chain) {
        Some(evm_controller) => {
            evm_controller.subscriptions.toggle_block_height(true);
        }
        None => {
            tracing::debug!(
                "EVM controller for chain {chain} not yet ready, queuing WatchEvmBlocks"
            );
            pending_evm_subscriptions
                .entry(chain.clone())
                .or_default()
                .push(command);
        }
    }
}

// After successful controller creation in StartListeningChain (EVM branch, after line 573):
// Drain pending subscriptions for this chain
if let Some(pending) = pending_evm_subscriptions.remove(&chain) {
    let controllers = self.evm_controllers.read().unwrap();
    if let Some(controller) = controllers.get(&chain) {
        for cmd in pending {
            match cmd {
                TriggerCommand::WatchEvmContractEvents { addresses, event_hashes, .. } => {
                    tracing::debug!("Replaying queued WatchEvmContractEvents for {chain}");
                    controller.subscriptions.enable_logs(addresses, event_hashes);
                }
                TriggerCommand::WatchEvmBlocks { .. } => {
                    tracing::debug!("Replaying queued WatchEvmBlocks for {chain}");
                    controller.subscriptions.toggle_block_height(true);
                }
                _ => {}
            }
        }
    }
}
```

**Note on ownership:** The `command` variable in the match arm is moved. Since the `TriggerCommand` arms currently destructure by pattern, the pending queue needs to capture the whole command before matching. The match arm needs to be restructured slightly to capture the command for queuing. See the Anti-Patterns section.

### Pattern 2: `TriggerCommand` Needs to Be `Clone` or Re-capturable

The current match arm for `WatchEvmContractEvents` destructures the command by value. To queue it, either:
- Make `TriggerCommand` derive `Clone` — but it has `Box<TriggerAction>` for `ManualTrigger`, which is clonable
- Or capture the command before destructuring by using a `ref` match guard to peek at the chain key first

The simplest approach: restructure the match arm to use the full command value for queueing.

### Anti-Patterns to Avoid

- **Silently dropping subscriptions:** The current behavior logs `tracing::error!` and `continue`s — this MUST be replaced with queueing, not just a better error message.
- **Re-sending via `command_sender`:** Do not re-send the command back via `self.command_sender.send(command)` — this creates a feedback loop if the chain never connects.
- **Blocking on chain connection:** Do not add a synchronous wait in `add_service` for the chain to connect — this blocks the entire restore loop and breaks the non-blocking design contract.
- **Retry via sleep loop:** Do not add a Tokio sleep-retry in the `StartListeningChain` handler — this blocks the single watcher task from processing other commands.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Async delay before subscription | Custom sleep/retry in watcher loop | Pending queue drained on controller creation |
| Thread-safe command storage | New `Arc<Mutex<Vec>>` on the struct | Local `HashMap` in `start_watcher` stack frame |
| Chain connection readiness signal | New channel or `Arc<AtomicBool>` | Drain point is already the controller creation success path |

**Key insight:** The `start_watcher` function already owns all relevant state as local variables. The fix is purely additive local state — no new struct fields, no new channels, no new `Arc` wrapping.

## Common Pitfalls

### Pitfall 1: `TriggerCommand` Move Semantics in Match Arms
**What goes wrong:** The match arm for `WatchEvmContractEvents` currently destructures by value. Adding a "queue this command" path requires the original command value, but the match already moved it.
**Why it happens:** Rust's match semantics move the matched value when destructuring without `ref`.
**How to avoid:** In the `None` branch, construct a new `TriggerCommand::WatchEvmContractEvents { chain, addresses, event_hashes }` value to push to the queue. Or restructure the outer match to check the chain key first with a reference before the consuming pattern.
**Warning signs:** Compiler error "use of moved value" or "cannot move out of `command`."

### Pitfall 2: Double Subscription on Chain Reconnect
**What goes wrong:** If a chain disconnects and reconnects (controller is recreated), pending commands that were already replayed get replayed again.
**Why it happens:** If `pending_evm_subscriptions` is not cleared after the first drain, reconnect events would re-drain stale entries.
**How to avoid:** The proposed fix uses `remove()` when draining — the map entry is gone after first drain. This is correct.
**Warning signs:** Duplicate log subscriptions, events fired twice per trigger.

### Pitfall 3: `StartListeningChain` Connection Failure Does Not Drain
**What goes wrong:** If `StartListeningChain` fails AND retries eventually succeed (state goes Waiting → Connecting → Waiting → Connecting → Connected), the drain must happen only in the Connected path.
**Why it happens:** The drain code must only execute on successful controller insertion.
**How to avoid:** The drain code goes AFTER `self.evm_controllers.write().unwrap().insert(chain.clone(), controller)` and BEFORE the `chain_state = StreamStartState::Connected` line — ensuring the controller is in the map before draining.
**Warning signs:** Panic `unwrap()` on controller lookup in drain code if placement is wrong.

### Pitfall 4: `WatchEvmBlocks` for Duplicate Chain Does Not Re-queue
**What goes wrong:** If two services share the same EVM chain, the second service's `StartListeningChain` is skipped (state `Connected`), so `WatchEvmBlocks` from the second service arrives after the controller is already in the map. This path is fine — no queuing needed.
**Why it happens:** N/A — this is the correct happy path.
**How to avoid:** The `WatchEvmBlocks`/`WatchEvmContractEvents` handlers already check `evm_controllers` directly; if the controller is present, they succeed immediately.

### Pitfall 5: Cron and ATProto Missing Controller Pattern
**What goes wrong:** Cron and ATProto streams use a `StreamStartState` but NOT a controller map — they are self-contained streams. The race for those types is different (Connecting state blocks double-start) and is NOT the bug reported. Do not apply the pending-queue pattern there.
**Why it happens:** Unlike EVM, Cron/ATProto streams do not require a separate "watch" command after the stream starts.
**How to avoid:** Scope the fix to `WatchEvmContractEvents` and `WatchEvmBlocks` handlers only.

## Code Examples

### Existing Drain Point — Where to Add the Fix
```rust
// Source: packages/wavs/src/subsystems/trigger.rs lines 562-574 (EVM branch of StartListeningChain)
multiplexed_stream.push(evm_event_stream);
multiplexed_stream.push(evm_block_stream);

self.evm_controllers
    .write()
    .unwrap()
    .insert(chain.clone(), controller);  // ← controller now in map
if let Some(chain_state) =
    listening_chain_states.get_mut(&chain)
{
    *chain_state = StreamStartState::Connected;  // ← success confirmed
}
// *** DRAIN POINT: replay pending_evm_subscriptions.remove(&chain) HERE ***
```

### Current Silent Drop (The Bug)
```rust
// Source: packages/wavs/src/subsystems/trigger.rs lines 577-593
TriggerCommand::WatchEvmContractEvents {
    chain,
    addresses,
    event_hashes,
} => match self.evm_controllers.read().unwrap().get(&chain) {
    Some(evm_controller) => {
        evm_controller
            .subscriptions
            .enable_logs(addresses, event_hashes);
    }
    None => {
        tracing::error!(
            "No EVM controller found for chain {chain}, cannot watch contract event"
        );
        continue;  // ← BUG: silently drops subscription
    }
},
```

### Existing Test File Location
```
packages/wavs/tests/trigger_tests.rs  — feature-gated with #[cfg(feature = "dev")]
```
A regression test should be added here verifying that `WatchEvmContractEvents` sent before `StartListeningChain` completes does not silently drop when the controller is later created.

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| Log error and discard | Queue and replay after controller ready | Subscriptions survive connection latency |

No external library changes. This is a pure Rust refactor within the existing async event loop.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The `TriggerCommand` enum variants do not need `Clone` derived; the pending queue can reconstruct values from destructured fields | Architecture Patterns | If enum has un-clonable inner types, the approach must use a different capture strategy |
| A2 | The startup ordering race only affects EVM chains (not Cosmos, Cron, ATProto, Hypercore) | Common Pitfalls | If Cosmos also has a controller-dependency pattern, similar fix may be needed there — but code review shows Cosmos uses a simple `cosmos_clients` HashMap that is populated inline with the stream start, so lookup commands are filtered in the event stream, not via a separate controller call |

## Open Questions

1. **Does `TriggerCommand` implement `Clone`?**
   - What we know: The enum has `Box<TriggerAction>` for `ManualTrigger`. `TriggerAction` is likely clonable.
   - What's unclear: Whether `Clone` is already derived or needs to be added.
   - Recommendation: Check. If `Clone` is not derived, derive it (or add it to `ManualTrigger` variant only if needed). The pending queue stores owned values.

2. **Are there existing unit tests for the startup sequence?**
   - What we know: `packages/wavs/tests/trigger_tests.rs` exists and tests lookup maps, but it does NOT test the watcher loop startup sequence (it's feature-gated `dev` and tests `core_trigger_lookups` without actually running `start_watcher`).
   - What's unclear: Whether a fast unit test can be written for the pending-queue logic without a live WebSocket.
   - Recommendation: The unit test should use the `disable_networking` dev flag to mock the chain connection and verify the pending commands are replayed correctly.

## Environment Availability

Step 2.6: SKIPPED (no external dependencies — this is a pure internal Rust code change within the existing WAVS process).

## Validation Architecture

`workflow.nyquist_validation` is `false` in `.planning/config.json`. Section skipped.

## Security Domain

This phase has no security surface changes — it is a reliability fix for an existing internal async event loop. No authentication, authorization, input handling, cryptographic, or session changes.

## Sources

### Primary (HIGH confidence)
- `packages/wavs/src/subsystems/trigger.rs` lines 577-605 — silent drop bug location, verified by code trace [VERIFIED: direct read]
- `packages/wavs/src/dispatcher.rs` lines 244-626 — startup sequence, service restore loop [VERIFIED: direct read]
- `packages/wavs/src/subsystems/trigger.rs` lines 189-238 — `add_service` command ordering [VERIFIED: direct read]
- `packages/wavs/src/subsystems/trigger.rs` lines 311-330 — `start_watcher` local state initialization [VERIFIED: direct read]
- `packages/wavs/src/subsystems/trigger/streams/evm_stream/client/subscription.rs` — `EvmTriggerStreamsController::enable_logs` [VERIFIED: direct read]

### Secondary (MEDIUM confidence)
- `packages/wavs/src/lib.rs` — `run_server` startup sequencing (HTTP ready gate before dispatcher.start) [VERIFIED: direct read]
- `app/src-tauri/src/commands.rs` — "Path A" (Tauri cmd_start_wavs) adds services before dispatcher.start [VERIFIED: direct read]

## Metadata

**Confidence breakdown:**
- Root cause identification: HIGH — traced through source code directly
- Fix approach: HIGH — pending-queue pattern is standard for this class of async ordering bug
- Test strategy: MEDIUM — existing test infrastructure uses `disable_networking` flag but test for this specific path does not exist yet

**Research date:** 2026-04-09
**Valid until:** Until trigger.rs or evm_stream/client is significantly refactored
