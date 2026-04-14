# Pitfalls Research

**Domain:** WAVS v1.3 — Activity UX improvements, result decoding, service restart fix, wallet kebab menu
**Researched:** 2026-04-09
**Confidence:** HIGH — analysis based on direct code inspection of all affected files

## Critical Pitfalls

### Pitfall 1: DispatcherCommand enum break on new fields

**What goes wrong:**
`DispatcherCommand` is a non-exhaustive enum matched in `dispatcher.rs` `start()` via a `while let Ok(command) = ...recv()` + `match command { ... }`. Adding a new variant (e.g., a `SubmissionConfirmedWithResult` carrying tx_hash and payload bytes) compiles fine but any other match site that uses a wildcard `_` arm silently swallows the new variant. Alternatively, if the approach chosen is to add fields directly to the existing `SubmissionConfirmed` variant (struct-style destructuring), every match site must be updated or it will not compile at all — which is the safer outcome. The dangerous path is adding a new variant and missing a match arm somewhere.

**Why it happens:**
There are two subsystems that currently read from `subsystem_to_dispatcher_rx`: the main dispatcher loop and the kill/shutdown handler. Adding a new enum variant without auditing every `match` on `DispatcherCommand` causes silent no-ops.

**How to avoid:**
Prefer adding fields to the existing `SubmissionConfirmed { ... }` variant (struct-style) rather than adding a new variant. This makes it a compile error at every existing match site, forcing an explicit audit. The aggregator's `handle_submit_action` already has `tx_resp.tx_hash()` available at the `SubmissionConfirmed` send site — the new `tx_hash: String` and `execution_result: Vec<u8>` fields can be threaded directly through there without new variants.

**Warning signs:**
- A new `DispatcherCommand` variant compiles cleanly despite no frontend change — suspect a silent wildcard arm somewhere.
- The activity feed shows no tx_hash even after the Rust side purportedly emits it — check if the event struct was updated in `event.rs` but not the matching `SubmissionEvent` TypeScript interface.

**Phase to address:**
Phase that adds richer activity data (v1.3 Phase 1).

---

### Pitfall 2: Rust event struct addition without matching TypeScript interface update

**What goes wrong:**
The Tauri event pipeline is: Rust `SubmissionEvent` struct (in `packages/gui/shared/src/event.rs`) serialized to JSON via `#[serde(rename_all = "snake_case")]`, emitted to the frontend, and deserialized by the TypeScript `SubmissionEvent` interface in `app/src/types/index.ts`. The frontend listener in `app/src/tauri/listeners.ts` maps the payload to `ActivityItem`. Adding `tx_hash: String` and `execution_result: Vec<u8>` (hex-serialized by serde) in Rust is silent on the frontend side — TypeScript will simply ignore the unknown fields. If the `ActivityItem` type and its creation in `listeners.ts` is not updated in the same commit, the new data is serialized and emitted but never stored, never displayed.

**Why it happens:**
There is no compile-time link between the Rust serde shape and the TypeScript interface. The gap is invisible until you open the activity feed and notice tx_hash is missing.

**How to avoid:**
Update Rust struct, TypeScript interface, and the `addActivity(...)` call in `listeners.ts` in the same diff. The `WasmResponse.payload` field uses `#[serde(with = "const_hex")]` so on the wire it is a hex string, not a `number[]`. The TypeScript interface should declare `execution_result: string` (hex), not `number[]`. Do not invent a separate byte-array encoding.

**Warning signs:**
- TypeScript console shows the Tauri event payload has `tx_hash` but `ActivityItem` in the Zustand store does not.
- The activity card renders "—" for tx_hash even when the aggregator log shows a successful on-chain submission.

**Phase to address:**
Same phase as adding richer activity cards. Must be one atomic change across Rust + TypeScript.

---

### Pitfall 3: Service restart — triggers firing before service is in DB

