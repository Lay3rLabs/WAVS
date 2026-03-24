# Phase 12: Unified Activity Events - Research

**Researched:** 2026-03-24
**Domain:** Frontend event merging, Rust backend event pipeline, Tauri IPC events
**Confidence:** HIGH

## Summary

Phase 12 transforms the activity feed from showing separate trigger and submission entries into unified event cards that track the full lifecycle of each workflow execution. The current system emits two independent Tauri events (`trigger` and `submission`) which the frontend stores as flat `ActivityItem` entries with a `kind` discriminator. The core challenge is correlating these events on the frontend and extending the backend to emit additional event states (errors, tx_hash) that are currently only logged.

The backend currently only emits `SubmissionConfirmed` (success path) to the GUI. **Submission errors are logged but never forwarded as events to the frontend.** The `tx_hash` is available at the point where `SubmissionConfirmed` is dispatched but is not included in the event payload. Both gaps require Rust backend changes in `packages/gui/shared/src/event.rs` and `packages/wavs/src/subsystems/aggregator.rs`.

**Primary recommendation:** Add two new Rust event variants (`SubmissionErrorEvent`, enhanced `SubmissionEvent` with `tx_hash`), build a `Map<correlationKey, UnifiedActivity>` in the Zustand store keyed by `serviceId + workflowId + triggerData hash`, and replace the `ActivityCard` component with a status-progression card that updates in place as events arrive.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ACT-01 | Trigger and submission events merged into unified event cards (trigger event with submission result inlined) | Correlation key from `serviceId + workflowId + triggerData` identity; frontend `Map<key, UnifiedActivity>` store pattern; backend already provides matching fields on both events |
| ACT-02 | Event status progression displayed (pending -> submitted -> confirmed/error) | Requires new `SubmissionErrorEvent` from backend (currently errors only logged); `tx_hash` must be added to `SubmissionEvent` payload; frontend status enum drives visual indicator |
| ACT-03 | Submission errors displayed inline on event cards | Requires `SubmissionErrorEvent` with `error_message` field emitted from `aggregator.rs` error path; frontend renders error state on the unified card |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19.1.0 | UI framework | Already in use |
| Zustand | 5.0.0 | State management | Already in use, stores the activity list |
| @tanstack/react-virtual | 3.13.18 | Virtualized list | Already powers ActivityFeed |
| clsx | 2.1.0 | Conditional classnames | Already in use throughout |
| Tailwind CSS | 3.4.0 | Styling | Already in use with custom theme |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| @tauri-apps/api | 2.10.1 | Tauri event listener | Already used for trigger/submission listeners |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Zustand Map for correlation | RxJS/observable merge | Overkill -- Zustand store mutation is simpler and already the pattern |
| Custom hash for correlation key | UUID from backend | Would require backend to generate and attach IDs to both trigger and submission events; correlation key from existing fields is sufficient |

**Installation:** No new packages needed. All dependencies already present.

## Architecture Patterns

### Current Event Flow (BEFORE)
```
[Rust Aggregator] --SubmissionConfirmed--> [Dispatcher] --emit_ext(SubmissionEvent)--> [Tauri Frontend]
[Rust TriggerMgr] --Trigger--> [Dispatcher] --emit_ext(TriggerEvent)--> [Tauri Frontend]
```

Both arrive as independent `ActivityItem` entries in `appStore.activityList` (flat array, no correlation).

### Target Event Flow (AFTER)
```
[Rust Aggregator] --SubmissionConfirmed{tx_hash}--> [Dispatcher] --emit_ext(SubmissionEvent{tx_hash})-->
[Rust Aggregator] --SubmissionError{error_msg}-->   [Dispatcher] --emit_ext(SubmissionErrorEvent)-->

Frontend: TriggerEvent -> create UnifiedActivity(status: pending)
          SubmissionEvent -> update UnifiedActivity(status: confirmed, tx_hash)
          SubmissionErrorEvent -> update UnifiedActivity(status: error, error_message)
```

### Correlation Key Strategy

