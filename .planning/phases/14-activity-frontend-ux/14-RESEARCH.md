# Phase 14: Activity Frontend UX - Research

**Researched:** 2026-04-09
**Domain:** React/TypeScript frontend component modification, hex decoding, clipboard API
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Card Layout**
- Submission info (tx_hash, result) appears inline below trigger details as additional DetailRow entries
- A subtle "─── submission ───" divider separates trigger data from submission data
- Consistent with existing DetailRow pattern in ActivityCard.tsx
- Only shown on submission cards (kind === 'submission'), not trigger or failed cards

**Tx Hash Display**
- Truncated display: first 6 + last 4 chars (e.g., 0xdead...beef)
- Clipboard icon (📋) next to hash — click copies full hash
- Tooltip shows full hash on hover
- No block explorer links (ACT-05 deferred to future requirements)

**Result Decoding & Presentation**
- Decode chain: hex string → bytes → UTF-8 attempt → JSON parse attempt → fallback to hex
- Inline preview with format indicator badge: [JSON], [Text], or [Hex]
- JSON results: show pretty-printed, max 3 lines inline, overflow hidden
- UTF-8 text results: show as plain text
- Hex fallback: show truncated hex string with byte count
- Decode utility function lives in a new `decodeResultPayload` helper

**Virtualizer Height**
- Bump ESTIMATED_ITEM_HEIGHT from 90 to 130 to account for taller submission cards
- Submit cards will be ~140-160px with submission rows
- Trigger cards unchanged at ~90-100px
- 130 is a reasonable average across card types

### Claude's Discretion
- Exact Tailwind classes for the submission divider styling
- Copy-to-clipboard implementation (navigator.clipboard vs fallback)
- JSON syntax coloring approach (simple class-based or a lightweight formatter)
- Whether to show "No result" or hide the result row when result_payload is null

### Deferred Ideas (OUT OF SCOPE)
- ACT-05: Block explorer links for tx hashes (future requirement)
- ACT-06: Copy-to-clipboard affordance for tx hash and result data (partially addressed — copy for tx hash included, but dedicated copy for result deferred)
- ACT-07: ABI-decode calldata for known contract interfaces
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ACT-03 | Activity cards show submission info (status, tx hash, result) inline without requiring expand | SubmissionRows sub-component renders inside ActivityCard and GroupedActivityCard; always visible (not behind Raw toggle) |
| ACT-04 | Result payloads decode intelligently: hex string to UTF-8 to JSON pretty-print to hex fallback | `decodeResultPayload` utility covers all three cases using TextDecoder fatal mode and JSON.parse with try/catch |
</phase_requirements>

---

## Summary

Phase 14 is a pure frontend modification phase — no Rust, no Tauri commands, no new events. All data is already available in the ActivityItem type (`txHash?: string`, `resultPayload?: string | null`) and forwarded through listeners.ts in Phase 13. This phase adds the UI layer to display those fields inline.

The work consists of three isolated deliverables: (1) a `decodeResultPayload` utility that implements the hex→UTF-8→JSON→hex-fallback chain, (2) a `SubmissionRows` sub-component (with `TxHashDisplay` and `ResultPreview` atoms) that renders the new inline fields, and (3) two integration points — adding `SubmissionRows` to `ActivityCard.tsx` and `GroupedActivityCard.tsx`, plus bumping `ESTIMATED_ITEM_HEIGHT` in `ActivityFeed.tsx`.

The codebase has directly reusable precedents for every pattern this phase needs: `AddressDisplay.tsx` demonstrates the exact copy-to-clipboard pattern with 1500ms feedback, `ServiceDetailPage.tsx` demonstrates the hex→UTF-8→JSON decode chain using `TextDecoder('utf-8', { fatal: true })`, and `DetailRow` in `ActivityCard.tsx` provides the label/value layout atom.

**Primary recommendation:** Implement `decodeResultPayload` as a pure utility first, integrate `SubmissionRows` into both card components, then update the virtualizer constant.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19 (project) | Component rendering, useState for copy feedback | Already in use |
| TypeScript | project standard | Typed utility return, component props | Already in use |
| clsx | project standard | Conditional Tailwind class composition | Already in use in ActivityCard.tsx |
| Tailwind CSS | project standard | All styling — no CSS modules | Already in use, token system defined |

[VERIFIED: codebase grep — all four are used in ActivityCard.tsx and ActivityFeed.tsx]

