# Feature Research

**Domain:** Activity UX improvements for WAVS Tauri desktop app (v1.3)
**Researched:** 2026-04-09
**Confidence:** HIGH (based on direct codebase inspection + established UI patterns)

## Scope

This research covers four feature areas for the v1.3 milestone:

1. Richer activity cards (trigger + result + submission visible without expanding)
2. Smart result decoding (byte vec → UTF-8 → JSON → hex)
3. Wallet settings kebab menu for uncommon actions
4. Service restart reliability fix (backend)

---

## Feature Landscape

### Table Stakes (Users Expect These)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Tx hash visible on submission cards without expanding | Developer tools universally show tx IDs inline; having to expand to find it is friction | LOW | `SubmissionConfirmed` currently does NOT carry `tx_hash` — backend gap: aggregator has it at `tx_resp.tx_hash()` but does not include it in `DispatcherCommand::SubmissionConfirmed` or the `SubmissionEvent` GUI struct. Requires Rust change to `DispatcherCommand`, `SubmissionEvent`, and `ActivityItem`. |
| Copy-to-clipboard on tx hash | Users always need to copy hashes to explorers/terminals | LOW | `AddressDisplay` atom already implements this pattern with hover-reveal copy icon + checkmark confirmation. Reuse that pattern; do NOT write new clipboard logic. |
| Result summary visible without expanding | If a component ran and produced output, that output should appear in the card summary row | MEDIUM | `SubmissionEvent` has `trigger_data` (what triggered it) but NO execution result/output payload. Same backend gap: the aggregator stores the execution result but does not forward it. Requires a new `result_bytes: Option<Vec<u8>>` (or base64 string) field through the event pipeline. |
| Non-destructive expand/collapse default | Cards should default to showing the most useful info without requiring clicks | LOW | Currently GroupedActivityCard shows trigger pill + status dot + timestamp by default. Submission is only visible after expand. This is the core issue to fix. |
| Status dot retains meaning | Pending/failed dots are already implemented and working | — | Already shipped. Do not remove or rework. |

### Differentiators (Competitive Advantage)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Smart result decoding (UTF-8 → JSON → hex) | Raw byte arrays are meaningless; auto-detecting and pretty-printing makes results immediately useful to developers without copy-paste to an external decoder | LOW | Pure frontend. Decode attempt order: (1) `TextDecoder('utf-8')` with `fatal: true`, (2) `JSON.parse` of the decoded string, (3) hex fallback via `Array.from(bytes).map(b => b.toString(16).padStart(2,'0')).join('')`. The decode function lives in a shared utility; `GroupedActivityCard` calls it on `result_bytes`. |
| Block explorer link on tx hash | One click to Etherscan/Mintscan instead of copy+paste | MEDIUM | Requires knowing the chain ID of the submission to construct the URL. Chain is available on `SubmissionEvent.workflow_id` → service config lookup. Complexity: chain-to-explorer URL mapping table; fallback to copy-only when chain unknown. Mark as differentiator, not table stakes — copy-only is acceptable for MVP. |
| Inline submission card always visible (no expand) | Saves one click per event; makes the feed scannable at a glance | LOW | Currently the submission child card lives inside the `{expanded && ...}` block of `GroupedActivityCard`. Move it outside that block to always render when `group.submission` is present. Keep "Raw" toggle for power users. |
| Kebab menu for wallet destructive actions | Moves "Reset Wallet" and "Export Recovery Phrase" out of the main WalletSection flow; reduces cognitive load and accidental clicks | LOW | `DropdownMenu` atom already exists with `variant: 'danger'` item support. `OwnerActionsMenu` uses it as a reference implementation. The wallet kebab replaces the current two full-width `Button` elements for those actions. Normal wallet info (address, balances) stays visible unconditionally. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| ABI-decode submission data | Developers want to see decoded EVM calldata/event logs | Requires ABI registry (which ABI for which contract?); no ABI source of truth in WAVS | Show raw hex with copy; let developer paste to Etherscan's decoder |
| Auto-expand all cards on activity updates | Show everything without clicking | Destroys readability when 100+ events arrive; virtualizer height estimation breaks with variable heights if all expanded | Keep expand-on-click; default to showing submission card summary without full raw expansion |
| Persistent expanded state across page navigations | Remember which cards were open | Expanded state is per-session in `ActivityFeed` (`expandedIds` Set) — persisting to storage adds complexity with minimal benefit since the feed is ephemeral debug output | Session-only expanded state is correct; do not persist |
| Animated transitions on card expand | Polished feel | Conflicts with `@tanstack/react-virtual` — row height must be stable/measurable; CSS height transitions give virtualizer incorrect measurements mid-animation | Static show/hide; instant render is correct for virtualizer |
| Real-time result streaming | Show partial results as component executes | WASM component execution is synchronous within Wasmtime; results are atomic | Show result only when complete; add pending indicator for in-flight executions |

