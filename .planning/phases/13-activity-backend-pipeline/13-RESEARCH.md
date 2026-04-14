# Phase 13: Activity Backend Pipeline - Research

**Researched:** 2026-04-09
**Domain:** Rust IPC pipeline — SubmissionEvent struct, DispatcherCommand, aggregator send site, TypeScript type mirroring
**Confidence:** HIGH

## Summary

This is a pure plumbing phase with four well-defined Rust touch points and two TypeScript touch points. The goal is to carry `tx_hash: String` and `result_payload: Option<Vec<u8>>` (capped at 4 KB) from the aggregator's `Ok(tx_resp)` branch through `DispatcherCommand::SubmissionConfirmed`, through the dispatcher match arm, into `SubmissionEvent`, and finally across Tauri IPC to TypeScript.

All code paths have been read directly from source. The aggregator already has `tx_resp.tx_hash()` (returns `String`) at the exact send site. The execution result bytes live in `submission.operator_response.payload` (`Vec<u8>`, hex-serialized in the wider codebase as `const_hex`). The dispatcher match arm already constructs a `SubmissionEvent` literal — adding two fields there is the only dispatcher change needed.

The TypeScript side mirrors the Rust struct via serde `rename_all = "snake_case"`. Because there is no compile-time link between Rust and TypeScript, all three layers (Rust struct, TS interface, listeners.ts destructuring) must be updated atomically in a single commit to avoid a silent runtime mismatch where events arrive without the new fields.

**Primary recommendation:** Add `tx_hash: String` and `result_payload: Option<Vec<u8>>` to exactly 4 Rust locations and 2 TypeScript locations, serialize `result_payload` as `option_const_hex` (hex string or null over IPC), extend `ActivityItem` with matching optional fields.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from STATE.md:
- result_payload capped at 4 KB in Rust before IPC to avoid 100 MB hex blowup
- Rust struct + TypeScript interface + listeners.ts must change atomically (no compile-time link)

### Claude's Discretion
All implementation details.

### Deferred Ideas (OUT OF SCOPE)
None — infrastructure phase.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ACT-01 | Submission events forward tx_hash from aggregator to frontend via SubmissionEvent pipeline | `tx_resp.tx_hash()` returns `String` at aggregator line ~632; add field to `DispatcherCommand::SubmissionConfirmed`, `SubmissionEvent`, TS interface, listeners.ts |
| ACT-02 | Submission events forward execution result payload (capped at 4KB) from aggregator to frontend | `submission.operator_response.payload` is `Vec<u8>` at aggregator send site; truncate to 4096 bytes before inserting into `DispatcherCommand`; serialize as hex via `option_const_hex` pattern |
</phase_requirements>

---

## Standard Stack

No new dependencies needed. All tools are already in the project.

### Core (already present)
| Component | Location | Purpose |
|-----------|----------|---------|
| `wavs_gui_shared::event` | `packages/gui/shared/src/event.rs` | Defines all Tauri event structs including `SubmissionEvent` |
| `DispatcherCommand` enum | `packages/wavs/src/dispatcher.rs:118` | Message bus between aggregator and dispatcher loop |
| Aggregator send site | `packages/wavs/src/subsystems/aggregator.rs:636–643` | Where `SubmissionConfirmed` is constructed and sent |
| `AnyTransactionReceipt::tx_hash()` | `packages/wavs/src/subsystems/aggregator/submit.rs:30` | Returns `String`; available at send site |
| `option_const_hex` serde helper | `packages/types/src/serde_helpers.rs` | Serializes `Option<Vec<u8>>` as hex-prefixed string or null |
| TS `SubmissionEvent` interface | `app/src/types/index.ts:108` | Must mirror Rust struct field names exactly |
| Tauri event listener | `app/src/tauri/listeners.ts:60` | Destructures `event.payload` into `ActivityItem` |
| `ActivityItem` interface | `app/src/types/index.ts:330` | Destination shape for activity feed entries |

### Installation
No new packages needed.

---

## Architecture Patterns

### Current Pipeline (verified by reading source)

