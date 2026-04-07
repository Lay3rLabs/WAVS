# Phase 4: Rust Event Foundation - Research

**Researched:** 2026-04-07
**Domain:** Rust event types, Tauri event emission, Crossbeam channel pipelines, TypeScript type mirroring
**Confidence:** HIGH

## Summary

Phase 4 adds a `correlation_id` field to both `TriggerEvent` and `SubmissionEvent` so the GUI can link a trigger to its eventual submission, and adds a new `SubmissionFailed` event for failures that are currently silently dropped with only a `tracing::error!` log.

The data flow is: trigger fires → `TriggerAction` created in `trigger.rs` → `DispatcherCommand::Trigger` → dispatcher emits `TriggerEvent` to GUI and sends `EngineCommand::ExecuteOperator` → engine returns `SubmissionRequest` → submission signs and dispatches → aggregator emits `DispatcherCommand::SubmissionConfirmed` → dispatcher emits `SubmissionEvent` to GUI. The correlation ID must be born on `TriggerAction` (the earliest common point) and carried forward through the whole pipeline to `SubmissionConfirmed`.

Two classes of failures currently drop silently: (1) signing errors in `SubmissionManager::start` at the `sign_request` call site, and (2) dispatch errors at the `dispatch` call site. Both are in `packages/wavs/src/subsystems/submission.rs` and both have the `TriggerAction` available in `req.trigger_action`. Adding a `DispatcherCommand::SubmissionFailed` variant and a corresponding `SubmissionFailedEvent` in `wavs-gui-shared` closes both gaps.

**Primary recommendation:** Add `correlation_id: String` to `TriggerAction` in `wavs-types`, carry it unchanged through all pipeline structs, add `SubmissionFailed` event/command pair, and mirror both changes in the TypeScript `types/index.ts` file. Three files change in Rust core; two files change on the TypeScript side.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all implementation choices are at Claude's discretion.

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

### Deferred Ideas (OUT OF SCOPE)
None — infrastructure phase.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EVT-01 | Every `TriggerEvent` and `SubmissionEvent` reaching the desktop app includes a `correlation_id` that uniquely identifies the trigger execution and links a trigger to its submission | `TriggerAction` is the root struct; adding `correlation_id: String` there propagates it through `SubmissionRequest`, `SubmissionCommand`, `DispatcherCommand::SubmissionConfirmed`, `TriggerEvent`, and `SubmissionEvent` with no intermediate storage needed |
| ERR-01 | When a submission fails (signing error or dispatch error), a failure event reaches the GUI with an error message string, rather than a silent `tracing::error!` log drop | Two silent drop sites identified in `submission.rs` lines 110-116 and 125-131; both have `req.trigger_action.correlation_id` available; adding `DispatcherCommand::SubmissionFailed` + `SubmissionFailedEvent` closes both |
</phase_requirements>

## Standard Stack

### Core — Already in codebase

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `uuid` (workspace) | 1.18.1 | Generate correlation IDs | Already in workspace `Cargo.toml` with `v7` and `serde` features; used in `packages/cli` and `packages/layer-tests` |
| `crossbeam` (workspace) | — | Channel message passing | Already used for all subsystem→dispatcher communication |
| `serde` (workspace) | — | Serialization for Tauri events | All event structs derive `Serialize + Deserialize` |
| `tauri` (workspace) | — | Event emission to frontend | Used via `TauriHandle::emit_ext` in dispatcher |

`uuid` is in the workspace but NOT yet a declared dependency of `wavs-types` or `wavs-gui-shared`. It must be added to `wavs-types/Cargo.toml` when `correlation_id` is placed on `TriggerAction`.

**Installation — uuid addition:**
```toml
# In packages/types/Cargo.toml [dependencies]
uuid = { workspace = true }
```

