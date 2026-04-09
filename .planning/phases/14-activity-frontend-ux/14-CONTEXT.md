# Phase 14: Activity Frontend UX - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can see submission status, tx hash, and decoded result inline on activity cards without expanding. This phase modifies ActivityCard.tsx to render new submission fields (tx_hash, result_payload from Phase 13) and adds a result decoding utility (hex → UTF-8 → JSON → hex fallback).

</domain>

<decisions>
## Implementation Decisions

### Card Layout
- Submission info (tx_hash, result) appears inline below trigger details as additional DetailRow entries
- A subtle "─── submission ───" divider separates trigger data from submission data
- Consistent with existing DetailRow pattern in ActivityCard.tsx
- Only shown on submission cards (kind === 'submission'), not trigger or failed cards

### Tx Hash Display
- Truncated display: first 6 + last 4 chars (e.g., 0xdead...beef)
- Clipboard icon (📋) next to hash — click copies full hash
- Tooltip shows full hash on hover
- No block explorer links (ACT-05 deferred to future requirements)

### Result Decoding & Presentation
- Decode chain: hex string → bytes → UTF-8 attempt → JSON parse attempt → fallback to hex
- Inline preview with format indicator badge: [JSON], [Text], or [Hex]
- JSON results: show pretty-printed, max 3 lines inline, overflow hidden
- UTF-8 text results: show as plain text
- Hex fallback: show truncated hex string with byte count
- Decode utility function lives in a new `decodeResultPayload` helper

### Virtualizer Height
- Bump ESTIMATED_ITEM_HEIGHT from 90 to 130 to account for taller submission cards
- Submit cards will be ~140-160px with submission rows
- Trigger cards unchanged at ~90-100px
- 130 is a reasonable average across card types

### Claude's Discretion
- Exact Tailwind classes for the submission divider styling
- Copy-to-clipboard implementation (navigator.clipboard vs fallback)
- JSON syntax coloring approach (simple class-based or a lightweight formatter)
- Whether to show "No result" or hide the result row when result_payload is null

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ActivityCard.tsx` — DetailRow component for label/value display
- `DetailRows` component — trigger-type-specific detail rendering
- `getTriggerAccent` — color mapping by trigger type
- `formatTimestamp` — time formatting utility
- `app/src/types/index.ts` — ActivityItem already has txHash and resultPayload fields (from Phase 13)
- `app/src/tauri/listeners.ts` — already forwards tx_hash and result_payload to store

### Established Patterns
- Tailwind utility classes for all styling (no CSS modules)
- clsx for conditional classes
- DetailRow pattern: label (w-20 shrink-0 text-tan-muted) + value (text-beige-warm font-mono)
- Zustand stores via useAppStore hooks
- Virtual list in ActivityFeed.tsx with ESTIMATED_ITEM_HEIGHT constant

### Integration Points
- ActivityCard.tsx — main render component to modify
- ActivityFeed.tsx — virtualizer height estimate to update
- New utility function for result payload decoding

</code_context>

<specifics>
## Specific Ideas

- Submission divider should be subtle (same style as existing card separators)
- Copy button should show brief "Copied!" feedback
- JSON preview should use the same monospace font (font-mono) as the Raw expand section

</specifics>

<deferred>
## Deferred Ideas

- ACT-05: Block explorer links for tx hashes (future requirement)
- ACT-06: Copy-to-clipboard affordance for tx hash and result data (partially addressed — copy for tx hash included, but dedicated copy for result deferred)
- ACT-07: ABI-decode calldata for known contract interfaces

</deferred>