### No New Dependencies
This phase adds no new npm packages. All required APIs are native:
- `TextDecoder` — Web API, available in all modern browsers and Tauri's WebView [VERIFIED: ServiceDetailPage.tsx line 48 uses it already]
- `navigator.clipboard.writeText` — Web API, available in Tauri WebView context [VERIFIED: listeners.ts line 122, WalletSection.tsx line 179, AddressDisplay.tsx line 23 all use it]
- `JSON.parse` / `JSON.stringify` — built-in [ASSUMED]

**Installation:** None required.

---

## Architecture Patterns

### Recommended Project Structure
```
app/src/
├── utils/
│   └── decodeResultPayload.ts   # NEW — pure decode utility
├── components/activity/
│   ├── ActivityCard.tsx         # MODIFY — add SubmissionRows
│   ├── GroupedActivityCard.tsx  # MODIFY — add SubmissionRows to child card
│   └── ActivityFeed.tsx         # MODIFY — ESTIMATED_ITEM_HEIGHT 90 → 130
```

### Pattern 1: Pure Utility Function for Decoding

The decode chain mirrors existing logic in `ServiceDetailPage.tsx` (lines 65–91). The new utility crystallizes this pattern into a reusable, typed form.

```typescript
// app/src/utils/decodeResultPayload.ts
// Source: mirrors ServiceDetailPage.tsx FileContentModal pattern [VERIFIED: codebase]

export type DecodeResult =
  | { kind: 'json'; display: string; truncated: boolean }
  | { kind: 'text'; display: string; truncated: boolean }
  | { kind: 'hex'; display: string; truncated: boolean };

export function decodeResultPayload(resultPayload: string | null | undefined): DecodeResult {
  if (!resultPayload) {
    return { kind: 'hex', display: '—', truncated: false };
  }

  // Step 1: hex string → bytes
  const clean = resultPayload.replace(/^0x/i, '');
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.substring(i * 2, i * 2 + 2), 16);
  }

  // Step 2: attempt UTF-8 decode (fatal: true rejects malformed sequences)
  try {
    const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);

    // Step 3: attempt JSON parse
    try {
      const parsed = JSON.parse(text);
      const pretty = JSON.stringify(parsed, null, 2);
      return { kind: 'json', display: pretty, truncated: false };
    } catch {
      return { kind: 'text', display: text, truncated: false };
    }
  } catch {
    // Step 4: hex fallback — truncate to first 40 chars + byte count
    const hexStr = clean.slice(0, 40);
    const truncated = clean.length > 40;
    return {
      kind: 'hex',
      display: truncated ? `${hexStr}… (${bytes.length} bytes)` : hexStr,
      truncated,
    };
  }
}
```

### Pattern 2: TxHashDisplay Inline Component

Follows the exact pattern in `AddressDisplay.tsx` [VERIFIED: codebase]:
- `useState(false)` for `copied`
- `navigator.clipboard.writeText(hash).catch(legacyFallback)`
- `setTimeout(() => setCopied(false), 1500)` reset
- `title={hash}` for native browser tooltip (no custom overlay)

Key difference from AddressDisplay: this is a smaller inline component inside a DetailRow value slot, not a standalone address chip. Clipboard icon uses the unicode glyph 📋 (per UI-SPEC) rather than SVG, and the `font-mono text-xs` size matches the DetailRow value style.

```typescript
// Inline within ActivityCard.tsx or extracted to sub-component
// Source: AddressDisplay.tsx pattern [VERIFIED: codebase]

function TxHashDisplay({ hash }: { hash: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(hash);
    } catch {
      // legacy fallback (document.execCommand) if needed
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const truncated = `${hash.slice(0, 6)}…${hash.slice(-4)}`;

  return (
    <span className="inline-flex items-center gap-1 font-mono text-xs text-beige-warm">
      <span title={hash}>{truncated}</span>
      <button
        type="button"
        onClick={handleCopy}
        className="ml-1 text-tan-muted hover:text-beige-warm cursor-pointer text-[11px]"
      >
        {copied ? 'Copied!' : '📋'}
      </button>
    </span>
  );
}
```

### Pattern 3: ResultPreview Inline Component

Badge-first layout using format indicator, then content. Three rendering branches based on `DecodeResult.kind`. The JSON branch uses `max-h-[3.6em] overflow-hidden` for 3-line capping (3 lines × 1.2em line-height = 3.6em).