```
Aggregator (aggregator.rs ~636)
  └─ sends DispatcherCommand::SubmissionConfirmed { service_id, workflow_id, trigger_data, correlation_id }
        │
        ▼
Dispatcher loop (dispatcher.rs ~462)
  └─ match arm constructs SubmissionEvent { service_id, workflow_id, trigger_data, correlation_id }
  └─ calls tauri_handle.emit_ext(event)
        │
        ▼
Tauri IPC (JSON serialized via serde)
        │
        ▼
listeners.ts listen<SubmissionEvent>()
  └─ destructures payload
  └─ calls store.addActivity({ kind: 'submission', ... })
        │
        ▼
appStore.activityList (Zustand)
```

### Post-Phase Pipeline (target state)

```
Aggregator (aggregator.rs ~636)
  ├─ tx_hash = tx_resp.tx_hash()                     // already String
  ├─ result_payload = submission.operator_response.payload[..4096].to_vec()  // cap here
  └─ sends DispatcherCommand::SubmissionConfirmed { ..., tx_hash, result_payload: Some(result_payload) }

Dispatcher loop match arm
  └─ constructs SubmissionEvent { ..., tx_hash, result_payload }

SubmissionEvent (event.rs)
  └─ pub tx_hash: String
  └─ #[serde(with = "option_const_hex")]
     pub result_payload: Option<Vec<u8>>    // serializes as "0x..." or null

TypeScript SubmissionEvent interface
  └─ tx_hash: string
  └─ result_payload: string | null          // hex string or null

listeners.ts
  └─ txHash: payload.tx_hash,
  └─ resultPayload: payload.result_payload ?? null,

ActivityItem interface
  └─ txHash?: string
  └─ resultPayload?: string | null
```

### Pattern: option_const_hex for Optional Byte Payloads
**What:** The project already uses `option_const_hex` from `packages/types/src/serde_helpers.rs` for `Option<Vec<u8>>`. It serializes to a hex-prefixed string when `Some`, and `null` when `None`. [VERIFIED: read source]

**When to use:** Any `Option<Vec<u8>>` field crossing the Tauri IPC boundary.

**Example (from existing codebase):**
```rust
// Source: packages/types/src/service.rs:664
#[serde(with = "crate::serde_helpers::option_const_hex")]
pub event_id_salt: Option<Vec<u8>>,
```

**For the new field in gui/shared, the helper needs to be accessible.** Check if `wavs_types` is a dependency of `wavs_gui_shared`, or if a simpler inline approach is preferred. Alternative: use `#[serde(with = "const_hex")]` for a non-optional `Vec<u8>` if None is represented as empty bytes, but `Option` with the existing helper is cleaner.

### Pattern: Rust DispatcherCommand Named Fields
**What:** `SubmissionConfirmed` uses named struct-variant syntax, not tuple variant. Adding fields is straightforward — add to enum variant definition and all construction sites. [VERIFIED: read source at dispatcher.rs:131]

**Construction site count:** Exactly one — `packages/wavs/src/subsystems/aggregator.rs:638`. [VERIFIED: grep confirms single send site]

### Pattern: serde rename_all = "snake_case" on SubmissionEvent
**What:** `SubmissionEvent` is decorated with `#[serde(rename_all = "snake_case")]`. Field names `tx_hash` and `result_payload` will serialize as `"tx_hash"` and `"result_payload"` — which is what TypeScript already uses as naming convention. No custom rename annotations needed. [VERIFIED: event.rs:56]

### Anti-Patterns to Avoid
- **Serializing full payload without cap:** `submission.operator_response.payload` can be up to 50 MB (see `WasmResponse::DEFAULT_MAX_PAYLOAD_SIZE`). Cap to 4096 bytes in Rust **before** inserting into `DispatcherCommand`. Never pass uncapped bytes over IPC.
- **Partial update across layers:** If only the Rust struct is updated but not the TS interface, events will arrive with extra fields that TypeScript silently ignores — ACT-01 and ACT-02 will appear to work in Rust but the frontend won't surface the data. All three layers must change in the same commit.
- **Using `const_hex` directly on Option:** The non-option `const_hex` serde attribute panics on `None`. Use `option_const_hex` for `Option<Vec<u8>>`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| `Option<Vec<u8>>` hex serialization | Custom serde impl | `option_const_hex` from `packages/types/src/serde_helpers.rs` | Already exists, tested, matches const_hex encoding used elsewhere |
| tx_hash formatting | Manual byte-to-hex | `AnyTransactionReceipt::tx_hash()` method | Already exists at submit.rs:30, returns `String` |
| 4 KB cap | Manual slice + clone | `payload[..payload.len().min(4096)].to_vec()` | Simple one-liner |