The key insight: `TriggerEvent` and `SubmissionEvent` both carry `service_id`, `workflow_id`, and `trigger_data`. The `trigger_data` contains unique identifiers per event (block number, tx_hash, log_index for EVM events; trigger_time for cron; etc.) that make the combination globally unique for each workflow execution.

**Correlation key:** `${serviceId}:${workflowId}:${stableHash(triggerData)}`

Where `stableHash` is a deterministic JSON stringification of `triggerData` (since Rust serde produces consistent key ordering). A simple approach: `JSON.stringify(triggerData)` since both events carry identical `TriggerData` from the same source.

### Recommended Data Model

```typescript
// New status enum
type ActivityStatus = 'pending' | 'confirmed' | 'error';

// New unified activity item (replaces ActivityItem)
interface UnifiedActivity {
  id: number;                      // unique id for React keys
  correlationKey: string;          // serviceId:workflowId:hash(triggerData)
  triggerTs: number;               // when trigger arrived
  submissionTs: number | null;     // when submission confirmed/errored
  status: ActivityStatus;          // lifecycle state
  serviceId: ServiceId;
  workflowId: WorkflowId;
  triggerData: TriggerData;
  triggerConfig?: TriggerConfig;
  txHash: string | null;           // from enhanced SubmissionEvent
  errorMessage: string | null;     // from new SubmissionErrorEvent
}
```

### Store Changes Pattern

```typescript
// appStore.ts changes
interface AppState {
  activityMap: Map<string, UnifiedActivity>;  // keyed by correlationKey
  activityList: UnifiedActivity[];            // derived sorted array for rendering

  // Actions
  handleTrigger: (event: TriggerEvent) => void;
  handleSubmission: (event: EnhancedSubmissionEvent) => void;
  handleSubmissionError: (event: SubmissionErrorEvent) => void;
}
```

On trigger event: create new `UnifiedActivity` with `status: 'pending'`, insert into map.
On submission event: look up by correlation key, update `status: 'confirmed'`, set `txHash`.
On error event: look up by correlation key, update `status: 'error'`, set `errorMessage`.

The `activityList` is derived from the map values, sorted by `triggerTs`. This maintains the existing virtualized list contract.

### Backend Changes (Rust)

#### 1. Add `tx_hash` to `SubmissionEvent`

In `packages/gui/shared/src/event.rs`:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubmissionEvent {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub trigger_data: TriggerData,
    pub tx_hash: Option<String>,  // NEW: hex-encoded tx hash
}
```

In `packages/wavs/src/subsystems/aggregator.rs` (success path ~line 643):
```rust
.send(DispatcherCommand::SubmissionConfirmed {
    service_id: submission.service_id().clone(),
    workflow_id: submission.workflow_id().clone(),
    trigger_data: submission.trigger_action.data.clone(),
    tx_hash: Some(tx_resp.tx_hash().to_string()),  // NEW
})
```

#### 2. Add `SubmissionErrorEvent`

In `packages/gui/shared/src/event.rs`:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubmissionErrorEvent {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub trigger_data: TriggerData,
    pub error_message: String,
}

impl TauriEventExt for SubmissionErrorEvent {
    const NAME: &'static str = "submission_error";
}
```

In the aggregator error path (~line 653), emit the error event:
```rust
Err(err) => {
    let err_msg = format!("{:?}", err);
    // ... existing logging ...

    // NEW: Emit error event to GUI
    if let Err(e) = self.subsystem_to_dispatcher_tx
        .send(DispatcherCommand::SubmissionError {
            service_id: submission.service_id().clone(),
            workflow_id: submission.workflow_id().clone(),
            trigger_data: submission.trigger_action.data.clone(),
            error_message: err_msg.clone(),
        })
    {
        tracing::error!("Error sending SubmissionError to dispatcher: {:?}", e);
    }
    // ... existing queue save ...
}
```