```typescript
// Inline within ActivityCard.tsx
// Source: UI-SPEC.md component inventory [VERIFIED: codebase]

function ResultPreview({ payload }: { payload: string | null | undefined }) {
  const result = decodeResultPayload(payload);

  if (!payload) return null; // hide row entirely per UI-SPEC interaction contract

  const badgeClass = result.kind === 'json'
    ? 'bg-primary-600/20 text-primary-500'
    : result.kind === 'text'
      ? 'bg-charcoal-medium text-tan-warm'
      : 'bg-charcoal-light text-tan-muted';

  return (
    <span className="inline-flex items-start gap-1 min-w-0">
      <span className={clsx('shrink-0 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wide', badgeClass)}>
        {result.kind === 'json' ? 'JSON' : result.kind === 'text' ? 'Text' : 'Hex'}
      </span>
      {result.kind === 'json' ? (
        <pre className="whitespace-pre-wrap font-mono text-xs text-beige-warm/90 max-h-[3.6em] overflow-hidden">
          {result.display}
        </pre>
      ) : result.kind === 'text' ? (
        <span className="font-mono text-xs text-beige-warm break-all">{result.display}</span>
      ) : (
        <span className="font-mono text-xs text-tan-muted">{result.display}</span>
      )}
    </span>
  );
}
```

### Pattern 4: SubmissionRows Sub-Component

Renders after trigger `DetailRows`. Guard condition: only show when `item.kind === 'submission'` AND at least one of `txHash` or `resultPayload` is present (non-null, non-empty). Uses the submission divider markup verbatim from UI-SPEC.

```typescript
// Within ActivityCard.tsx, added after DetailRows render
function SubmissionRows({ txHash, resultPayload }: {
  txHash?: string;
  resultPayload?: string | null;
}) {
  if (!txHash && !resultPayload) return null;

  return (
    <>
      {/* Submission divider — from UI-SPEC.md verbatim */}
      <div className="relative my-2">
        <div className="border-t border-charcoal-light" />
        <span className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 top-0 bg-charcoal-dark px-2 text-[10px] text-tan-muted tracking-widest">
          submission
        </span>
      </div>
      <div className="flex flex-col gap-1">
        {txHash && <DetailRow label="tx" value={<TxHashDisplay hash={txHash} />} />}
        {resultPayload && <DetailRow label="result" value={<ResultPreview payload={resultPayload} />} />}
      </div>
    </>
  );
}
```

### Pattern 5: GroupedActivityCard Integration

The submission child card (`group.submission`) is only rendered when `group.status === 'complete'` (implicitly, when `group.submission` exists). `SubmissionRows` is added after the error text, before the Raw toggle button — same position as the spec requires.

Note: `GroupedActivityCard` does NOT already import `useState` from React for the copy feedback — but `TxHashDisplay` is a self-contained component that manages its own state, so no changes to `GroupedActivityCard`'s hook usage are needed.

### Pattern 6: Virtualizer Height Update

Single constant change in `ActivityFeed.tsx`:
```typescript
// Before:
const ESTIMATED_ITEM_HEIGHT = 90;
// After:
const ESTIMATED_ITEM_HEIGHT = 130;
```

The virtualizer uses `ref={virtualizer.measureElement}` for actual measurement, so this is only an estimate affecting initial render. Cards will self-report their true size. [VERIFIED: ActivityFeed.tsx line 14, line 333]

### Anti-Patterns to Avoid