---

## Common Pitfalls

### Pitfall 1: option_const_hex not accessible from wavs_gui_shared
**What goes wrong:** `option_const_hex` lives in `wavs_types`. `wavs_gui_shared` may not depend on `wavs_types`.
**Why it happens:** Cross-crate dependency needed.
**How to avoid:** Check `packages/gui/shared/Cargo.toml` for `wavs-types` dependency. If absent, either add the dependency or inline a minimal hex serde helper directly in `event.rs`. Alternatively, represent `result_payload` as `Option<String>` (pre-encoded hex) in `SubmissionEvent` and encode in the dispatcher arm — avoids the serde helper entirely.
**Warning signs:** Compile error "use of unresolved module `wavs_types::serde_helpers`".

### Pitfall 2: TypeScript null vs undefined mismatch
**What goes wrong:** Rust `Option::None` serializes as JSON `null`. TypeScript `optional field` (`field?: T`) allows `undefined`. When `payload.result_payload` is `null`, destructuring as `payload.result_payload` gives `null`, not `undefined`.
**Why it happens:** JSON null and JS undefined are different.
**How to avoid:** In `listeners.ts`, use `payload.result_payload ?? undefined` when building the `ActivityItem` if the field is typed as `string | undefined`. Or type `ActivityItem.resultPayload` as `string | null` and pass `null` directly. Pick one convention and be consistent.
**Warning signs:** Null showing up as "null" string in UI components that expect undefined to mean "absent".

### Pitfall 3: Forgetting to update the mock test path
**What goes wrong:** `packages/wavs/tests/mock_e2e.rs` constructs `DispatcherCommand::SubmissionConfirmed` variants. Adding fields to the enum makes the existing test construction sites fail to compile.
**Why it happens:** Named struct variants require all fields to be specified.
**How to avoid:** `grep -rn "SubmissionConfirmed"` before finishing — find all construction sites. Update the mock test file at the same time.
**Warning signs:** Compile error in test file.

### Pitfall 4: result_payload cap placed at wrong layer
**What goes wrong:** Cap applied in the dispatcher arm instead of at the aggregator send site. The `DispatcherCommand` message (sent via `crossbeam::channel`) carries the full uncapped bytes across the channel.
**Why it happens:** Temptation to cap "right before emit" in the dispatcher.
**How to avoid:** Cap in `aggregator.rs` before constructing the `DispatcherCommand` — this is the canonical guidance from STATE.md. Channel messages can be large; cap early.

---

## Code Examples

All examples verified by reading source.

### Touch Point 1: event.rs — SubmissionEvent (add two fields)
```rust
// Source: packages/gui/shared/src/event.rs (current state at line 56)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubmissionEvent {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub trigger_data: TriggerData,
    pub correlation_id: String,
    // ADD:
    pub tx_hash: String,
    #[serde(with = "<hex_helper>")]   // option_const_hex or inline equivalent
    pub result_payload: Option<Vec<u8>>,
}
```

### Touch Point 2: dispatcher.rs — DispatcherCommand::SubmissionConfirmed (add two fields)
```rust
// Source: packages/wavs/src/dispatcher.rs line 131 (current state)
SubmissionConfirmed {
    service_id: ServiceId,
    workflow_id: WorkflowId,
    trigger_data: TriggerData,
    correlation_id: String,
    // ADD:
    tx_hash: String,
    result_payload: Option<Vec<u8>>,
},
```