**What goes wrong:**
`change_service_inner` (the code path used on WAVS process restart to update stale service definitions) calls `add_service_to_managers` after `self.services.save(&service)`, which looks correct. However, `add_service_to_managers` immediately sends `TriggerCommand` variants onto `trigger_manager.command_sender`. The trigger manager processes these in a separate thread. If a cron or block-interval trigger fires in the nanoseconds between `add_service_to_managers` returning and any subsequent initialization, the `DispatcherCommand::Trigger` arrives before the dispatcher has finished iterating over all registry entries. At that moment the dispatcher's `services.get()` call will find the service (it was saved first), but the engine manager may not yet have stored the WASM component bytes for that service, causing a silent drop.

The reported service restart bug is more specifically: services registered via `register_and_add_service` during startup may fail to re-register triggers because the `already_in_memory` fast-path in `start()` skips `add_service_to_managers` for services loaded from the settings cache before the HTTP server came up. Those services have their DB records but no active trigger streams after a WAVS process restart.

**Why it happens:**
The startup path in `dispatcher.rs::start()` has two load paths: (A) fast-load from settings cache which calls `self.services.save()` but potentially skips `add_service_to_managers`, and (B) the service registry restore loop which does call `add_service_to_managers`. If Path A runs before Path B for the same service, the `already_in_memory` check fires true in Path B and the manager registration is skipped.

**How to avoid:**
Audit the `already_in_memory` branch. The comment says "skip manager setup" when the service was already loaded from settings cache — but this means trigger streams are never set up for those services after restart. Either: (1) remove the `already_in_memory` skip so manager setup always runs from the authoritative on-chain version, or (2) ensure Path A itself calls `add_service_to_managers`. Add a tracing log at each skip point so it is visible in Jaeger after restart.

**Warning signs:**
- WAVS restarts cleanly (no errors) but a service that was active before restart never fires triggers after restart.
- `trigger_manager.command_sender` receives zero `StartListeningCron` commands at startup in traces, but services exist in DB.
- The Jaeger trace shows `Dispatcher received trigger action` for some services but not others after restart.

**Phase to address:**
Dedicated service restart fix phase.

---

### Pitfall 4: UTF-8 / JSON decode of Vec<u8> payload edge cases

**What goes wrong:**
The proposed decode chain is: try UTF-8 → try JSON pretty-print → fall back to hex. Four edge cases break this naively:

1. **`const_hex` wire encoding.** `WasmResponse.payload` is serialized with `#[serde(with = "const_hex")]`. On the wire it is a hex string like `"48656c6c6f"`, not `[72, 101, 108, 108, 111]`. The TypeScript decode chain must hex-decode this string to `Uint8Array` first, then attempt UTF-8 decode, then JSON. If the field is incorrectly treated as a number array (copying the `TriggerData.Raw` pattern which is `number[]`), `TextDecoder.decode` receives a string and fails silently producing garbage.

2. **Large payloads over Tauri IPC.** `WasmResponse::DEFAULT_MAX_PAYLOAD_SIZE` is 50 MB. A 50 MB payload hex-serialized is 100 MB of string sent over the Tauri IPC boundary. The IPC has no enforced size limit; 100 MB in a single Tauri event freezes the WebView. The backend should truncate `execution_result` to a display-safe size (e.g., 4 KB) before emitting the Tauri event.

3. **Valid UTF-8 that is not JSON but looks like it.** A payload of `{malformed` passes the UTF-8 check, fails `JSON.parse`, and should fall through to showing the raw UTF-8 string — not hex. The try/catch around `JSON.parse` must preserve the UTF-8 string when JSON parse fails.

4. **Null bytes in otherwise valid UTF-8.** A `Vec<u8>` that is valid UTF-8 but contains `\0` bytes will decode without error but render incorrectly in some React text nodes. Use `TextDecoder` with `fatal: false` and strip null bytes from display output.

**Why it happens:**
Developers copy the `TriggerData.Raw` TypeScript type (which is `number[]`) and assume `WasmResponse.payload` uses the same serialization. It does not — `const_hex` produces a hex string, not an array.