### Recommended Project Structure
```
app/src/
  components/
    activity/
      ActivityFeed.tsx        # Updated: uses UnifiedActivity[], removes kind filter
      ActivityCard.tsx        # Rewritten: unified card with status indicator
      StatusBadge.tsx         # NEW: pending/confirmed/error badge component
  stores/
    appStore.ts               # Updated: Map-based correlation, new handlers
  tauri/
    listeners.ts              # Updated: new submission_error listener, enhanced submission
  types/
    index.ts                  # Updated: UnifiedActivity type, new event types
packages/
  gui/shared/src/
    event.rs                  # Updated: SubmissionEvent.tx_hash, new SubmissionErrorEvent
  wavs/src/
    dispatcher.rs             # Updated: new DispatcherCommand::SubmissionError variant
    subsystems/aggregator.rs  # Updated: emit error events to dispatcher
```

### Anti-Patterns to Avoid
- **Polling for submission status:** The event-driven architecture already sends events on state changes. Do not add polling.
- **Storing two arrays and merging on render:** Would cause O(n*m) merge on every render. Use a Map keyed by correlation key instead.
- **Deep comparison of triggerData for correlation:** `JSON.stringify` is sufficient since both events carry the same Rust-serialized `TriggerData`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Virtualized list | Custom windowing | @tanstack/react-virtual | Already in use, handles dynamic heights |
| State management | Context + reducer | Zustand | Already the project pattern, simpler API |
| Event correlation ID | UUID generation system | Deterministic key from existing fields | Both events already carry identical service_id + workflow_id + trigger_data |

**Key insight:** The correlation problem looks like it needs UUIDs, but both trigger and submission events already carry the same `(serviceId, workflowId, triggerData)` triple from the Rust backend. A deterministic key from these fields eliminates the need for ID generation.

## Common Pitfalls

### Pitfall 1: TriggerData Serialization Mismatch
**What goes wrong:** `JSON.stringify(triggerData)` produces different strings for the trigger event vs the submission event because JavaScript object key ordering is not guaranteed.
**Why it happens:** Different code paths in Rust serialize TriggerData slightly differently, or JavaScript runtime reorders keys.
**How to avoid:** Use a canonical serialization function that sorts keys before stringifying, OR use a subset of trigger data fields that are primitive (e.g., for EvmContractEvent: `${chain}:${block_number}:${log_index}`).
**Warning signs:** Submission events creating new cards instead of updating existing pending cards.

### Pitfall 2: Orphaned Submission Events
**What goes wrong:** A submission event arrives but has no matching trigger event in the map (e.g., app was restarted between trigger and submission).
**Why it happens:** The activity store is in-memory only, cleared on restart. Triggers may have been processed before the app connected.
**How to avoid:** When a submission/error event arrives with no matching trigger, create a new UnifiedActivity directly with `status: 'confirmed'` or `status: 'error'` (backfill the trigger data from the submission event payload).
**Warning signs:** Submission events being silently dropped.

### Pitfall 3: DispatcherCommand Enum Size
**What goes wrong:** Adding a new `SubmissionError` variant to `DispatcherCommand` could increase enum size if not careful with field types.
**Why it happens:** Rust enums are sized to their largest variant.
**How to avoid:** The `error_message: String` is heap-allocated, so only a pointer/length/capacity (24 bytes) is stored in the enum. The existing `SubmissionConfirmed` already has `TriggerData` which is likely larger. No concern here.
**Warning signs:** None expected, but `#[allow(clippy::large_enum_variant)]` is already on the enum.

### Pitfall 4: Virtualizer Height Recalculation
**What goes wrong:** When a pending card gets updated to confirmed/error (adding tx_hash or error message), its height changes but the virtualizer doesn't know.
**Why it happens:** @tanstack/react-virtual caches measured heights and doesn't re-measure on content changes.
**How to avoid:** The existing pattern uses `virtualizer.measureElement` via `ref`. The virtualizer re-measures when elements are recycled. For status updates on visible cards, force a re-measure by changing the key or calling `virtualizer.measure()`.
**Warning signs:** Overlapping or clipped cards after status updates.