### Touch Point 3: aggregator.rs — send site (add fields from available data)
```rust
// Source: packages/wavs/src/subsystems/aggregator.rs lines 628–643 (current state)
Ok(tx_resp) => {
    // tx_resp.tx_hash() -> String is already logged on line 632
    // submission.operator_response.payload is Vec<u8>
    let tx_hash = tx_resp.tx_hash();
    let raw_payload = &submission.operator_response.payload;
    let result_payload = if raw_payload.is_empty() {
        None
    } else {
        Some(raw_payload[..raw_payload.len().min(4096)].to_vec())
    };

    self.subsystem_to_dispatcher_tx
        .send(DispatcherCommand::SubmissionConfirmed {
            service_id: submission.service_id().clone(),
            workflow_id: submission.workflow_id().clone(),
            trigger_data: submission.trigger_action.data.clone(),
            correlation_id: submission.trigger_action.correlation_id.clone(),
            tx_hash,
            result_payload,
        })
```

### Touch Point 4: dispatcher.rs match arm — pass through to SubmissionEvent
```rust
// Source: packages/wavs/src/dispatcher.rs lines 462–481 (current state)
DispatcherCommand::SubmissionConfirmed {
    service_id,
    workflow_id,
    trigger_data,
    correlation_id,
    tx_hash,          // ADD: destructure
    result_payload,   // ADD: destructure
} => {
    if let Err(err) = _self.tauri_handle.emit_ext(
        wavs_gui_shared::event::SubmissionEvent {
            service_id,
            workflow_id,
            trigger_data,
            correlation_id,
            tx_hash,          // ADD
            result_payload,   // ADD
        },
    ) { ... }
}
```

### Touch Point 5: app/src/types/index.ts — SubmissionEvent interface
```typescript
// Source: app/src/types/index.ts line 108 (current state)
export interface SubmissionEvent {
  service_id: ServiceId;
  workflow_id: WorkflowId;
  trigger_data: TriggerData;
  correlation_id: string;
  // ADD:
  tx_hash: string;
  result_payload: string | null;  // hex-encoded bytes or null
}
```

### Touch Point 6: app/src/tauri/listeners.ts — submission event listener
```typescript
// Source: app/src/tauri/listeners.ts line 60 (current state)
const unlistenSubmission = await listen<SubmissionEvent>(EVENTS.SUBMISSION, (event) => {
  const payload = event.payload;
  store.addActivity({
    id: nextActivityId(),
    ts: Date.now(),
    kind: 'submission',
    serviceId: payload.service_id,
    workflowId: payload.workflow_id,
    triggerData: payload.trigger_data,
    correlationId: payload.correlation_id,
    // ADD:
    txHash: payload.tx_hash,
    resultPayload: payload.result_payload,
  });
});
```

### ActivityItem interface extension
```typescript
// Source: app/src/types/index.ts line 330 (current state)
export interface ActivityItem {
  id: number;
  ts: number;
  kind: ActivityKind;
  serviceId: ServiceId;
  workflowId: WorkflowId;
  triggerData?: TriggerData;
  triggerConfig?: TriggerConfig;
  correlationId?: string;
  error?: string;
  // ADD:
  txHash?: string;
  resultPayload?: string | null;
}
```

---

## Runtime State Inventory

Not applicable — greenfield field addition with no rename/refactor.

---

## Environment Availability

Step 2.6: SKIPPED — this is a pure code change with no new external dependencies. All required tools (Rust compiler, Cargo, Node.js, Vite) are standard project dependencies already verified by CLAUDE.md build commands.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test + layer-tests (E2E) |
| Config file | `packages/layer-tests/layer-tests.toml` |
| Quick run command | `cargo build -p wavs` (compile check) |
| Full suite command | `just test-wavs-e2e` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | Notes |
|--------|----------|-----------|-------------------|-------|
| ACT-01 | SubmissionEvent carries non-empty tx_hash | compile | `cargo build -p wavs -p wavs-gui-shared` | No unit test for IPC shape — success criteria is compile + manual inspection |
| ACT-02 | SubmissionEvent carries result_payload capped at 4 KB | compile + manual | `cargo build -p wavs -p wavs-gui-shared` | Cap logic is a trivial slice; verify via log output in dev run |