**How to avoid:**
Define one utility function `decodePayload(hexStr: string): string` that: hex-decodes to `Uint8Array`, attempts `TextDecoder.decode` with `fatal: true`, on failure returns hex string with `0x` prefix; if UTF-8 succeeded, attempts `JSON.parse`, on success returns `JSON.stringify(parsed, null, 2)`, on failure returns the UTF-8 string. Apply a `MAX_DISPLAY_BYTES = 4096` truncation before hex-decoding and show a "truncated — N bytes total" notice if exceeded. Apply the size cap in the Rust Tauri event emission, not just in the display layer.

**Warning signs:**
- Activity card shows garbled characters for a service that returns ASCII text.
- UI freezes or becomes unresponsive when a high-throughput service produces large payloads.
- The decode chain always falls through to hex for known UTF-8 outputs.

**Phase to address:**
Phase adding smart result decoding to the activity feed.

---

### Pitfall 5: Wallet kebab menu — confirmation state and modal ownership

**What goes wrong:**
The existing `WalletSection.tsx` manages its own `showResetConfirm` boolean state. The proposed kebab menu moves "Reset Wallet" and "Export Recovery Phrase" behind a three-dot trigger. Two sub-pitfalls:

1. **Dropdown auto-closes before handlers run.** The existing `DropdownMenu` component calls `setIsOpen(false)` inside the `onClick` wrapper before invoking the user's `onClick` handler. This means any state managed inside the dropdown is lost. The confirmation step must live in `WalletSection` parent state, not inside a dropdown option handler.

2. **Confirmation dialog ownership.** If a `Modal.open(...)` imperative pattern is used (as in `OwnerActionsMenu`), the modal does not tie into `WalletSection` React state — `showResetConfirm` state becomes stale and `handleResetWallet` may close over incorrect values. Choose one pattern: inline confirm panel (existing `WalletSection` pattern) or modal (`OwnerActionsMenu` pattern). Do not mix.

3. **Danger variant styling.** The `DropdownMenu` component supports `variant: 'danger'` on `MenuOption` which applies `text-red-3`. "Reset Wallet" must use `variant: 'danger'`. Forgetting this makes a destructive action look identical to a non-destructive one.

**Why it happens:**
The `DropdownMenu.tsx` auto-close is an intentional UX pattern for action menus but is invisible unless you read the component source. Developers expect the dropdown to stay open during a confirmation step.

**How to avoid:**
Keep all multi-step state (`showMnemonic`, `showResetConfirm`) in `WalletSection` component state. Kebab menu options are pure action triggers that set parent state booleans (`onClick: () => setShowResetConfirm(true)`). Confirm and execute actions happen in the `WalletSection` render tree, outside the dropdown. Use `variant: 'danger'` for the reset option.

**Warning signs:**
- Clicking "Reset Wallet" in the kebab menu immediately resets without confirmation.
- The seed phrase panel renders inside the dropdown and disappears when the dropdown closes.
- The reset confirm panel appears but the "Yes, Reset Wallet" button does nothing.

