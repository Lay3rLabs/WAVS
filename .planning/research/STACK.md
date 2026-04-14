# Stack Research

**Domain:** Tauri 2 + React 19 desktop app — activity UX, result decoding, backend bug fixes, dropdown menus
**Researched:** 2026-04-09
**Confidence:** HIGH — all findings are from direct codebase inspection; no new external dependencies required

---

## Executive Summary

All four v1.3 features are implementable with zero new dependencies. The codebase already contains every primitive needed:

1. **tx_hash forwarding** — Pure Rust struct change: add `tx_hash: Option<String>` to `SubmissionConfirmed` in `DispatcherCommand` and mirror it in `SubmissionEvent` in `packages/gui/shared/src/event.rs`. The hash is already computed in `aggregator/submit.rs` via `AnyTransactionReceipt::tx_hash()` and passed to `DispatcherCommand::AggregatorExecute` as `AnyTxHash`. The `DispatcherCommand::SubmissionConfirmed` path already receives the `Submission` struct which holds `operator_response: WasmResponse` (the `payload` Vec<u8> field). Both tx_hash and the result payload need to flow through to the `SubmissionConfirmed` variant and into `SubmissionEvent`.

2. **Smart result decoding** — Pure TypeScript: `TextDecoder` (built into browsers/WebViews), `JSON.parse`, and hex formatting with `Array.from` are all zero-dependency. No library needed. Decode priority: UTF-8 → JSON pretty-print → hex fallback.

3. **Service restart race fix** — Pure Rust: the `StreamStartState` state machine in `trigger.rs` uses a `HashMap<ChainKey, StreamStartState>` that is local to `start_watcher`. When services are restored from the registry on startup, `add_service` sends `StartListeningChain` followed immediately by `WatchEvmContractEvents` in a loop. If `StartListeningChain` is still async-connecting, the subsequent `WatchEvmContractEvents` command arrives when `evm_controllers` has no entry for that chain, causing the subscription to be silently dropped. Fix is ordering-only with a local pending queue — no new crates.

4. **Wallet settings kebab menu** — `DropdownMenu` atom already exists at `app/src/components/atoms/DropdownMenu.tsx` with full click-outside handling and a `danger` variant. A `KebabMenu` wrapper requires only a Unicode character (U+22EE `⋮`) or an `iconTrigger` prop on the existing atom. No icon library needed.

---

## Recommended Stack

### Core Technologies (unchanged)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust (wavs_types, wavs_gui_shared) | existing | Backend event types, Tauri command contracts | Struct changes propagate to frontend via serde + TS types |
| Tauri 2 | ^2.x | Desktop shell, IPC | Already in use; `emit_ext` pattern is established |
| React 19 | ^19.1.0 | Frontend UI | Already in use |
| Zustand 5 | ^5.0.0 | Frontend state | Already in use; `ActivityItem` already stores all activity |
| TypeScript 5.8 | ~5.8.3 | Frontend type safety | Already in use; types/index.ts mirrors Rust event structs |
| clsx | ^2.1.0 | Conditional class names | Already in use in all components |

### Supporting Libraries (unchanged — nothing new)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| TextDecoder (Web API) | built-in | UTF-8 byte decoding | Decoding WasmResponse.payload in result display |
| JSON.parse (built-in) | built-in | JSON detection | Second pass in decode chain after UTF-8 succeeds |
| Array.from + toString(16) (built-in) | built-in | Hex fallback | Third pass when payload is not valid UTF-8 |

No npm installs required.

---

## Integration Points by Feature

### Feature 1: tx_hash + execution result forwarding (Rust to GUI)

**Data path today:**
```
aggregator/submit.rs
  -> AnyTransactionReceipt::tx_hash()          // String available here
  -> DispatcherCommand::SubmissionConfirmed { service_id, workflow_id, trigger_data, correlation_id }
  -> dispatcher.rs emit_ext(SubmissionEvent { service_id, workflow_id, trigger_data, correlation_id })
```

**Gap:** `tx_hash` and `operator_response.payload` are never forwarded into `SubmissionConfirmed`. They exist in scope — `tx_resp.tx_hash()` is logged at line 632 of aggregator.rs, and `submission.operator_response.payload` is in the `Submission` struct — but are dropped before the `SubmissionConfirmed` send.

**Change surfaces:**

