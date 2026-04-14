# Architecture Research

**Domain:** WAVS v1.3 — Activity UX & Bug Fixes
**Researched:** 2026-04-09
**Confidence:** HIGH (all findings from direct source inspection)

## Standard Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                       Tauri Backend (Rust)                       │
│                                                                   │
│  ┌──────────────┐  Crossbeam  ┌──────────┐  ┌──────────────┐    │
│  │   Trigger    │────────────▶│Dispatcher│──▶│    Engine    │    │
│  │   Manager    │             │  (loop)  │  │   Manager    │    │
│  └──────────────┘             │          │  └──────────────┘    │
│  ┌──────────────┐             │          │  ┌──────────────┐    │
│  │  Submission  │────────────▶│          │──▶│  Aggregator  │    │
│  │   Manager    │             │          │  └──────────────┘    │
│  └──────────────┘             └────┬─────┘                      │
│                                    │ tauri::Emitter              │
├────────────────────────────────────┼────────────────────────────┤
│                 IPC boundary       │                             │
├────────────────────────────────────┼────────────────────────────┤
│                   Tauri Frontend   ▼                             │
│  ┌──────────────────────────────────────────────┐               │
│  │              listeners.ts                     │               │
│  │  'trigger' | 'submission' | 'submission_failed'│              │
│  └──────────────────────┬───────────────────────┘               │
│                         │ store.addActivity()                    │
│  ┌──────────────────────▼───────────────────────┐               │
│  │   appStore (Zustand) — ActivityItem[]         │               │
│  └──────────────────────┬───────────────────────┘               │
│                         │ useGroupedActivity hook                │
│  ┌──────────────────────▼───────────────────────┐               │
│  │  GroupedActivityCard / ActivityFeed           │               │
│  └──────────────────────────────────────────────┘               │
└─────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Location |
|-----------|----------------|----------|
| Dispatcher | Routes DispatcherCommand variants to correct subsystem; emits Tauri events | `packages/wavs/src/dispatcher.rs` |
| Aggregator | Quorum accumulation, on-chain submission, yields `AnyTransactionReceipt` | `packages/wavs/src/subsystems/aggregator.rs` |
| Engine (SubmitCallback) | Post-submission callback component execution | `packages/wavs/src/subsystems/engine.rs` |
| TriggerManager | Manages `MultiplexedStream` of EVM/Cosmos/Cron streams; calls `add_service` via channel | `packages/wavs/src/subsystems/trigger.rs` |
| `wavs_gui_shared::event` | Shared event structs implementing `TauriEventExt`; the serialization contract | `packages/gui/shared/src/event.rs` |
| `listeners.ts` | Tauri event subscribers; maps raw payloads to `ActivityItem` and calls `store.addActivity()` | `app/src/tauri/listeners.ts` |
| `appStore` | Zustand store holding `ActivityItem[]`; `addActivity()` is the entry point for all feed items | `app/src/stores/appStore.ts` |
| `GroupedActivityCard` | Renders one trigger + optional child submission card; reads `group.submission` | `app/src/components/activity/GroupedActivityCard.tsx` |
| `WalletSection` | Shows addresses, balances, Export and Reset Wallet buttons inline | `app/src/components/settings/WalletSection.tsx` |

---

## Feature 1: tx_hash in SubmissionConfirmed

### Data Flow (Current)

```
aggregator.rs:632
  tx_resp.tx_hash()            // AnyTransactionReceipt -> String, logged but NOT forwarded
       |
       v
DispatcherCommand::SubmissionConfirmed {
  service_id, workflow_id, trigger_data, correlation_id    // tx_hash ABSENT
}
       |
       v
dispatcher.rs:462
  emit_ext(SubmissionEvent { ... })       // tx_hash ABSENT in struct
       |
       v
listeners.ts:60
  store.addActivity({ kind: 'submission', ... })    // no tx_hash field
       |
       v
ActivityItem { ... }                     // no tx_hash field
       |
       v
GroupedActivityCard                      // nothing to render
```

### Required Changes

**1. `DispatcherCommand::SubmissionConfirmed` — add `tx_hash: String`**

File: `packages/wavs/src/dispatcher.rs` (the enum definition, lines ~131-136)