---

## Feature Dependencies

```
[Richer activity cards — inline submission]
    └──requires──> [tx_hash forwarded from aggregator]
                       └──requires──> DispatcherCommand::SubmissionConfirmed gains tx_hash field
                                          └──requires──> SubmissionEvent gains tx_hash field
                                                             └──requires──> ActivityItem gains txHash field

[Smart result decoding]
    └──requires──> [result_bytes forwarded from aggregator]
                       └──requires──> Same pipeline as tx_hash above (parallel field addition)

[Block explorer link on tx hash]
    └──requires──> [tx_hash visible on submission cards]
    └──requires──> [chain known at submission time]

[Wallet kebab menu]
    └──uses──> [Existing DropdownMenu atom]
    └──replaces──> [Export Recovery Phrase button in WalletSection]
    └──replaces──> [Reset Wallet button in WalletSection]

[Service restart fix]
    ──independent──> [All activity UX features] (backend-only, no frontend dependency)
```

### Dependency Notes

- **tx_hash requires Rust pipeline changes:** `DispatcherCommand::SubmissionConfirmed` → `SubmissionEvent` (gui/shared) → Tauri `ActivityItem` (frontend type). Three files minimum.
- **result_bytes has same pipeline path:** Can be added in the same Rust PR as tx_hash to avoid double pipeline surgery.
- **Smart result decoding is pure frontend** but only becomes useful once `result_bytes` is forwarded; can be implemented against a stub/placeholder in the same phase.
- **Wallet kebab is fully self-contained:** Zero Rust changes, zero new atoms — just reorganize existing WalletSection UI.
- **Inline submission card** only requires moving JSX outside the `expanded` block in `GroupedActivityCard.tsx`. The submission data is already present in `group.submission`.

---

## MVP Definition

### Launch With (v1.3)

- [ ] Inline submission card — submission child card always visible when `group.submission` is present (move out of `expanded` block). LOW complexity, immediate UX win with zero backend changes.
- [ ] Smart result decoding utility — `decodeResultBytes(bytes: number[]): string` pure function; wire into submission card once `result_bytes` arrives.
- [ ] tx_hash forwarding — add `tx_hash: Option<String>` to `DispatcherCommand::SubmissionConfirmed`, `SubmissionEvent`, and frontend `ActivityItem`. Display as truncated hash with copy affordance in submission card.
- [ ] result_bytes forwarding — add alongside tx_hash in same pipeline pass. Display decoded in submission card.
- [ ] Wallet kebab menu — three-dot button in WalletSection header; moves Export and Reset into dropdown.
- [ ] Service restart fix — race condition in trigger stream re-subscription (backend; separate from UX features).

### Add After Validation (v1.x)

- [ ] Block explorer link on tx hash — add after tx_hash display works; requires chain-to-explorer mapping table. Trigger: user request or dogfooding feedback.
- [ ] Copy-all result button — copy decoded result text to clipboard directly from card (beyond the existing copy-hash pattern). Trigger: if result_bytes content is frequently long.

### Future Consideration (v2+)

- [ ] ABI-decode calldata — needs ABI registry feature first. Defer until contract interaction features mature.
- [ ] Grouped result history per service — aggregate result timelines; a different UX surface than the live feed.

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Inline submission card (always visible) | HIGH | LOW (JSX move) | P1 |
| tx_hash display with copy | HIGH | MEDIUM (Rust pipeline + frontend) | P1 |
| Smart result decoding | HIGH | LOW (pure function + wire-up) | P1 |
| result_bytes forwarding | HIGH | MEDIUM (same pipeline as tx_hash) | P1 |
| Wallet kebab menu | MEDIUM | LOW (DropdownMenu atom + JSX reorganize) | P1 |
| Service restart fix | HIGH (reliability) | MEDIUM (backend race condition) | P1 |
| Block explorer link | MEDIUM | MEDIUM (chain mapping table) | P2 |
| Copy-all result text | LOW | LOW | P3 |

---

## Implementation Notes by Feature

### 1. Inline Submission Card

Current: `{expanded && (<div>...group.submission...</div>)}` inside `GroupedActivityCard.tsx`

Target: Move submission child card outside the `expanded` block. Show it unconditionally when `group.submission` is defined. The "Raw" toggle inside the submission card stays gated on its own `childRawExpanded` state.

Impact: GroupedActivityCard height increases for complete events. The `ESTIMATED_ITEM_HEIGHT = 90` constant in `ActivityFeed.tsx` will need to increase (or switch to `useVirtualizer` dynamic measurement). This is the main risk: virtualizer height estimation. Use `overscan` generously or switch to `estimateSize` callback that returns ~150 when submission is present.