**Phase to address:**
Phase adding wallet kebab menu.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Adding `tx_hash` as `Option<String>` in event struct | Avoids breaking existing consumers that don't care about tx_hash | Optional fields accumulate; struct shape becomes hard to reason about | Acceptable here — tx_hash is genuinely absent for non-aggregated (Submit::None) workflows |
| Truncating payload display to 4 KB client-side | Avoids IPC freeze with minimal code | User cannot see full result without an additional "copy full payload" affordance | Acceptable — add a "copy full payload" Tauri command alongside display truncation |
| Reusing existing `DropdownMenu` for the kebab trigger | Zero new component | `DropdownMenu` requires a text `label` prop; three-dot kebab icon requires a workaround (pass `"⋮"` as label) | Acceptable for v1.3; add an `icon` prop variant to `DropdownMenu` later |
| Not serializing `DispatcherCommand` via bincode | Simpler change | Not a concern — `DispatcherCommand` is a crossbeam-local in-process channel, never serialized to disk or network | Always fine |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `WasmResponse.payload` wire format | Treating the hex string as `number[]` (copying `TriggerData::Raw` pattern) | `TriggerData::Raw` is `number[]` on the wire; `WasmResponse.payload` uses `const_hex` and is a hex string — hex-decode first |
| Tauri event payload field names | Using camelCase TypeScript field names that don't match Rust `snake_case` serde output | Rust `#[serde(rename_all = "snake_case")]` on event structs means TypeScript fields must use `snake_case` names exactly |
| `DispatcherCommand::SubmissionConfirmed` struct variant | Adding a new enum variant instead of extending the existing one, relying on wildcard `_` arms | Add fields to existing `SubmissionConfirmed` variant so the compiler flags every stale destructuring match |
| Aggregator `tx_hash` availability | Assuming tx_hash is a `String` — it is actually a `TxHash` (alloy `FixedBytes<32>`) requiring `.to_string()` or hex formatting | Call `tx_resp.tx_hash().to_string()` or use `alloy_primitives::hex::encode` at the send site |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Large `execution_result` over Tauri IPC | WebView freezes; UI becomes unresponsive | Cap payload at 4 KB in the Rust Tauri event emission before it reaches IPC | Any payload > ~1 MB hex-encoded (~512 KB raw bytes) |
| `useMemo` in `useGroupedActivity` recomputes on every `addActivity` | Activity feed re-renders entire group list on each new event | Acceptable at hundreds of events; add virtualization if list grows to thousands | >5000 activity items in the store |
| `JSON.parse` on every render of payload display | Redundant parse work on re-renders | Memoize decoded result per `ActivityItem.id` | High-frequency trigger services; >100 items visible simultaneously |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Logging `execution_result` payload bytes at INFO level | Execution results may contain sensitive application data | Log at DEBUG level for payload content; INFO only for tx_hash and service/workflow identifiers |
| Seed phrase rendered in DOM without unmount cleanup | Phrase stays in DOM if user navigates away mid-reveal | Existing `handleHideMnemonic` clears state; also add it to `useEffect` cleanup for the section; verify kebab option does not bypass this |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Showing raw hex for all byte payloads | Unreadable for the majority of services returning UTF-8 text or JSON | Apply UTF-8 → JSON → hex decode chain; default to most human-readable form |
| Showing full 64-char tx_hash inline in card | Card overflow, especially in narrow sidebar layouts | Truncate to `0x1234...abcd` (first 6 + last 4 chars) with click-to-copy |
| Placing "Reset Wallet" and "Export Recovery Phrase" at same visual weight | User may accidentally trigger destructive action | `variant: 'danger'` for Reset; place it last in the options array; separator line between safe and destructive options if the component supports it |
| Showing "pending" status indefinitely for Submit::None workflows | User thinks the service is stuck when it completed correctly | Distinguish Submit::None workflows from stuck ones; "no submission" label instead of pending indicator |

## "Looks Done But Isn't" Checklist