### Sampling Rate
- **Per task commit:** `cargo build -p wavs -p wavs-gui-shared` — catches all Rust struct/enum mismatches
- **Per wave merge:** `cargo build --workspace` — catches TS type errors via `just app-build-frontend`
- **Phase gate:** All compile clean + manual dev run confirms tx_hash and result_payload appear in Tauri events before `/gsd-verify-work`

### Wave 0 Gaps
None — existing infrastructure covers compilation validation. No new test files needed for this phase; the success criteria are observable in dev tooling.

---

## Security Domain

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | 4 KB cap on result_payload before IPC — prevents memory exhaustion from large WASM outputs |
| V6 Cryptography | no | — |
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |

### Known Threat Patterns
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Oversized IPC payload | DoS | Cap `result_payload` to 4096 bytes in Rust before channel send (STATE.md requirement) |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `option_const_hex` serde helper is accessible (or easily made accessible) from `wavs_gui_shared` | Code Examples (Touch Point 1) | If not available, need to inline a minimal hex helper or represent payload as pre-encoded `Option<String>` |
| A2 | `submission.operator_response.payload` at the aggregator send site contains the execution result bytes (not an intermediate or signed form) | Architecture Patterns | If wrong, need to find the correct field — but WasmResponse.payload is the WASM component output, which matches the requirement |

---

## Open Questions

1. **option_const_hex availability in wavs_gui_shared**
   - What we know: The helper is defined in `wavs-types`. `wavs_gui_shared` imports `wavs_types` types in `event.rs` (ServiceId, TriggerAction, etc.) so `wavs-types` likely IS a dependency already.
   - What's unclear: Whether `wavs_types::serde_helpers` is `pub` or `pub(crate)`.
   - Recommendation: Check `packages/gui/shared/Cargo.toml` and `packages/types/src/lib.rs` visibility. If `serde_helpers` is `pub(crate)`, the simplest fix is to represent `result_payload` as `Option<String>` (hex-encoded) in `SubmissionEvent` and encode with `const_hex::encode_prefixed` in the dispatcher arm before constructing the event — no serde helper needed.

2. **Mock E2E test construction sites for SubmissionConfirmed**
   - What we know: `packages/wavs/tests/mock_e2e.rs` exists and uses `DispatcherCommand`.
   - What's unclear: Whether it constructs `SubmissionConfirmed` variants directly.
   - Recommendation: `grep -rn "SubmissionConfirmed" /workspace/packages` before finalizing the plan to identify all construction sites.

---

## Sources

### Primary (HIGH confidence — verified by reading source files)
- `/workspace/packages/gui/shared/src/event.rs` — SubmissionEvent struct, TauriEventExt pattern, serde rename_all
- `/workspace/packages/wavs/src/dispatcher.rs:118–143` — DispatcherCommand enum with SubmissionConfirmed variant
- `/workspace/packages/wavs/src/dispatcher.rs:462–481` — match arm emitting SubmissionEvent
- `/workspace/packages/wavs/src/subsystems/aggregator.rs:628–643` — tx_resp.tx_hash() call and SubmissionConfirmed send site
- `/workspace/packages/wavs/src/subsystems/aggregator/submit.rs:30–35` — AnyTransactionReceipt::tx_hash() method
- `/workspace/packages/types/src/submission.rs` — Submission struct with operator_response: WasmResponse
- `/workspace/packages/types/src/service.rs:660–666` — WasmResponse struct with payload: Vec<u8>
- `/workspace/packages/types/src/serde_helpers.rs` — option_const_hex serde helper
- `/workspace/app/src/types/index.ts:108–120,330–340` — SubmissionEvent and ActivityItem TS interfaces
- `/workspace/app/src/tauri/listeners.ts:60–72` — submission event listener

### Secondary (MEDIUM confidence)
- None needed — all claims verified directly from source.

---

## Metadata

**Confidence breakdown:**
- Touch points identified: HIGH — all 4 Rust + 2 TS locations verified by reading source
- Implementation pattern: HIGH — option_const_hex helper exists and matches the need
- Pitfalls: HIGH — derived from direct source reading
- one open question (option_const_hex visibility): MEDIUM — likely accessible but not confirmed without Cargo.toml read

**Research date:** 2026-04-09
**Valid until:** 60 days — this is stable internal code, no external dependencies