**Version verification:** `uuid` workspace entry confirmed as 1.18.1 with `v7,serde` features [VERIFIED: /workspace/Cargo.toml line 223].

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `uuid::Uuid::now_v7()` as String | nanoid or random bytes | uuid v7 is time-ordered and already in workspace; consistent with how `WorkflowId` is generated in `packages/cli` (line 622 of `service.rs`) |
| String field on `TriggerAction` | Separate `CorrelationId` newtype | String is simpler; newtype would require more derive boilerplate. Given the lightweight nature of this phase, String is sufficient |
| New field on `TriggerConfig` | New field directly on `TriggerAction` | Either works; `TriggerAction` is the outermost struct and the natural "envelope" — placing it there keeps it adjacent to `data`, matching the pattern established by `config` and `data` fields |

## Architecture Patterns

### Data Flow — Where Correlation ID Lives and Travels

```
trigger.rs (many call sites)
  └─ TriggerAction { config, data, correlation_id }   ← BORN HERE
       │
       ├─ DispatcherCommand::Trigger(action)
       │   └─ dispatcher emits TriggerEvent { action }  → GUI (EVT-01 satisfied)
       │
       └─ EngineCommand::ExecuteOperator { action, service }
           └─ EngineResponse::Operator(SubmissionRequest { trigger_action, ... })
               └─ SubmissionCommand::Submit(req)
                   └─ req.trigger_action.correlation_id available at all error sites
                       │
                       ├─ ON SUCCESS: DispatcherCommand::SubmissionConfirmed { ..., correlation_id }
                       │   └─ dispatcher emits SubmissionEvent { ..., correlation_id }  → GUI (EVT-01 satisfied)
                       │
                       └─ ON FAILURE: DispatcherCommand::SubmissionFailed { correlation_id, error }
                           └─ dispatcher emits SubmissionFailedEvent { ... }  → GUI (ERR-01 satisfied)
```

### Pattern 1: Add `correlation_id` to `TriggerAction`

**What:** New `String` field on `TriggerAction` in `wavs-types/src/service.rs`. Must add `uuid` as dependency of `wavs-types`.

**When to use:** For every field that must be born at trigger-reception time and survive through the pipeline unchanged.

**Example (pattern from existing code):**
```rust
// Source: packages/types/src/service.rs (existing struct, new field added)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, bincode::Decode, bincode::Encode, ToSchema)]
pub struct TriggerAction {
    #[bincode(with_serde)]
    pub config: TriggerConfig,
    #[bincode(with_serde)]
    pub data: TriggerData,
    /// Unique ID linking this trigger to its eventual submission event
    pub correlation_id: String,
}
```

**Impact:** Every `TriggerAction { ... }` construction site must supply `correlation_id`. There are ~15 sites (trigger.rs, debug.rs, mock_trigger_manager.rs, engine_setup.rs). Most are in test/bench code.

### Pattern 2: Extend `DispatcherCommand` with `SubmissionFailed`

**What:** New variant on the `DispatcherCommand` enum in `dispatcher.rs`. The submission subsystem sends this instead of silently returning.

**Example:**
```rust
// Source: packages/wavs/src/dispatcher.rs (enum extension)
pub enum DispatcherCommand {
    // ... existing variants ...
    SubmissionFailed {
        service_id: ServiceId,
        workflow_id: WorkflowId,
        correlation_id: String,
        error: String,
    },
}
```

### Pattern 3: New `SubmissionFailedEvent` in `wavs-gui-shared`

**What:** New event struct in `packages/gui/shared/src/event.rs` following the exact pattern of existing event structs.

**Example (following existing pattern):**
```rust
// Source: packages/gui/shared/src/event.rs (pattern from SubmissionEvent)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubmissionFailedEvent {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub correlation_id: String,
    pub error: String,
}

impl TauriEventExt for SubmissionFailedEvent {
    const NAME: &'static str = "submission_failed";
}
```

### Pattern 4: Update `SubmissionConfirmed` to carry `correlation_id`

**What:** Add `correlation_id: String` to the `DispatcherCommand::SubmissionConfirmed` variant and to `SubmissionEvent` in `wavs-gui-shared`. The aggregator already has access to `submission.trigger_action.correlation_id`.

### Pattern 5: TypeScript type mirroring

**What:** The frontend does NOT auto-generate types from Rust. All types are manually maintained in `app/src/types/index.ts`. Both `TriggerAction` and `SubmissionEvent` interfaces must be updated, and a new `SubmissionFailedEvent` interface added. The listener in `app/src/tauri/listeners.ts` must register for `submission_failed` events.