- [ ] **Richer activity cards:** `tx_hash` added to Rust `SubmissionConfirmed` but not threaded through `SubmissionEvent` TypeScript interface — verify `activityItem.tx_hash` is non-null in Zustand devtools after a real on-chain submission.
- [ ] **Richer activity cards:** `execution_result` displays as hex string because decode chain was not applied — verify a service returning plain ASCII text shows the text, not hex.
- [ ] **Service restart fix:** WAVS restarts with no errors but the cron-triggered service never fires again — verify Jaeger shows `StartListeningCron` commands after restart.
- [ ] **Service restart fix:** The `already_in_memory` skip path still skips `add_service_to_managers` — check that every startup code path that saves a service also registers it with managers.
- [ ] **Wallet kebab:** Clicking "Reset Wallet" from the menu triggers immediate deletion without confirmation — verify `showResetConfirm` state is set true, not `handleResetWallet` called directly.
- [ ] **Wallet kebab:** "Export Recovery Phrase" option closes the menu and the phrase panel appears in `WalletSection`, not inside a floating dropdown element — verify DOM structure.
- [ ] **Result decode:** A payload from a high-output service does not freeze the UI — verify truncation is applied at the Rust IPC emission site, not only at the display layer.

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| DispatcherCommand new variant with silent wildcard arm | MEDIUM | Grep all `match command` sites in the codebase; add explicit arms; no data loss, just missing events |
| TypeScript interface not updated to match new Rust fields | LOW | Add fields to `SubmissionEvent` and `ActivityItem`; the Tauri event already carries the data — Zustand store starts populating immediately on next event |
| Service restart loses triggers | HIGH | Audit `already_in_memory` branch; add `add_service_to_managers` call for the settings-cache load path; requires WAVS restart to verify |
| Payload decode always falls to hex | LOW | Fix `const_hex` handling in `decodePayload` utility; no backend change needed |
| Wallet kebab bypasses confirmation | LOW | Move confirmation state to parent component; add `showResetConfirm` gate before calling `handleResetWallet` |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| DispatcherCommand enum break | Phase adding richer activity data (Rust side) | `cargo build` with exhaustive match; search all `match command` sites |
| Rust/TypeScript struct field mismatch | Same phase as Rust event struct change | Open activity feed after a real submission; confirm `tx_hash` field is populated in Zustand devtools |
| Service restart trigger loss | Dedicated service restart fix phase | Restart WAVS; confirm cron trigger fires within expected interval; check Jaeger for `StartListeningCron` after restart |
| Payload `const_hex` decode mishandled | Smart result decoding phase | Test with empty payload, UTF-8 text payload, JSON payload, binary payload, 10 MB payload |
| Wallet kebab confirmation state | Wallet kebab menu phase | Click "Reset Wallet" from menu; confirm panel appears; confirm cancel works; confirm reset executes after confirmation only |

## Sources

- Direct code inspection: `/workspace/packages/wavs/src/dispatcher.rs` — `DispatcherCommand` enum, `start()` main loop, `SubmissionConfirmed` send site, `already_in_memory` branch
- Direct code inspection: `/workspace/packages/gui/shared/src/event.rs` — Tauri event structs and `TauriEventExt` trait
- Direct code inspection: `/workspace/packages/types/src/service.rs` — `WasmResponse` with `const_hex` payload encoding
- Direct code inspection: `/workspace/packages/types/src/submission.rs` — `Submission` struct with `operator_response: WasmResponse`
- Direct code inspection: `/workspace/packages/wavs/src/subsystems/aggregator.rs` — `SubmissionConfirmed` dispatch at line 638, `tx_resp.tx_hash()` at line 632
- Direct code inspection: `/workspace/app/src/types/index.ts` — `ActivityItem`, `SubmissionEvent`, `TriggerData` types; note `TriggerData::Raw` is `number[]` vs `WasmResponse.payload` hex string
- Direct code inspection: `/workspace/app/src/tauri/listeners.ts` — event listener pipeline and `addActivity` call sites
- Direct code inspection: `/workspace/app/src/hooks/useGroupedActivity.ts` — correlation grouping logic
- Direct code inspection: `/workspace/app/src/components/atoms/DropdownMenu.tsx` — auto-close behavior on option click (line 56: `setIsOpen(false)` before `option.onClick()`)
- Direct code inspection: `/workspace/app/src/components/settings/WalletSection.tsx` — existing multi-step wallet state management pattern
- Direct code inspection: `/workspace/app/src/components/poa/OwnerActionsMenu.tsx` — existing `DropdownMenu` usage with `Modal.open` pattern

---
*Pitfalls research for: WAVS v1.3 Activity UX & Bug Fixes*
*Researched: 2026-04-09*