### 2. tx_hash Forwarding (Backend)

Rust change path:
1. `packages/wavs/src/dispatcher.rs`: Add `tx_hash: Option<String>` to `SubmissionConfirmed` variant.
2. `packages/wavs/src/subsystems/aggregator.rs`: Pass `tx_resp.tx_hash()` into the `SubmissionConfirmed` command.
3. `packages/gui/shared/src/event.rs`: Add `tx_hash: Option<String>` to `SubmissionEvent`.
4. `packages/wavs/src/dispatcher.rs` (handler): Pass through `tx_hash` when constructing `SubmissionEvent`.
5. Frontend `app/src/types/index.ts`: Add `txHash?: string` to `ActivityItem`.
6. Tauri event bridge: confirm the field passes through serde correctly (snake_case mapping).

Display: Truncate to `0x1234...abcd` format; use same copy-icon pattern as `AddressDisplay` atom.

### 3. result_bytes Forwarding (Backend)

Same pipeline as tx_hash. The execution result is the WASM component output. The execution result currently flows through `AggregatorExecuteKind::SubmitCallback { result: Result<AnyTxHash, String> }` — but that carries the on-chain tx hash, not the WASM output bytes. The WASM output is in the `Submission` struct in the aggregator. Validate the exact field name in `packages/types/src/` before assuming structure. This may require a slightly more involved Rust refactor if the execution output is not currently stored on `Submission`.

### 4. Smart Result Decoding

```typescript
// Pure utility function, no dependencies
export function decodeResultBytes(bytes: number[]): string {
  if (bytes.length === 0) return '(empty)';
  const u8 = new Uint8Array(bytes);
  try {
    const str = new TextDecoder('utf-8', { fatal: true }).decode(u8);
    try {
      return JSON.stringify(JSON.parse(str), null, 2); // pretty-print JSON
    } catch {
      return str; // valid UTF-8, not JSON
    }
  } catch {
    return '0x' + Array.from(u8).map(b => b.toString(16).padStart(2, '0')).join('');
  }
}
```

Edge cases: empty byte array → show "(empty)"; null/undefined → show nothing; extremely long strings → truncate to ~500 chars in summary row, show full in Raw toggle.

### 5. Wallet Kebab Menu

The `DropdownMenu` atom accepts `label`, `options[]`, and `size`. Currently it renders a text label button. For a kebab pattern, either:
- Extend `DropdownMenu` to accept `ReactNode` as label (preferred — keeps the atom DRY), or
- Create a thin `KebabMenu` wrapper that passes a three-dot SVG icon as the label.

Options for the menu:
```
- Export Recovery Phrase  (default variant) → triggers existing handleExportWallet flow
- Reset Wallet            (danger variant)  → triggers existing setShowResetConfirm(true) flow
```

The existing two-step confirmation UI and mnemonic display panels stay in WalletSection; only the entry-point buttons move into the kebab. This is purely a layout change.

Reference: `OwnerActionsMenu.tsx` shows the exact pattern — `DropdownMenu` with `variant: 'danger'` items opening inline workflows.

---

## Sources

- `/workspace/app/src/components/activity/GroupedActivityCard.tsx` — current expand/collapse structure; submission child in expanded-only block
- `/workspace/app/src/components/activity/ActivityFeed.tsx` — virtualizer with `ESTIMATED_ITEM_HEIGHT = 90`
- `/workspace/app/src/components/atoms/DropdownMenu.tsx` — existing dropdown atom API
- `/workspace/app/src/components/atoms/AddressDisplay.tsx` — existing copy-to-clipboard pattern
- `/workspace/app/src/components/settings/WalletSection.tsx` — current wallet UI with inline Export/Reset buttons
- `/workspace/app/src/components/poa/OwnerActionsMenu.tsx` — reference implementation of DropdownMenu with danger items
- `/workspace/packages/gui/shared/src/event.rs` — `SubmissionEvent` struct (no tx_hash or result_bytes today)
- `/workspace/packages/wavs/src/dispatcher.rs` — `DispatcherCommand::SubmissionConfirmed` (no tx_hash today)
- `/workspace/packages/wavs/src/subsystems/aggregator.rs` — `tx_resp.tx_hash()` exists and is logged but not forwarded
- `/workspace/app/src/types/index.ts` — `ActivityItem` shape and existing `TriggerData` type with tx_hash in `EvmContractEvent`
- `/workspace/app/src/hooks/useGroupedActivity.ts` — grouping and status logic

---

*Feature research for: WAVS Tauri desktop app — v1.3 Activity UX & Bug Fixes*
*Researched: 2026-04-09*