**Existing pattern:**
```typescript
// Source: app/src/tauri/listeners.ts — existing trigger listener pattern
const unlistenTrigger = await listen<TriggerEvent>(EVENTS.TRIGGER, (event) => {
  const action = event.payload.action;
  store.addActivity({
    id: nextActivityId(),
    ts: Date.now(),
    kind: 'trigger',
    serviceId: action.config.service_id,
    workflowId: action.config.workflow_id,
    triggerData: action.data,
    triggerConfig: action.config,
    correlationId: action.correlation_id,  // NEW
  });
});
```

### Anti-Patterns to Avoid

- **Generating the correlation ID in the dispatcher:** The dispatcher receives `TriggerAction` from multiple trigger sources. The ID must be generated at the point of `TriggerAction` construction in `trigger.rs` so it survives P2P broadcast if a trigger is distributed.
- **Placing `correlation_id` only on event structs (not `TriggerAction`):** This would require passing it separately through every pipeline stage. It belongs on the root struct so it's "free" at all downstream sites.
- **Using `Option<String>` for `correlation_id`:** Every code path that constructs `TriggerAction` (including P2P-received triggers) should generate an ID. Making it optional adds unwrap/default complexity with no benefit.
- **Changing the Tauri event name `"submission"`:** Existing frontend code listens for `"submission"`. Changing it would break the running app. Only add the new `"submission_failed"` name.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Unique ID generation | Custom timestamp-based string | `uuid::Uuid::now_v7().to_string()` | Already in workspace; time-ordered, collision-safe, consistent with `WorkflowId` generation in CLI |
| Linking trigger to submission | In-memory HashMap in dispatcher | `correlation_id` field on `TriggerAction` | HashMap adds state and concurrency complexity; field on struct is free and survives serialization |

**Key insight:** The `TriggerAction` struct already travels the entire pipeline intact (trigger → engine → submission → aggregator → dispatcher). Placing `correlation_id` on it means zero intermediate plumbing.

## Common Pitfalls

### Pitfall 1: `TriggerAction` has `bincode` derives — new field needs `#[bincode(with_serde)]` or a compatible type

**What goes wrong:** `TriggerAction` derives `bincode::Decode` and `bincode::Encode`. A plain `String` field encodes fine natively with bincode, but if the field were a `Uuid` type it would need `#[bincode(with_serde)]`. Using `String` avoids this issue entirely.

**Why it happens:** bincode 2.x requires explicit serde bridge annotations for types that don't implement bincode traits natively.

**How to avoid:** Store as `String` (not `uuid::Uuid`) in the struct. Generate with `Uuid::now_v7().to_string()` at construction time.

**Warning signs:** Compile error `the trait bound 'Uuid: bincode::Decode' is not satisfied`.

### Pitfall 2: ~15 `TriggerAction { ... }` construction sites will fail to compile

**What goes wrong:** Adding a non-optional field to `TriggerAction` causes every struct literal construction to fail.

**Why it happens:** Rust requires all fields in struct literals.

**How to avoid:** Enumerate all sites before adding the field. Key sites [VERIFIED: grep output]:
- `packages/wavs/src/subsystems/trigger.rs` — lines 817, 898, 925, 1039, 1116, 1162, 1262 (production code)
- `packages/wavs/src/http/handlers/debug.rs` — lines 49, 206 (dev HTTP handlers)
- `packages/wavs/src/subsystems/engine/wasm_engine.rs` — line 701 (aggregator path)
- `packages/wavs/tests/wavs_systems/mock_trigger_manager.rs` — lines 30, 55, 263, 267
- `packages/wavs/tests/mock_e2e.rs` — line 277
- `packages/wavs/benches/common/src/engine_setup.rs` — line 134

**Warning signs:** Cascading compile errors across multiple crates.

### Pitfall 3: `SubmissionConfirmed` currently only carries `service_id`, `workflow_id`, `trigger_data`