```rust
SubmissionConfirmed {
    service_id: ServiceId,
    workflow_id: WorkflowId,
    trigger_data: TriggerData,
    correlation_id: String,
    tx_hash: String,          // NEW
},
```

**2. Aggregator send site — pass tx_hash**

File: `packages/wavs/src/subsystems/aggregator.rs` (the `Ok(tx_resp)` arm, lines ~634-647)

`tx_resp.tx_hash()` is already computed and logged on line 632. Pass it into the command at the same call site.

**3. `SubmissionEvent` GUI struct — add `tx_hash`**

File: `packages/gui/shared/src/event.rs` (lines ~56-65)

```rust
pub struct SubmissionEvent {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub trigger_data: TriggerData,
    pub correlation_id: String,
    pub tx_hash: String,       // NEW
}
```

**4. Dispatcher emit site — forward tx_hash**

File: `packages/wavs/src/dispatcher.rs` (the `SubmissionConfirmed` match arm, lines ~462-481)

Destructure `tx_hash` from the command variant and include it in `SubmissionEvent`.

**5. Frontend types — add `tx_hash` to `SubmissionEvent`**

File: `app/src/types/index.ts` (lines ~108-113)

```typescript
export interface SubmissionEvent {
  service_id: ServiceId;
  workflow_id: WorkflowId;
  trigger_data: TriggerData;
  correlation_id: string;
  tx_hash: string;           // NEW
}
```

**6. `ActivityItem` — add optional `txHash`**

File: `app/src/types/index.ts` (lines ~330-340)

```typescript
export interface ActivityItem {
  ...
  txHash?: string;           // NEW
}
```

**7. `listeners.ts` — map `tx_hash` to `txHash`**

File: `app/src/tauri/listeners.ts` (lines ~60-72)

Pass `txHash: payload.tx_hash` when constructing the submission `ActivityItem`.

**8. `GroupedActivityCard` — render tx_hash in child card**

File: `app/src/components/activity/GroupedActivityCard.tsx`

In the child submission card section (lines ~128-182), add a line or link showing `group.submission.txHash` when present. A shortened hex with copy-to-clipboard is appropriate UX.

### Modification Boundary Summary

| File | Change Type | What Changes |
|------|-------------|--------------|
| `dispatcher.rs` (enum) | Modify | Add `tx_hash: String` field to `SubmissionConfirmed` variant |
| `aggregator.rs` | Modify | Pass `tx_hash: tx_resp.tx_hash()` into the send call |
| `gui/shared/src/event.rs` | Modify | Add `tx_hash: String` field to `SubmissionEvent` |
| `dispatcher.rs` (match arm) | Modify | Destructure and forward `tx_hash` into `emit_ext` call |
| `app/src/types/index.ts` | Modify | Add `tx_hash` to `SubmissionEvent`, `txHash?` to `ActivityItem` |
| `app/src/tauri/listeners.ts` | Modify | Map `payload.tx_hash` to `txHash` in addActivity call |
| `app/src/components/activity/GroupedActivityCard.tsx` | Modify | Render `txHash` in submission child card |

---

## Feature 2: WasmResponse.payload in Activity Feed

### Current State

`Submission.operator_response: WasmResponse` is in scope at the aggregator call site. The payload (`Vec<u8>`) is the raw bytes returned by the WASM component. It is not currently forwarded to the GUI.

The `SubmissionEvent` only carries `trigger_data` (the incoming trigger bytes), not the execution result bytes.

### Where Payload Lives

At the aggregator success path (`packages/wavs/src/subsystems/aggregator.rs`), the `submission` variable (type `Submission`) is in scope. `submission.operator_response.payload` is the bytes.

The `SubmissionConfirmed` dispatch call already has access to `submission` at that point — `submission.trigger_action.data` and `submission.trigger_action.correlation_id` are already being pulled from it.

### Required Changes

**1. `DispatcherCommand::SubmissionConfirmed` — add `result_payload: Vec<u8>`**

At the aggregator send site, add `result_payload: submission.operator_response.payload.clone()`.

**2. `SubmissionEvent` GUI struct — add `result_payload: Vec<u8>`**