- **Mutating DetailRow's span wrapper:** `DetailRow` uses `<span className="text-beige-warm font-mono break-all">` as the value wrapper. Nesting a `<pre>` inside a `<span>` is technically invalid HTML. The `ResultPreview` component should be structured so the `<pre>` is the direct value prop, and `DetailRow`'s `value` prop is `React.ReactNode` (already typed that way) — but `break-all` on the span wrapper may conflict with `whitespace-pre-wrap` on the inner `<pre>`. Consider rendering the result row differently: either override the wrapper or not use `DetailRow` for the result row, to avoid CSS conflicts.
- **Re-rendering cost:** `decodeResultPayload` is called on every render of cards containing it. For large lists with many submission items this could be a minor perf concern. Consider `useMemo` wrapping or memoizing at the component level if profiling shows issues. For the expected event volume (< 1000 items in a feed), this is not a concern.
- **Hex strings with odd length:** If `result_payload` is malformed hex (odd number of chars), `parseInt` in the decode loop will read partial bytes. The `clean.length / 2` calculation should use `Math.floor` and the loop bound should be `Math.floor(clean.length / 2)` to avoid `NaN` bytes.
- **`break-all` on DetailRow value vs JSON pre-wrap:** The existing `DetailRow` value span has `break-all` which will fight `whitespace-pre-wrap` on the inner pre. The `ResultPreview` component should be self-contained without relying on the parent span's text-wrap behavior — use `overflow-hidden` on the pre to clip instead.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UTF-8 decoding with error detection | Custom byte-to-char loop | `TextDecoder('utf-8', { fatal: true })` | Handles multi-byte sequences, surrogate pairs, BOM correctly; fatal mode throws on invalid sequences |
| Copy to clipboard | `document.execCommand('copy')` as primary | `navigator.clipboard.writeText()` primary | Modern async API; execCommand is deprecated but remains as fallback |
| Address/hash truncation | Custom truncate component | Inline slice pattern (already used in AddressDisplay.tsx) | No library needed — trivial operation |

**Key insight:** The codebase already has all required patterns implemented. `ServiceDetailPage.tsx` has the decode chain, `AddressDisplay.tsx` has the copy-to-clipboard pattern. This phase replicates and composes them in new components.

---

## Common Pitfalls

### Pitfall 1: DetailRow `break-all` vs `whitespace-pre-wrap` Conflict

**What goes wrong:** The existing `DetailRow` value span has class `break-all`. If `ResultPreview` renders a `<pre className="whitespace-pre-wrap">` as the value, the outer `break-all` and inner `whitespace-pre-wrap` fight each other in CSS — the `pre` will attempt to preserve whitespace but `break-all` on the parent forces word breaks that disrupt indented JSON.

**Why it happens:** `DetailRow` was designed for short monospace strings (addresses, block numbers). It was not designed for multi-line pre-formatted content.

**How to avoid:** Either (a) do not wrap the result row in `DetailRow` — instead render it as a custom row with `flex gap-3 text-xs` matching the `DetailRow` structure but without `break-all` on the value span, or (b) apply `whitespace-pre-wrap break-all` together and accept that JSON indentation may break. Option (a) is cleaner.

**Warning signs:** JSON output looks like a single long line rather than indented.

### Pitfall 2: `TxHashDisplay` `e.stopPropagation()` in GroupedActivityCard

**What goes wrong:** `GroupedActivityCard`'s header row uses `onClick={onToggleExpand}`. The submission child card is inside the `expanded` block, so card-level click propagation is less of an issue there. However, if `TxHashDisplay` is used anywhere near a click-propagating parent, the clipboard button click will bubble up and toggle the expand state.

**Why it happens:** Event bubbling in React — the clipboard button click propagates to parent divs.

**How to avoid:** `TxHashDisplay`'s button `onClick` must call `e.stopPropagation()` before clipboard write. [VERIFIED: AddressDisplay.tsx line 22 does this already]

**Warning signs:** Clicking the clipboard icon causes the card to expand/collapse unexpectedly.

### Pitfall 3: Odd-Length Hex Strings in Decode

**What goes wrong:** If `result_payload` comes in as an odd-length hex string (e.g., `"abc"`), `parseInt` in the hex→bytes loop produces `NaN` for the partial last byte, which becomes `0` in the Uint8Array, causing silent corruption.

**Why it happens:** Malformed hex from the backend, or a payload that was already a string and got hex-encoded then partially truncated.

**How to avoid:** In `decodeResultPayload`, use `Math.floor(clean.length / 2)` as the byte array length and loop bound. This silently drops the trailing nibble rather than producing NaN bytes.

**Warning signs:** UTF-8 decode succeeds but produces garbled text or an extra null character at the end.

### Pitfall 4: `resultPayload` as Empty String vs Null

**What goes wrong:** The `ActivityItem.resultPayload` field is typed as `string | null` (from the type definition). However, the backend may send an empty string `""` rather than `null` when there's no result. The guard `if (!resultPayload)` handles both null and empty string, but the decode function must also handle empty string before attempting the hex→bytes conversion (an empty hex string produces a zero-length Uint8Array, which TextDecoder decodes to `""`, then JSON.parse(`""`) throws, resulting in `{ kind: 'text', display: '' }` rather than hiding the row).

**Why it happens:** Rust's `Option<String>` serializes as `null` in JSON, but if the backend were to send `Some("")`, the frontend receives `""`.