- `packages/wavs/src/dispatcher.rs` — `DispatcherCommand::SubmissionConfirmed` variant: add `tx_hash: Option<String>` and `result_payload: Vec<u8>`
- `packages/wavs/src/subsystems/aggregator.rs` — where `SubmissionConfirmed` is constructed (around line 636): pass `Some(tx_resp.tx_hash())` and `submission.operator_response.payload.clone()`
- `packages/gui/shared/src/event.rs` — `SubmissionEvent`: add `tx_hash: Option<String>` and `result_payload: String` (hex-encoded for readability)
- `app/src/types/index.ts` — `SubmissionEvent` interface: add `tx_hash: string | null` and `result_payload: string`
- `app/src/stores/appStore.ts` or activity listener — `ActivityItem`: add `txHash?: string` and `resultPayload?: string` when consuming `SubmissionEvent`

**Payload encoding choice:** Carry `result_payload` as a hex `String` (matching the `const_hex` serde annotation already on `WasmResponse.payload`) to avoid a JSON array-of-numbers in the Tauri IPC payload. Frontend receives a hex string and decodes via the existing `hexToBytes` helper in `types/index.ts`.

### Feature 2: Smart result decoding (TypeScript)

**Algorithm (zero dependencies):**
```typescript
// Proposed location: app/src/utils/decodeResult.ts
// Reuses hexToBytes / bytesToHex from app/src/types/index.ts (extract to utils/bytes.ts first)

export function decodeResultPayload(hex: string): string {
  const bytes = hexToBytes(hex);

  // 1. Try valid UTF-8
  let text: string;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    // Not valid UTF-8 -- return hex
    return '0x' + bytesToHex(bytes);
  }

  // 2. Try JSON pretty-print
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    // Valid UTF-8 but not JSON -- return as plain string
    return text;
  }
}
```

`TextDecoder` with `{ fatal: true }` throws on invalid UTF-8 sequences rather than replacing with U+FFFD, enabling clean fallback to hex. This is available in all modern WebViews including Tauri's WebKit/WebView2.

**Usage:** Call in `GroupedActivityCard.tsx` when rendering the submission child card's result summary. The `hexToBytes` and `bytesToHex` helpers currently in `app/src/types/index.ts` should be moved to `app/src/utils/bytes.ts` to be shared between the types module and the new decode utility.

### Feature 3: Service restart race fix (Rust)

**The problem (confirmed by code inspection):**

`TriggerCommand::WatchEvmContractEvents` (trigger.rs ~line 577) looks up `evm_controllers.read().get(&chain)` and calls `enable_logs`. This works when the chain is `Connected`. But `TriggerManager::add_service` sends `StartListeningChain` followed immediately by `WatchEvmContractEvents` via the same `UnboundedSender` — both messages are queued synchronously. In `start_watcher`, `StartListeningChain` sets state to `Connecting` and then `await`s the EVM stream connection. The next iteration of the loop picks up `WatchEvmContractEvents` — but `evm_controllers` has no entry yet because the stream is still connecting. The controller insert only happens after the `await` resolves. Result: the log filter subscription is dropped with "No EVM controller found for chain" and never retried.

**Fix approach (no new crates):**

Add a pending subscriptions buffer local to `start_watcher`:

```rust
// In start_watcher, alongside listening_chain_states:
let mut pending_log_subs: HashMap<ChainKey, Vec<(Vec<Address>, Vec<B256>)>> = HashMap::new();
```

In the `WatchEvmContractEvents` arm:
```rust
// If not yet Connected, buffer for later
if listening_chain_states.get(&chain) != Some(&StreamStartState::Connected) {
    pending_log_subs.entry(chain).or_default().push((addresses, event_hashes));
    continue;
}
// Otherwise apply immediately via controller
```

In the `Connected` transition path (after inserting into `evm_controllers`):
```rust
// Drain pending log subscriptions for this chain
if let Some(pending) = pending_log_subs.remove(&chain) {
    for (addrs, hashes) in pending {
        controller.subscriptions.enable_logs(addrs, hashes);
    }
}
```

Same pattern applies to `WatchEvmBlocks` buffering if needed.

**Change surfaces:**
- `packages/wavs/src/subsystems/trigger.rs` — `start_watcher` loop only. No API changes, no new public types.

### Feature 4: Wallet settings kebab menu (React)

**Existing capability:** `DropdownMenu` atom (`app/src/components/atoms/DropdownMenu.tsx`) already has:
- Click-outside close via `useRef` + `addEventListener`
- `danger` variant styling (red text)
- Array of `MenuOption` items with `label`, `onClick`, `variant`

**Recommended change:** Add `iconTrigger?: boolean` prop to `DropdownMenu`. When true, the button renders `⋮` (U+22EE VERTICAL ELLIPSIS) instead of `{label} {arrow}`. No new component file needed.