**What goes wrong:** The `DispatcherCommand::SubmissionConfirmed` variant (dispatcher.rs line 131) must also receive `correlation_id`. The aggregator constructs this at line 638 of `aggregator.rs` where `submission.trigger_action.correlation_id` is available.

**How to avoid:** Add `correlation_id: String` to the `SubmissionConfirmed` variant and update the single construction site in `aggregator.rs`.

### Pitfall 4: Two distinct failure sites in `submission.rs`, not one

**What goes wrong:** Assuming a single error hook is sufficient. There are actually two independent silent drop sites:
1. `sign_request` failure (line 110-116) — signing error, happens before the submission is even built
2. `dispatch` failure (line 125-131) — network/aggregator error, happens after signing

**Why it happens:** Both currently call `return;` with only a `tracing::error!`. Both have `req` in scope which has `req.trigger_action.correlation_id`.

**How to avoid:** Add `DispatcherCommand::SubmissionFailed` send in BOTH error arms. Both need the `subsystem_to_dispatcher_tx` sender, which `SubmissionManager` already holds.

### Pitfall 5: Frontend `ActivityItem` must be updated to carry `correlationId`

**What goes wrong:** The `ActivityItem` interface in `app/src/types/index.ts` (line 297) is what gets stored in the Zustand store. If it doesn't include `correlationId`, the frontend can't build the unified activity model described in the phase goal.

**How to avoid:** Update `ActivityItem` in `types/index.ts`, `TriggerAction` interface, `SubmissionEvent` interface, and add `SubmissionFailedEvent`. Update `listeners.ts` to (a) pass `correlationId` into `addActivity` and (b) register a new listener for `submission_failed`.

## Code Examples

Verified patterns from existing codebase:

### Generating a uuid v7 ID (matches existing CLI pattern)
```rust
// Source: packages/cli/src/command/service.rs line 622 [VERIFIED: grep output]
use uuid::Uuid;
let correlation_id = Uuid::now_v7().as_hyphenated().to_string();
```

### Sending DispatcherCommand from submission subsystem (existing dispatch site)
```rust
// Source: packages/wavs/src/subsystems/submission.rs lines 229-231 [VERIFIED: codebase read]
self.subsystem_to_dispatcher_tx
    .send(DispatcherCommand::SubmissionResponse(submission))
    .map_err(Box::new)?;
```

For `SubmissionFailed`, this becomes:
```rust
// In the sign_request error arm (line 110-116) and dispatch error arm (125-131):
let _ = _self.subsystem_to_dispatcher_tx.send(DispatcherCommand::SubmissionFailed {
    service_id: req.service_id().clone(),
    workflow_id: req.workflow_id().clone(),
    correlation_id: req.trigger_action.correlation_id.clone(),
    error: e.to_string(),
});
```

### Emitting a Tauri event in dispatcher (existing pattern)
```rust
// Source: packages/wavs/src/dispatcher.rs lines 460-471 [VERIFIED: codebase read]
if let Err(err) = _self.tauri_handle.emit_ext(
    wavs_gui_shared::event::SubmissionEvent {
        service_id,
        workflow_id,
        trigger_data,
        // NEW: correlation_id,
    },
) {
    tracing::error!("Error emitting submission event to GUI: {:?}", err);
}
```

### TypeScript listener registration (existing pattern)
```typescript
// Source: app/src/tauri/listeners.ts lines 58-69 [VERIFIED: codebase read]
const unlistenSubmissionFailed = await listen<SubmissionFailedEvent>('submission_failed', (event) => {
  const payload = event.payload;
  store.addActivity({
    id: nextActivityId(),
    ts: Date.now(),
    kind: 'submission_failed',
    serviceId: payload.service_id,
    workflowId: payload.workflow_id,
    correlationId: payload.correlation_id,
    error: payload.error,
  });
});
unlistenFns.push(unlistenSubmissionFailed);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Trigger and submission as independent GUI events, no linkage | Add `correlation_id` as a shared key | This phase | Frontend can group trigger + submission as a single activity unit |
| Submission failures only in `tracing::error!` | New `SubmissionFailedEvent` to GUI | This phase | GUI can surface failures in the activity feed instead of requiring log inspection |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `correlation_id` as plain `String` does not need `#[bincode(with_serde)]` annotation | Architecture Patterns / Pitfall 1 | Low — Rust `String` implements bincode natively; only would matter if we used `Uuid` type |
| A2 | `SubmissionManager` already holds `subsystem_to_dispatcher_tx` sender, making it straightforward to send `SubmissionFailed` from error arms | Architecture Patterns | Low — field is confirmed on `SubmissionManager` struct at line 35 of submission.rs [VERIFIED: codebase read] |
| A3 | The P2P-distributed trigger path (if triggers are re-broadcast over libp2p) preserves `TriggerAction` fields intact including `correlation_id` | Architecture Patterns | Medium — if P2P deserializes `TriggerAction` and the remote side is on an older binary without `correlation_id`, bincode deserialization will fail. In practice, single-operator dev mode is the target for this phase. |