### Pitfall 5: Kind Filter Removal
**What goes wrong:** The current ActivityFeed has a "Triggers | Submissions" kind filter. With unified cards, this filter becomes meaningless but removing it changes the UI contract.
**Why it happens:** Unified cards represent both trigger and submission. The "kind" concept goes away.
**How to avoid:** Replace the kind filter with a status filter: "All | Pending | Confirmed | Error". This provides equivalent filtering on the new data model.
**Warning signs:** Users looking for a way to see only errors or only pending items.

## Code Examples

### Correlation Key Generation
```typescript
// Deterministic correlation key from event fields
function correlationKey(serviceId: string, workflowId: string, triggerData: TriggerData): string {
  // Use specific identifying fields from trigger data for a stable key
  // rather than full JSON.stringify which may have ordering issues
  if ('EvmContractEvent' in triggerData) {
    const d = triggerData.EvmContractEvent;
    return `${serviceId}:${workflowId}:evm:${d.chain}:${d.block_number}:${d.log_index}`;
  }
  if ('CosmosContractEvent' in triggerData) {
    const d = triggerData.CosmosContractEvent;
    return `${serviceId}:${workflowId}:cosmos:${d.chain}:${d.block_height}:${d.event_index}`;
  }
  if ('BlockInterval' in triggerData) {
    const d = triggerData.BlockInterval;
    return `${serviceId}:${workflowId}:block:${d.chain}:${d.block_height}`;
  }
  if ('Cron' in triggerData) {
    return `${serviceId}:${workflowId}:cron:${triggerData.Cron.trigger_time}`;
  }
  if ('AtProtoEvent' in triggerData) {
    const d = triggerData.AtProtoEvent;
    return `${serviceId}:${workflowId}:atproto:${d.sequence}`;
  }
  if ('HypercoreAppend' in triggerData) {
    const d = triggerData.HypercoreAppend;
    return `${serviceId}:${workflowId}:hypercore:${d.feed_key}:${d.index}`;
  }
  if ('Raw' in triggerData) {
    return `${serviceId}:${workflowId}:raw:${triggerData.Raw.length}:${Date.now()}`;
  }
  return `${serviceId}:${workflowId}:unknown:${Date.now()}`;
}
```

### Zustand Store Update for Trigger Event
```typescript
handleTrigger: (event: TriggerEvent) => {
  const action = event.action;
  const key = correlationKey(action.config.service_id, action.config.workflow_id, action.data);

  set((state) => {
    const newMap = new Map(state.activityMap);
    newMap.set(key, {
      id: nextActivityId(),
      correlationKey: key,
      triggerTs: Date.now(),
      submissionTs: null,
      status: 'pending',
      serviceId: action.config.service_id,
      workflowId: action.config.workflow_id,
      triggerData: action.data,
      triggerConfig: action.config,
      txHash: null,
      errorMessage: null,
    });

    // Derive sorted list and enforce max size
    const list = Array.from(newMap.values()).sort((a, b) => a.triggerTs - b.triggerTs);
    if (list.length > MAX_ACTIVITY_ITEMS) {
      const toRemove = list.slice(0, list.length - MAX_ACTIVITY_ITEMS);
      toRemove.forEach((item) => newMap.delete(item.correlationKey));
    }

    return { activityMap: newMap, activityList: Array.from(newMap.values()) };
  });
},
```

### StatusBadge Component Pattern
```typescript
// Source: Project design system (tailwind.config.js custom colors)
function StatusBadge({ status }: { status: ActivityStatus }) {
  const styles = {
    pending: 'bg-amber-900/40 text-amber-400',
    confirmed: 'bg-green-900/40 text-green-400',
    error: 'bg-red-900/40 text-red-400',
  };
  const labels = {
    pending: 'Pending',
    confirmed: 'Confirmed',
    error: 'Error',
  };
  return (
    <span className={clsx('px-1.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wide', styles[status])}>
      {labels[status]}
    </span>
  );
}
```