**How to avoid:** Guard at the top of `decodeResultPayload` on `!resultPayload` (falsy check covers null, undefined, and ""). Also guard in `ResultPreview` before rendering: `if (!payload) return null`.

**Warning signs:** An empty result row appears on submission cards.

### Pitfall 5: `bg-charcoal-dark` vs `bg-charcoal-darkest` in Divider

**What goes wrong:** The submission divider's center label uses `bg-charcoal-dark px-2` to "cut through" the border line visually. If the card background is changed or the component is rendered inside `GroupedActivityCard`'s child card (which uses `bg-charcoal-darkest`), the label background won't match and the divider line will show through behind the text.

**Why it happens:** The divider uses a "knockout" technique — the text sits on a same-color background to appear as if the border line stops at the text. The color must match the parent container.

**How to avoid:** For `ActivityCard.tsx` (bg-charcoal-dark): use `bg-charcoal-dark` on the span. For `GroupedActivityCard.tsx` child card (bg-charcoal-darkest): use `bg-charcoal-darkest` on the span. Either pass the background color as a prop to `SubmissionRows`, or define two variants, or make `SubmissionRows` accept a `bgColor` prop defaulting to `'charcoal-dark'`.

**Warning signs:** The divider label has a visible background that doesn't match the card.

---

## Code Examples

Verified patterns from existing codebase:

### Clipboard Copy with 1500ms Feedback (from AddressDisplay.tsx)
```typescript
// Source: app/src/components/atoms/AddressDisplay.tsx lines 19-26 [VERIFIED: codebase]
const [copied, setCopied] = useState(false);

const handleCopy = async (e: React.MouseEvent) => {
  e.stopPropagation();
  await navigator.clipboard.writeText(address);
  setCopied(true);
  setTimeout(() => setCopied(false), 1500);
};
```

### TextDecoder UTF-8 with Fatal Mode (from ServiceDetailPage.tsx)
```typescript
// Source: app/src/pages/services/ServiceDetailPage.tsx lines 65-91 [VERIFIED: codebase]
try {
  display = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  try {
    display = JSON.stringify(JSON.parse(display), null, 2);
  } catch {
    // leave as plain text
  }
} catch {
  display = Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}
```

### DetailRow Pattern (from ActivityCard.tsx)
```typescript
// Source: app/src/components/activity/ActivityCard.tsx lines 41-48 [VERIFIED: codebase]
export function DetailRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex gap-3 text-xs">
      <span className="text-tan-muted w-20 shrink-0">{label}</span>
      <span className="text-beige-warm font-mono break-all">{value}</span>
    </div>
  );
}
```