Serialize as hex (the existing pattern for `WasmResponse.payload` uses `#[serde(with = "const_hex")]`). Apply the same attribute so the frontend receives a hex string.

**3. `SubmissionEvent` frontend type — add `result_payload: string`**

The hex string arrives as-is. Frontend decodes for display.

**4. `ActivityItem` — add optional `resultPayload?: string`**

Store the hex string in the activity item.

**5. Smart decoding utility (new)**

Add a pure function (suggested location: `app/src/utils/decode.ts`) that implements:
```
decodePayload(hex: string): { mode: 'json' | 'utf8' | 'hex', display: string }
  1. Strip 0x prefix, decode hex to bytes
  2. Try UTF-8 decode
  3. If valid UTF-8, try JSON.parse
  4. If valid JSON, return { mode: 'json', display: JSON.stringify(parsed, null, 2) }
  5. If valid UTF-8 (not JSON), return { mode: 'utf8', display: utf8string }
  6. Otherwise, return { mode: 'hex', display: originalHex }
```

**6. `GroupedActivityCard` — render decoded result**

In the submission child card, show the decoded result using `decodePayload(group.submission.resultPayload)`. Label by mode (JSON / UTF-8 / Hex). Keep below tx_hash.

### Modification Boundary Summary

| File | Change Type | What Changes |
|------|-------------|--------------|
| `dispatcher.rs` (enum) | Modify | Add `result_payload: Vec<u8>` to `SubmissionConfirmed` |
| `aggregator.rs` | Modify | Pass `submission.operator_response.payload.clone()` |
| `gui/shared/src/event.rs` | Modify | Add `result_payload` with `const_hex` serde |
| `dispatcher.rs` (match arm) | Modify | Forward `result_payload` into `SubmissionEvent` |
| `app/src/types/index.ts` | Modify | Add `result_payload: string` to `SubmissionEvent`, `resultPayload?` to `ActivityItem` |
| `app/src/tauri/listeners.ts` | Modify | Map `payload.result_payload` to `resultPayload` |
| `app/src/utils/decode.ts` | New | `decodePayload` function |
| `app/src/components/activity/GroupedActivityCard.tsx` | Modify | Call `decodePayload`, render with mode label |

**Note on batching:** Features 1 and 2 both touch the same four Rust files and the same three frontend files. Implement them together to avoid editing `DispatcherCommand`, `SubmissionEvent`, `ActivityItem`, `listeners.ts`, and `GroupedActivityCard` twice.

---

## Feature 3: Service Restart Race Condition Fix

### Root Cause

`Dispatcher::start()` (dispatcher.rs lines ~241-315) spawns all subsystem threads simultaneously, including `trigger_manager.start()`. That call runs `start_watcher()`, which is the async loop that processes `TriggerCommand`s.

Service restore runs synchronously via `ctx.rt.block_on()` (dispatcher.rs lines ~518-620) and calls `add_service_to_managers()`, which calls `trigger_manager.add_service()`. That method sends `TriggerCommand::StartListeningChain` and `TriggerCommand::WatchEvmContractEvents` through the `command_sender` channel.

`WatchEvmContractEvents` in `start_watcher` does:
```rust
match self.evm_controllers.read().unwrap().get(&chain) {
    Some(evm_controller) => { evm_controller.subscriptions.enable_logs(...); }
    None => {
        tracing::error!("No EVM controller found for chain, cannot watch contract event");
        continue;       // silently drops the subscription
    }
}
```

The `evm_controllers` map is populated only when `StartListeningChain` is processed AND the EVM websocket connection succeeds. If `WatchEvmContractEvents` arrives before `StartListeningChain` completes (before the WebSocket connects and the controller is inserted), the subscription is silently dropped. On restart, the trigger stream never fires for that service.

### Why Ordering Cannot Be Guaranteed

`start()` spawns `trigger_manager.start(ctx)` in a separate OS thread. That thread calls `ctx.rt.block_on(self.start_watcher(...))`. There is no synchronization point between the `start_watcher` loop processing `TriggerCommand`s and the main thread calling `add_service_to_managers()`. In practice, `WatchEvmContractEvents` can arrive before `StartListeningChain` completes its async WebSocket connection.

### Fix Options