## Open Questions

1. **P2P / multi-operator bincode compatibility**
   - What we know: `TriggerAction` derives `bincode::Decode + bincode::Encode` and is likely sent over the wire in multi-operator P2P mode
   - What's unclear: Whether adding a new non-optional field to `TriggerAction` breaks wire compatibility with existing deployed nodes that don't have `correlation_id`
   - Recommendation: For Phase 4 (infrastructure/dev phase), accept the incompatibility. If the codebase has a versioned wire protocol, consult before finalizing. If all operators always upgrade together (single-node dev target), this is a non-issue.

2. **`ActivityKind` in frontend — add `'submission_failed'` or use status field?**
   - What we know: `ActivityKind = 'trigger' | 'submission'` in `types/index.ts` line 295
   - What's unclear: Whether `SubmissionFailedEvent` should create a new `ActivityItem` (kind `'submission_failed'`) or update an existing trigger item's status
   - Recommendation: Create a new `ActivityItem` with `kind: 'submission_failed'` — matches existing pattern where each event is an independent item. The `correlationId` on both the trigger item and failed item lets the frontend link them visually.

## Environment Availability

Step 2.6: SKIPPED (no external dependencies — pure Rust/TypeScript code changes with no new external tools, services, or CLIs required).

## Security Domain

Security enforcement not applicable to this phase — no authentication, authorization, input validation from external sources, or cryptographic operations are introduced. The `correlation_id` is an internal node-generated UUID, not user-supplied data.

## Sources

### Primary (HIGH confidence)
- `packages/gui/shared/src/event.rs` — All current event types: `TriggerEvent`, `SubmissionEvent`, `SettingsEvent`, `LogEvent` [VERIFIED: codebase read]
- `packages/wavs/src/dispatcher.rs` — `DispatcherCommand` enum, both event emission sites, `TauriHandle::emit_ext` pattern [VERIFIED: codebase read]
- `packages/wavs/src/subsystems/submission.rs` — Two silent failure sites at lines 110-116 and 125-131 [VERIFIED: codebase read]
- `packages/wavs/src/subsystems/aggregator.rs` — `SubmissionConfirmed` construction site at line 638 [VERIFIED: codebase read]
- `packages/types/src/service.rs` — `TriggerAction` struct at line 491 [VERIFIED: codebase read]
- `app/src/types/index.ts` — TypeScript mirrors of all Rust event types [VERIFIED: codebase read]
- `app/src/tauri/listeners.ts` — All Tauri event listener registrations [VERIFIED: codebase read]
- `Cargo.toml` (workspace) — `uuid = { version = "1.18.1", features = ["v7", "serde"] }` at line 223 [VERIFIED: codebase read]
- `packages/types/Cargo.toml` — `uuid` NOT yet a dependency of `wavs-types` [VERIFIED: codebase read]

### Secondary (MEDIUM confidence)
- `packages/cli/src/command/service.rs` line 622 — `Uuid::now_v7().as_hyphenated().to_string()` as the established UUID generation pattern in this codebase [VERIFIED: grep output]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in workspace, usage patterns confirmed by reading source
- Architecture: HIGH — all construction sites, error sites, and emission sites verified by codebase search
- Pitfalls: HIGH — all pitfalls derived from direct codebase reading, not assumptions

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable infrastructure, low churn expected)