### Virtualizer measureElement Pattern (from ActivityFeed.tsx)
```typescript
// Source: app/src/components/activity/ActivityFeed.tsx lines 329-334 [VERIFIED: codebase]
<div
  data-index={virtualItem.index}
  ref={virtualizer.measureElement}
  // ...
>
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Show result only in Raw expand section | Show inline in card without expand | Phase 14 | Satisfies ACT-03 |
| ESTIMATED_ITEM_HEIGHT = 90 | ESTIMATED_ITEM_HEIGHT = 130 | Phase 14 | Better initial virtualizer sizing for taller submission cards |
| No result decoding | Hex → UTF-8 → JSON → hex fallback | Phase 14 | Satisfies ACT-04 |

---

## Integration Verification Checklist

These are the exact integration points the planner must create tasks for:

| File | Change | Guard Condition |
|------|--------|-----------------|
| `app/src/utils/decodeResultPayload.ts` | Create new utility | None — pure function |
| `app/src/components/activity/ActivityCard.tsx` | Add `SubmissionRows` render after `DetailRows` | `item.kind === 'submission'` |
| `app/src/components/activity/GroupedActivityCard.tsx` | Add `SubmissionRows` inside child card block after error text | `group.submission` exists AND `group.submission.kind === 'submission'` |
| `app/src/components/activity/ActivityFeed.tsx` | Change `ESTIMATED_ITEM_HEIGHT` from `90` to `130` | None |

**Existing data flow confirmed:**
- `txHash` and `resultPayload` are already on `ActivityItem` type [VERIFIED: types/index.ts lines 341-343]
- `listeners.ts` already forwards both fields from `SubmissionEvent` to `addActivity` [VERIFIED: listeners.ts lines 69-72]
- The `SubmissionEvent` interface already declares both fields [VERIFIED: types/index.ts lines 108-115]

No new Tauri commands, no store changes, no new events needed.

---

## Environment Availability

Step 2.6: SKIPPED — no external dependencies. This phase is pure frontend code changes using only native Web APIs and existing project dependencies.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `JSON.parse` and `JSON.stringify` with `null, 2` for pretty-printing are available and behave as expected | Architecture Patterns | Low — universal JS built-in |
| A2 | Tauri's WebView supports `navigator.clipboard.writeText` without additional permissions | Don't Hand-Roll | Medium — if clipboard is blocked in Tauri, the copy button silently fails; the .catch() fallback handles this |
| A3 | `TextDecoder` with `{ fatal: true }` correctly rejects binary-looking hex that happens to decode as valid UTF-8 | Architecture Patterns | Low — spec-compliant behavior; real binary data is statistically unlikely to be valid UTF-8 |
| A4 | Phase 13 has forwarded `tx_hash` and `result_payload` through the full Rust → Tauri → listeners.ts pipeline | Integration | HIGH if wrong — if Phase 13 incomplete, this phase has no data to display. The type definitions confirm the TypeScript side is ready, but the Rust side depends on Phase 13 delivery. |

---

## Open Questions

1. **Phase 13 Completion Status**
   - What we know: `ActivityItem.txHash` and `ActivityItem.resultPayload` are typed in `types/index.ts`; `listeners.ts` already reads `payload.tx_hash` and `payload.result_payload` from `SubmissionEvent`
   - What's unclear: Whether Phase 13 Rust backend changes have been committed — `git status` shows Phase 13 plan/summary files as staged deletions on the `better-mcp` branch, suggesting the branch diverged before Phase 13 execution
   - Recommendation: Phase 14 plan should note this dependency explicitly. If Phase 13 is incomplete, the UI will render correctly but show no data (txHash will be undefined, resultPayload will be undefined — both guard conditions handle this gracefully with no visible output)

2. **`DetailRow` Break-All vs Pre Conflict**
   - What we know: `DetailRow` wraps its value in `<span className="text-beige-warm font-mono break-all">` — this wraps the `ResultPreview` component
   - What's unclear: Whether using a custom row structure vs reusing `DetailRow` is the right call
   - Recommendation: Use `DetailRow` for the tx row (TxHashDisplay is an inline-flex span that is fine in break-all context). For the result row, define a custom row without `break-all` to avoid CSS conflict with the inner `pre` element.

---

## Sources

### Primary (HIGH confidence)
- `app/src/components/activity/ActivityCard.tsx` — DetailRow pattern, DetailRows component, existing card structure [VERIFIED: codebase]
- `app/src/components/activity/ActivityFeed.tsx` — ESTIMATED_ITEM_HEIGHT constant, virtualizer pattern [VERIFIED: codebase]
- `app/src/components/activity/GroupedActivityCard.tsx` — child card structure, integration point [VERIFIED: codebase]
- `app/src/components/atoms/AddressDisplay.tsx` — canonical clipboard copy pattern with 1500ms feedback [VERIFIED: codebase]
- `app/src/pages/services/ServiceDetailPage.tsx` lines 65–91 — canonical TextDecoder UTF-8 + JSON decode chain [VERIFIED: codebase]
- `app/src/types/index.ts` — ActivityItem type with txHash/resultPayload, SubmissionEvent type [VERIFIED: codebase]
- `app/src/tauri/listeners.ts` — confirms tx_hash and result_payload forwarded in submission handler [VERIFIED: codebase]
- `app/tailwind.config.js` — all color tokens confirmed present (charcoal-dark, primary-600, primary-500, tan-muted, etc.) [VERIFIED: codebase]
- `.planning/phases/14-activity-frontend-ux/14-CONTEXT.md` — locked decisions [VERIFIED: codebase]
- `.planning/phases/14-activity-frontend-ux/14-UI-SPEC.md` — component inventory, color contract, divider markup [VERIFIED: codebase]

### Secondary (MEDIUM confidence)
None — all claims verified directly in codebase.

### Tertiary (LOW confidence)
None — no unverified WebSearch claims.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all confirmed in codebase, no new deps
- Architecture: HIGH — all patterns have direct codebase precedents
- Pitfalls: HIGH — identified from direct code inspection of actual files

**Research date:** 2026-04-09
**Valid until:** 2026-05-09 (stable frontend stack)