**Option A (recommended): Retry buffer in `start_watcher`**

When `WatchEvmContractEvents` arrives and no controller exists, push to a local `pending_watches: HashMap<ChainKey, Vec<(Vec<Address>, Vec<B256>)>>`. When `StartListeningChain` succeeds and inserts a controller, immediately apply all pending watches for that chain.

This is self-healing and handles both startup and runtime add-service scenarios without requiring synchronization.

**Option B: Signal readiness before restore**

Add a `oneshot` channel from `start_watcher` to the main thread, sent after the first loop iteration begins. Block `add_service_to_managers()` calls on receiving this signal. Does not eliminate the actual async gap (WebSocket connection is I/O-bound, not tick-bound).

**Option C: Two-phase startup**

Send all `StartListeningChain` commands, wait for all controllers to be ready, then send `WatchEvmContractEvents`. Requires a protocol change across `add_service` and the trigger loop.

**Recommended: Option A** — local to `start_watcher`, no cross-thread protocol changes, also fixes the runtime case.

### Modification Boundary

| File | Change Type | What Changes |
|------|-------------|--------------|
| `packages/wavs/src/subsystems/trigger.rs` | Modify | `start_watcher` function: add `pending_watches` map, apply on controller insertion |

No changes needed to `dispatcher.rs`, `add_service`, or any other subsystem.

---

## Feature 4: Wallet Kebab Menu

### Current State

`WalletSection` (lines ~202-303) renders two action buttons inline in the main content flow:
- "Export Recovery Phrase" (`Button variant="outline"`)
- "Reset Wallet" (`Button color="red" variant="outline"`) with an inline confirmation card

Both buttons are permanently visible, placing a dangerous action prominently next to a normal one.

### Target State

Collapse the uncommon actions into a kebab menu (`...`) in the wallet card header. The main section body shows only accounts and balances.

### Component Design

**New component: `KebabMenu`**

Location: `app/src/components/atoms/KebabMenu.tsx`

```
Props:
  items: Array<{
    label: string,
    onClick: () => void,
    variant?: 'default' | 'danger',
    disabled?: boolean,
  }>
```

Renders a three-dot button. On click, toggles an absolute-positioned dropdown. Closes on outside-click via `useEffect` + `document.addEventListener('mousedown', ...)`.

**Modified: `WalletSection`**

The header row (`<h2>Wallet</h2>`) becomes a flex container with the heading on the left and `<KebabMenu>` on the right. Items:
- "Export Recovery Phrase" triggers existing `handleExportWallet` logic
- "Reset Wallet" triggers existing `setShowResetConfirm(true)` logic

The mnemonic display and confirmation card stay in the body — they are feedback UI, not launchers.

### Component Tree Location

```
Settings.tsx
  └── WalletSection          (modified: adds KebabMenu to header)
        └── KebabMenu        (new atom component)
```

`KebabMenu` belongs in atoms because it is a generic UI primitive reusable across the app. `OwnerActionsMenu.tsx` in `components/poa/` already shows a similar dropdown pattern; `KebabMenu` can be a cleaner generalization for future reuse.

### Modification Boundary

| File | Change Type | What Changes |
|------|-------------|--------------|
| `app/src/components/atoms/KebabMenu.tsx` | New | Generic kebab dropdown atom |
| `app/src/components/atoms/index.ts` | Modify | Re-export `KebabMenu` |
| `app/src/components/settings/WalletSection.tsx` | Modify | Add `KebabMenu` to header, remove inline buttons from body |

---

## Architectural Patterns

### Pattern 1: Extend DispatcherCommand, Not New Events

**What:** Add data to existing event flows by extending the `DispatcherCommand` enum variant, `TauriEventExt` struct, frontend type, and listener in lockstep — never adding a parallel event for data that belongs to an existing lifecycle moment.

**When to use:** Any time new data is available at an existing event origin (e.g., tx_hash at SubmissionConfirmed, payload at the same point).

**Trade-offs:** All four layers must change together. This is correct — they form one serialization boundary.

### Pattern 2: Retry-Buffer for Async Command Ordering

**What:** When a command depends on async infrastructure that a prior command creates, buffer the dependent command locally and replay it when the dependency arrives — instead of introducing synchronization across thread boundaries.