### Enhanced SubmissionEvent Rust Type
```rust
// Source: packages/gui/shared/src/event.rs
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubmissionEvent {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub trigger_data: TriggerData,
    pub tx_hash: Option<String>,  // hex-encoded, present on success
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate trigger/submission lists | Unified activity feed (Phase 9) | 2026-03-24 | ActivityFeed already merged into one list but with `kind` discriminator |
| No error forwarding to GUI | Log-only errors | Current | Errors visible only in Logs page, not Activity |

**Deprecated/outdated:**
- The `kind: ActivityKind` field on `ActivityItem` (either `'trigger'` or `'submission'`) becomes unnecessary with unified cards
- The `kindFilter` state in `ActivityFeed` should be replaced with a `statusFilter`

## Open Questions

1. **SubmissionEvent `tx_hash` format**
   - What we know: `tx_resp.tx_hash()` returns a type that implements `Display`. For EVM it's `FixedBytes<32>`, for Cosmos it's a `String`.
   - What's unclear: Whether `to_string()` on the EVM tx hash includes the `0x` prefix or not.
   - Recommendation: Use `format!("0x{}", const_hex::encode(hash))` for EVM hashes to ensure consistent `0x`-prefixed hex. For Cosmos, pass through as-is. The frontend should handle both.

2. **Services with `submit: 'none'`**
   - What we know: Some services have `submit: 'none'` (no aggregator submission). Their trigger events will never get a matching submission.
   - What's unclear: Should these show as permanently "pending" or should they have a different status?
   - Recommendation: For `submit: 'none'` services, set status to `'confirmed'` immediately on trigger (no submission expected). Check `service.workflows[workflowId].submit` when creating the UnifiedActivity.

3. **Max map size cleanup**
   - What we know: Current implementation caps at `MAX_ACTIVITY_ITEMS = 2000`.
   - What's unclear: Whether evicting by oldest `triggerTs` could remove pending items that later get a submission.
   - Recommendation: Keep the 2000 cap. If a submission arrives for an evicted item, create a new entry with backfilled data (same as orphaned submission handling).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | None (no test framework configured for frontend app) |
| Config file | none -- see Wave 0 |
| Quick run command | N/A |
| Full suite command | N/A |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ACT-01 | Trigger+submission merged into one card | manual-only | Visual inspection in running app | N/A |
| ACT-02 | Status progression (pending->confirmed/error) | manual-only | Visual inspection with live WAVS node | N/A |
| ACT-03 | Error messages displayed inline | manual-only | Trigger a submission error with `WAVS_FORCE_SUBMISSION_ERROR_XXX` env var | N/A |

### Sampling Rate
- **Per task commit:** `cd app && pnpm build` (TypeScript compilation check)
- **Per wave merge:** `cd app && pnpm build` + visual inspection
- **Phase gate:** Full build green + manual verification with live node

### Wave 0 Gaps
None -- no test framework exists and adding one is out of scope for this phase. The requirement IDs are best verified through manual inspection (UI behavior with live events). TypeScript compilation (`pnpm build`) provides type-safety verification.

## Sources

### Primary (HIGH confidence)
- `app/src/types/index.ts` -- ActivityItem, TriggerEvent, SubmissionEvent types (lines 88-330)
- `app/src/stores/appStore.ts` -- Current activity store pattern (lines 1-91)
- `app/src/tauri/listeners.ts` -- Current event listener setup (lines 1-87)
- `app/src/components/activity/ActivityCard.tsx` -- Current card rendering (lines 1-241)
- `app/src/components/activity/ActivityFeed.tsx` -- Current feed with virtualizer (lines 1-323)
- `packages/gui/shared/src/event.rs` -- Rust event definitions (lines 1-83)
- `packages/wavs/src/dispatcher.rs` -- DispatcherCommand enum, event emission (lines 116-470)
- `packages/wavs/src/subsystems/aggregator.rs` -- Submission success/error paths (lines 620-700)

### Secondary (MEDIUM confidence)
- `packages/wavs/src/subsystems/submission.rs` -- Submission signing pipeline (context for error types)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies
- Architecture: HIGH -- complete trace of event flow from Rust backend through Tauri IPC to React frontend
- Pitfalls: HIGH -- identified from direct code analysis of both frontend and backend
- Backend changes: HIGH -- exact files, line numbers, and required modifications identified

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable -- internal app, no external API changes)