```tsx
// In DropdownMenu.tsx, change button content:
{iconTrigger
  ? <span className="text-tan-muted text-base leading-none px-1">⋮</span>
  : <>{label} {isOpen ? '▲' : '▼'}</>
}
```

**Usage in WalletSection.tsx:** Replace the standalone `Button` rows for `Export Recovery Phrase` and `Reset Wallet` with a single `DropdownMenu iconTrigger` in the section header. Existing state (`showMnemonic`, `showResetConfirm`) drives the same handlers — kebab just triggers those setters.

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `@radix-ui/react-dropdown-menu` or Headless UI | Zero new capability vs existing DropdownMenu atom; adds 40+ kB and new import patterns | Extend DropdownMenu with `iconTrigger` prop |
| `iconv-lite` or `encoding` npm packages | TextDecoder (WHATWG Encoding API) is built into all modern WebViews including Tauri | `new TextDecoder('utf-8', { fatal: true })` |
| `hex-to-bytes` or `@noble/hashes` npm packages | `hexToBytes` and `bytesToHex` already exist in `app/src/types/index.ts` | Extract to `app/src/utils/bytes.ts` and reuse |
| `tokio::sync::Mutex` for pending log queue | The trigger watcher is single-threaded (one async task owns the loop) | Plain `HashMap<ChainKey, Vec<...>>` local variable |
| New Rust channel or new TriggerCommand variant | The existing `UnboundedSender<TriggerCommand>` is sufficient; pending subscriptions are watcher-local state | Local HashMap in `start_watcher` |
| Result decoding on the Rust side | Frontend already receives byte payloads; decoding is display logic that belongs in UI | `decodeResultPayload` utility in TypeScript |

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `Option<String>` for tx_hash in SubmissionEvent | Always-present String with empty sentinel `""` | Never — `None` is semantically correct for services with `Submit::None` that never go on-chain |
| Hex string for result_payload in SubmissionEvent | `Vec<u8>` serialized as JSON number array | Number array is acceptable if payload size matters; hex string is more readable in DevTools and consistent with WasmResponse's existing `const_hex` serde |
| Local `HashMap` pending queue in `start_watcher` | New `TriggerCommand::DeferredWatchEvmContractEvents` variant | Acceptable but adds API surface; local state is zero-API-change and easier to reason about |
| `iconTrigger` prop on existing `DropdownMenu` | New `KebabMenu` wrapper component | New component if `KebabMenu` needs substantially different styling (different border, size, positioning) |

---

## Version Compatibility

All changes are internal struct/type extensions — no version upgrades or new packages. Additive field additions to `SubmissionEvent` are backward-compatible with any consumers that don't yet read those fields.

| Package | Version | Notes |
|---------|---------|-------|
| `@tauri-apps/api` | ^2.10.1 | SubmissionEvent shape change is additive — existing listeners that don't destructure new fields are unaffected |
| `zustand` | ^5.0.0 | ActivityItem type extension is additive |
| `react` | ^19.1.0 | No new hooks patterns beyond what's already used |
| `wavs_gui_shared` | internal | SubmissionEvent field addition — all consumers (dispatcher.rs, app listeners.ts) updated together |

---

## Sources

- Direct inspection of `/workspace/packages/gui/shared/src/event.rs` — SubmissionEvent current fields (HIGH confidence)
- Direct inspection of `/workspace/packages/wavs/src/subsystems/aggregator.rs` lines 628–694 — tx_hash availability and SubmissionConfirmed construction path (HIGH confidence)
- Direct inspection of `/workspace/packages/wavs/src/dispatcher.rs` lines 118–143 and 462–480 — DispatcherCommand variants and emit_ext callsite (HIGH confidence)
- Direct inspection of `/workspace/packages/types/src/service.rs` lines 657–666 — WasmResponse.payload field type and serde annotation (HIGH confidence)
- Direct inspection of `/workspace/packages/wavs/src/subsystems/trigger.rs` lines 421–594 — StreamStartState machine and WatchEvmContractEvents ordering gap (HIGH confidence)
- Direct inspection of `/workspace/app/src/components/atoms/DropdownMenu.tsx` — existing kebab-compatible primitive (HIGH confidence)
- Direct inspection of `/workspace/app/src/types/index.ts` — hexToBytes/bytesToHex helpers and ActivityItem shape (HIGH confidence)
- WHATWG Encoding API specification — TextDecoder `fatal` mode available in all Chromium/WebKit/Gecko since 2015, present in Tauri WebViews (HIGH confidence)

---
*Stack research for: WAVS v1.3 — activity UX, result decoding, restart fix, kebab menu*
*Researched: 2026-04-09*