**When to use:** Any command handler where a dependency may not be ready due to async I/O, without wanting to block the caller.

**Trade-offs:** Small amount of local state in `start_watcher`. Self-healing; handles both startup and runtime cases.

### Pattern 3: Atoms for Shared Interaction Primitives

**What:** Pure UI interaction components (KebabMenu, Button, AddressDisplay) live in `components/atoms/` and are composed into feature sections. Feature logic stays in the section component.

**When to use:** Any interaction widget with no domain logic that could plausibly be reused in two or more sections.

---

## Data Flow

### End-to-End: Trigger to Submission with tx_hash + payload (after v1.3)

```
1. On-chain event fires
       |
2. TriggerManager -> DispatcherCommand::Trigger(action)
       |
3. Dispatcher -> EngineCommand::ExecuteOperator
       |
4. Engine executes WASM -> WasmResponse { payload: Vec<u8> }
       |
5. EngineResponse::Operator(SubmissionRequest) -> Dispatcher
       |
6. Dispatcher -> SubmissionCommand -> SubmissionManager
       |
7. SubmissionManager -> DispatcherCommand::SubmissionResponse(Submission)
       |
8. Dispatcher -> AggregatorCommand::Execute(Submission)
       |
9. Aggregator accumulates quorum -> submits on-chain
       |
10. AnyTransactionReceipt -> tx_hash: String
    submission.operator_response.payload: Vec<u8>  <- both originate here
       |
11. DispatcherCommand::SubmissionConfirmed { tx_hash, result_payload, ... }
       |
12. Dispatcher emits SubmissionEvent { tx_hash, result_payload (hex), ... }
       |
13. listeners.ts -> store.addActivity({ txHash, resultPayload })
       |
14. GroupedActivityCard renders tx_hash + decodePayload(resultPayload)
```

### Service Restart Data Flow (after fix)

```
Dispatcher::start()
  |-- thread: trigger_manager.start() -> start_watcher() loop begins
  |     |-- processes TriggerCommand::StartListeningChain
  |     |     |-- (async) WebSocket connects -> evm_controllers.insert(chain, controller)
  |     |         |-- apply pending_watches[chain] immediately
  |     |-- processes TriggerCommand::WatchEvmContractEvents
  |           |-- controller ready -> enable_logs() SUCCESS
  |           |-- controller missing -> push to pending_watches[chain]
  |
  |-- (main) ctx.rt.block_on(restore services)
        |-- add_service_to_managers() sends StartListeningChain + WatchEvmContractEvents
              (ordering no longer matters due to pending_watches buffer)
```

---

## Integration Points

### Internal Boundaries

| Boundary | Communication | Notes for v1.3 |
|----------|---------------|----------------|
| Aggregator to Dispatcher | `crossbeam::channel::Sender<DispatcherCommand>` | Add `tx_hash` and `result_payload` to `SubmissionConfirmed` variant |
| Dispatcher to Frontend | `tauri::Emitter::emit()` via `TauriEventExt` | Add fields to `SubmissionEvent` struct |
| Frontend IPC to Store | `listen<SubmissionEvent>()` in `listeners.ts` | Map new fields to `ActivityItem` |
| `start_watcher` internal | Local `pending_watches` HashMap | New local state, no cross-thread boundary |
| `WalletSection` to Atoms | Component import | New `KebabMenu` atom |

### New vs Modified Summary

**New files:**
- `app/src/utils/decode.ts` — payload decoding utility (`decodePayload`)
- `app/src/components/atoms/KebabMenu.tsx` — kebab dropdown atom

**Modified backend files:**
- `packages/wavs/src/dispatcher.rs` — `DispatcherCommand` enum + match arm
- `packages/wavs/src/subsystems/aggregator.rs` — `SubmissionConfirmed` send site
- `packages/gui/shared/src/event.rs` — `SubmissionEvent` struct
- `packages/wavs/src/subsystems/trigger.rs` — `start_watcher` retry buffer

**Modified frontend files:**
- `app/src/types/index.ts` — `SubmissionEvent`, `ActivityItem`
- `app/src/tauri/listeners.ts` — submission listener mapping
- `app/src/components/activity/GroupedActivityCard.tsx` — render tx_hash + decoded result
- `app/src/components/settings/WalletSection.tsx` — kebab menu integration
- `app/src/components/atoms/index.ts` — re-export `KebabMenu`

---

## Recommended Build Order

Features 1 and 2 share all four Rust touch points and all three frontend touch points. Build them together.

1. **Backend (features 1+2 combined):** Extend `DispatcherCommand::SubmissionConfirmed` with `tx_hash` and `result_payload`, update aggregator send site, extend `SubmissionEvent`, update dispatcher match arm. One compile pass.

2. **Frontend (features 1+2 combined):** Extend `SubmissionEvent` and `ActivityItem` types, update `listeners.ts`, add `decode.ts`, update `GroupedActivityCard`.

3. **Trigger restart fix:** Edit `start_watcher` in `trigger.rs` to add `pending_watches` buffer. Fully isolated — no dependency on features 1/2.

4. **Wallet kebab menu:** New `KebabMenu` atom, modify `WalletSection`. Pure frontend, no backend changes, no dependency on any other feature.

---

## Anti-Patterns

### Anti-Pattern 1: Separate Tauri Event for tx_hash

**What people do:** Emit a new `"submission_hash"` event alongside `"submission"` rather than extending the existing struct.

**Why it's wrong:** The frontend must correlate two events by `correlation_id` to display a single card row. The existing grouping already handles this; a third event type adds complexity.

**Do this instead:** Add `tx_hash` directly to `SubmissionEvent`. One event, one card update.

### Anti-Pattern 2: Decode payload in the Rust backend

**What people do:** Convert `Vec<u8>` to a string (JSON, UTF-8, hex) in Rust before sending to the GUI.

**Why it's wrong:** The decoding choice (JSON vs UTF-8 vs hex) belongs with the display. Raw bytes in hex carry all information. Decoding in Rust forces an irreversible choice at the wrong layer.

**Do this instead:** Serialize as hex string (consistent with existing `const_hex` pattern on `WasmResponse.payload`). Decode in `app/src/utils/decode.ts` at render time.

### Anti-Pattern 3: Synchronization barrier at start_watcher entry

**What people do:** Add a `tokio::sync::Barrier` or `oneshot` channel so `add_service_to_managers()` waits for `start_watcher` to be "ready" before sending commands.

**Why it's wrong:** "Ready" only means the loop started, not that `StartListeningChain` has completed its async WebSocket connection. The race remains because stream connection is I/O-bound.

**Do this instead:** Buffer `WatchEvmContractEvents` in `start_watcher` and apply when the controller is inserted. Fix is at the point of failure, not at a barrier that does not eliminate the actual async gap.

---

## Sources

All findings from direct source inspection (HIGH confidence):

- `packages/wavs/src/dispatcher.rs` — `DispatcherCommand` enum, `start()`, `SubmissionConfirmed` match arm
- `packages/wavs/src/subsystems/aggregator.rs` — lines 628-650, 680-695 (tx_hash origin, SubmissionConfirmed send)
- `packages/wavs/src/subsystems/aggregator/submit.rs` — `AnyTransactionReceipt::tx_hash()`
- `packages/wavs/src/subsystems/engine.rs` — `AggregatorExecuteKind`, `EngineResponse`
- `packages/wavs/src/subsystems/trigger.rs` — `add_service()`, `start_watcher()`, `WatchEvmContractEvents` handling
- `packages/gui/shared/src/event.rs` — `SubmissionEvent`, `TauriEventExt` trait
- `packages/types/src/submission.rs` — `Submission` struct with `operator_response: WasmResponse`
- `packages/types/src/service.rs` — `WasmResponse` struct with `payload: Vec<u8>`
- `app/src/types/index.ts` — `ActivityItem`, `SubmissionEvent`, `TriggerData`
- `app/src/tauri/listeners.ts` — all event listener mappings
- `app/src/components/activity/GroupedActivityCard.tsx` — submission child card structure
- `app/src/components/settings/WalletSection.tsx` — current button layout

---
*Architecture research for: WAVS v1.3 Activity UX & Bug Fixes*
*Researched: 2026-04-09*
