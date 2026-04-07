# Phase 6: Unified Activity Frontend - Research

**Researched:** 2026-04-07
**Domain:** React component refactoring — grouped virtual list, status filtering, nested card UI
**Confidence:** HIGH

## Summary

Phase 6 is a pure frontend refactoring phase. All backend data infrastructure was completed in Phase 4: every `ActivityItem` already carries `correlationId` and `error` fields, and `submission_failed` is a first-class `ActivityKind`. This phase wires those fields into the UI by introducing a `GroupedActivityEvent` grouping model in `ActivityFeed.tsx` and a new `GroupedActivityCard` component.

The scope is tightly constrained by a detailed UI-SPEC and CONTEXT decisions. No new libraries are needed. The existing Tailwind keyframes (`animate-glow-amber`, `animate-glow-red`) and the full custom color palette are already declared in `tailwind.config.js`. The virtualizer (`@tanstack/react-virtual ^3.13.18`) stays in place with the only change being that its array switches from `ActivityItem[]` to `GroupedActivityEvent[]`.

The primary risk is virtualizer height measurement: switching from flat `ActivityItem[]` to grouped `GroupedActivityEvent[]` changes the average estimated height, and expanded grouped cards (parent + child) will be taller than the current `ESTIMATED_ITEM_HEIGHT = 90`. The `measureElement` ref pattern is already in use and handles dynamic heights correctly — no action needed beyond keeping it wired to the outer grouped card div.

**Primary recommendation:** Extract grouping logic into a `useGroupedActivity` hook, create `GroupedActivityCard` as a new file, and keep `ActivityCard` unchanged for orphan submissions.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Nesting & Grouping Model**
- Client-side grouping by correlationId — group ActivityItems with matching correlationId into a single card (trigger is parent, submission is child)
- Grouping logic lives in ActivityFeed.tsx via useMemo — derive grouped items from the flat activity list
- Standalone triggers (no submission yet) show as single card with pending indicator (pulsing amber dot next to kind badge)
- Orphan submissions (no matching trigger) show as standalone cards — handle gracefully

**Error & Status Display**
- Error badge: red dot badge next to kind pill on collapsed card — subtle but visible at a glance
- Error message: inline red text below submission details within the existing expand section
- Failed events are never auto-removed from the activity feed — successful events follow existing FIFO (2000 cap)
- Pending indicator: pulsing amber dot next to kind badge, same position as error dot

**Filtering Changes**
- Replace kind-filter tabs (trigger/submission) with status-based tabs: All / Pending / Failed / Complete
- "Failed" filter shows grouped events where the submission has failed (whole card visible)
- "Pending" filter shows grouped events where trigger has no matching submission yet
- Search unchanged — searches service name, workflow ID, trigger data label across grouped events

### Claude's Discretion
- Internal data structure for grouped events (interface shape) — specified in UI-SPEC
- Animation/transition details for expand/collapse
- Exact CSS for pulsing amber dot and red error dot
- Whether to add correlationId to search

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

The REQUIREMENTS.md file tracks Phase 1-3 requirements only. EVT-02 through EVT-05 and ERR-02 through ERR-04 are Phase 6 frontend requirements defined via ROADMAP success criteria. Mapping from ROADMAP success criteria:

| ID | Description | Research Support |
|----|-------------|------------------|
| EVT-02 | Trigger with completed submission appears as single expandable card; expanding reveals submission nested underneath | GroupedActivityEvent model + GroupedActivityCard component; correlationId already on all ActivityItems from Phase 4 |
| EVT-03 | Trigger whose submission has not yet arrived shows visible pending/in-flight indicator | `status: 'pending'` in GroupedActivityEvent; amber pulsing dot via existing `animate-glow-amber` keyframe |
| EVT-04 | Failed submission shows error badge on collapsed card and full error message when expanded | `status: 'failed'` in GroupedActivityEvent; red dot via `animate-glow-red`; `item.error` field already populated by Phase 4 |
| EVT-05 | Unified event model present on both standalone Activity page and per-service activity tab | Both surfaces render `ActivityFeed`; `ServiceActivity.tsx` passes serviceId — no surface-specific changes needed |
| ERR-02 | Failed events never automatically removed from activity feed | Guard in `appStore.addActivity` FIFO eviction: skip eviction for items where `kind === 'submission_failed'` |
| ERR-03 | Error badge visible on collapsed card for failed submissions | Red dot rendered in collapsed `GroupedActivityCard` header when `group.status === 'failed'` |
| ERR-04 | Full error message displayed in expanded view, no truncation | Inline `text-red-400` div in expanded child card with no `truncate` class |
</phase_requirements>

---

## Standard Stack

### Core (already installed — no new installs needed)
[VERIFIED: /workspace/app/package.json]

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | ^19.1.0 | Component model | Existing app framework |
| @tanstack/react-virtual | ^3.13.18 | Virtual scrolling | Already wired in ActivityFeed; keep unchanged |
| zustand | ^5.0.0 | State management | ActivityItem list lives in appStore |
| clsx | ^2.1.0 | Conditional className | Used throughout existing components |
| TypeScript | ~5.8.3 | Type safety | Existing project language |

### Design System (already declared — no new tokens)
[VERIFIED: /workspace/app/tailwind.config.js]

| Token | Value | Phase 6 Use |
|-------|-------|------------|
| `animate-glow-amber` | 2.5s ease-in-out infinite | Pending dot animation |
| `animate-glow-red` | 2.5s ease-in-out infinite | Failed dot animation |
| `bg-charcoal-darkest` (`#1E1E1E`) | Child card background | Distinguish from parent |
| `border-charcoal-light` (`#383232`) | Child card border | Lighter than parent accent |
| `bg-amber-400` | `#FBBF24` | Pending dot color |
| `bg-red-400` | `#F87171` | Failed dot color |
| `bg-purple-1` (`#4A345D`) | Active filter tab bg | Unchanged from current |

**Installation:** None required. All dependencies and tokens are already present.

---

## Architecture Patterns

### Recommended Project Structure

```
app/src/components/activity/
├── ActivityFeed.tsx          # MODIFY: grouping logic, status filter tabs, virtualizer array change
├── ActivityCard.tsx          # MODIFY (minor): used only for orphan submissions now
└── GroupedActivityCard.tsx   # CREATE: new component for grouped trigger+submission cards
```

No new directories needed.

### Pattern 1: GroupedActivityEvent Data Model

**What:** Derived in-memory structure computed via `useMemo` from the flat `activityList`. Never persisted. Single-pass Map construction over the flat array.

**When to use:** Only inside `ActivityFeed.tsx`. Not exported to consumers.

**Interface (from UI-SPEC — Claude's Discretion area):**

```typescript
// Source: 06-UI-SPEC.md Data Model Contract
interface GroupedActivityEvent {
  trigger: ActivityItem;
  submission?: ActivityItem;
  status: 'pending' | 'complete' | 'failed';
  groupKey: string; // correlationId if present, else String(trigger.id)
}
```

Orphan submissions (kind === 'submission' | 'submission_failed' with no matching trigger in the window) are stored separately as `ActivityItem[]` and rendered as flat `ActivityCard` at their natural position in the list.

### Pattern 2: Grouping Algorithm (single-pass useMemo)

```typescript
// Source: 06-UI-SPEC.md Grouping algorithm
const grouped = useMemo(() => {
  const byCorrelation = new Map<string, GroupedActivityEvent>();
  const orphanSubmissions: ActivityItem[] = [];

  for (const item of sourceList) {
    if (item.kind === 'trigger') {
      const key = item.correlationId ?? String(item.id);
      byCorrelation.set(key, {
        trigger: item,
        submission: undefined,
        status: 'pending',
        groupKey: key,
      });
    } else {
      // submission or submission_failed
      const corrId = item.correlationId;
      if (corrId && byCorrelation.has(corrId)) {
        const group = byCorrelation.get(corrId)!;
        group.submission = item;
        group.status = item.kind === 'submission_failed' ? 'failed' : 'complete';
      } else {
        orphanSubmissions.push(item);
      }
    }
  }

  return { groups: Array.from(byCorrelation.values()), orphans: orphanSubmissions };
}, [sourceList]);
```

The virtualizer receives a merged/sorted array that interleaves groups and orphans by timestamp position.

### Pattern 3: expandedIds keyed by groupKey (string)

**What:** Change from `Set<number>` (item id) to `Set<string>` (groupKey). Child raw JSON toggle is separate local state inside `GroupedActivityCard`.

**Key change in ActivityFeed.tsx:**

```typescript
// BEFORE
const [expandedIds, setExpandedIds] = useState<Set<number>>(() => new Set());
// toggleExpanded(item.id)

// AFTER
const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());
// toggleExpanded(group.groupKey)
```

### Pattern 4: Status-Based Filter Tabs

Replace the current `KindFilter` type and `kindFilter` state:

```typescript
// BEFORE
type KindFilter = 'all' | ActivityKind;
const [kindFilter, setKindFilter] = useState<KindFilter>('all');
// filter: items.filter(i => i.kind === kindFilter)

// AFTER
type StatusFilter = 'all' | 'pending' | 'failed' | 'complete';
const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
// filter applied to GroupedActivityEvent[] not ActivityItem[]
```

Tab DOM order: All | Pending | Failed | Complete (left to right per UI-SPEC).

### Pattern 5: Failed Event Eviction Guard (appStore.ts)

The FIFO 2000-cap eviction slice must skip `submission_failed` items:

```typescript
// Source: 06-CONTEXT.md — "Failed events are never auto-removed"
// In appStore.ts addActivity:
addActivity: (item) =>
  set((state) => {
    const next = [...state.activityList, item];
    if (next.length > MAX_ACTIVITY_ITEMS) {
      // Find oldest non-failed item to evict, not just slice from front
      // Simplest correct approach: filter out failed from eviction candidates
      const failedItems = next.filter(i => i.kind === 'submission_failed');
      const nonFailedItems = next.filter(i => i.kind !== 'submission_failed');
      const trimmed = nonFailedItems.length > MAX_ACTIVITY_ITEMS
        ? nonFailedItems.slice(nonFailedItems.length - MAX_ACTIVITY_ITEMS)
        : nonFailedItems;
      return { activityList: [...trimmed, ...failedItems].sort((a, b) => a.id - b.id) };
    }
    return { activityList: next };
  }),
```

Note: This is one valid approach. The planner may choose a simpler equivalent (e.g., slice from front but skip failed items). The invariant is: `submission_failed` items must never be removed by the eviction logic.

### Pattern 6: GroupedActivityCard Component Structure

New file `GroupedActivityCard.tsx`. Key structural decisions from UI-SPEC:

- Outer card: same `pl-3 pr-4 pt-3 pb-3 rounded-lg border border-l-4 bg-charcoal-dark` as `ActivityCard`, keyed by `groupKey`
- Status dot: `w-2 h-2 rounded-full` inline after kind pill, `gap-1.5` spacing, `aria-label` on the dot element
- Pending: `bg-amber-400 animate-glow-amber`
- Failed: `bg-red-400 animate-glow-red`
- Complete / no dot: element not rendered
- Click target for expand: entire header row div, not just "Raw ▼" button
- Child card: `ml-2 mt-2 border border-charcoal-light bg-charcoal-darkest rounded-md` — no `border-l-4` accent stripe
- Child raw JSON toggle: independent `useState<boolean>` local to `GroupedActivityCard`
- Error text in child: `text-red-400 text-xs mt-1` with full text, no `truncate`

### Anti-Patterns to Avoid

- **Do not add a pulse animation to the dot border** — `animate-glow-*` keyframes use `boxShadow`, not `border-color`. Apply to the dot element itself (`w-2 h-2 rounded-full bg-amber-400 animate-glow-amber`).
- **Do not truncate error messages** — the current `ActivityCard` has `truncate` on the error div. `GroupedActivityCard` must NOT use `truncate` on the child error text (UI-SPEC requirement).
- **Do not change the virtualizer setup** — keep `measureElement` ref, `estimateSize`, and `overscan: 8`. Only the item array type changes.
- **Do not use `item.id` as the expandedIds key** — use `group.groupKey` (string). After the change, `expandedIds` is `Set<string>`.
- **Do not run grouping on submission_failed items before checking for orphan** — submissions with no matching trigger must be collected as orphans, not silently dropped.
- **Do not search correlationId** — UI-SPEC explicitly states correlationId is an internal detail, not searched.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Virtual list row height measurement | Custom IntersectionObserver | TanStack Virtual `measureElement` ref (already in use) | Already handles dynamic height correctly |
| Pulsing dot animation | Custom CSS @keyframes | `animate-glow-amber` / `animate-glow-red` already in tailwind.config.js | Keyframes already declared and tested |
| Conditional class merging | String concatenation | `clsx` (already imported in ActivityCard) | Handles falsy values cleanly |

---

## Common Pitfalls

### Pitfall 1: Mismatched expandedIds key type after refactor

**What goes wrong:** Old `expandedIds: Set<number>` checks like `expandedIds.has(item.id)` silently pass TypeScript if `item.id` is `number` — but after the refactor the set holds `string` groupKeys. Any call site that still passes `item.id` (a number) will always return false (Set type mismatch at runtime in JS since `has(2) !== has("2")`).

**Why it happens:** The type change is `Set<number>` → `Set<string>`. The old numeric `item.id` and the new string `groupKey` look similar but compare differently.

**How to avoid:** Change `expandedIds` type annotation to `Set<string>` immediately and trace all `expandedIds.has(...)` and `toggleExpanded(...)` call sites.

**Warning signs:** Cards never stay expanded after a click.

### Pitfall 2: Grouping Map keyed on correlationId collision

**What goes wrong:** If two triggers arrive with the same `correlationId` (shouldn't happen with UUID v7, but defensive coding matters), the second trigger overwrites the first in the Map, losing the earlier event.

**Why it happens:** `byCorrelation.set(key, ...)` unconditionally overwrites.

**How to avoid:** On a trigger `set` call, if the key already exists, keep the earlier entry (first-write-wins). This is a low-probability edge case but worth noting.

### Pitfall 3: Virtual list total-size jump on expansion

**What goes wrong:** Expanding a grouped card suddenly makes it much taller (parent details + child card + child raw JSON). The virtualizer uses `ESTIMATED_ITEM_HEIGHT = 90` as initial estimate. After measuring, it corrects. But the scroll position can jump if correction happens after render.

**Why it happens:** TanStack Virtual's `measureElement` corrects after layout, causing a reflow.

**How to avoid:** This is inherent to virtual lists with dynamic content — the `measureElement` pattern already handles it. No code change needed. The existing `overscan: 8` reduces visible artifacts.

### Pitfall 4: Orphan submissions silently disappearing when flat filter applied

**What goes wrong:** If the status filter is applied only to `GroupedActivityEvent[]` and orphan submissions are handled separately, a "Pending" filter that hides non-pending groups will also hide orphans entirely — even though orphans should always be visible (they have no status).

**How to avoid:** Orphan submissions bypass the status filter — they always appear regardless of the status filter tab selection. Only `GroupedActivityEvent` entries are status-filtered.

### Pitfall 5: appStore FIFO eviction removing failed events

**What goes wrong:** The current `addActivity` slice removes from the front of the array unconditionally. A burst of 2000 successful submissions after several failures will evict the failures.

**How to avoid:** Implement the eviction guard described in Pattern 5 before connecting the feed to production traffic. This is an appStore change, not a component change — plan it as a separate task.

---

## Code Examples

### GroupedActivityCard collapsed header with status dot

```typescript
// Source: 06-UI-SPEC.md Component Anatomy
<div className="flex items-center gap-2 min-w-0" onClick={onToggleExpand} role="button">
  <span className={clsx('shrink-0 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wide', kindPillClass)}>
    Trigger
  </span>
  <span className={clsx('shrink-0 px-2 py-0.5 rounded text-xs font-medium', accent.pill)}>
    {triggerLabel}
  </span>
  {group.status === 'pending' && (
    <span
      className="w-2 h-2 rounded-full bg-amber-400 animate-glow-amber shrink-0"
      aria-label="Waiting for submission"
    />
  )}
  {group.status === 'failed' && (
    <span
      className="w-2 h-2 rounded-full bg-red-400 animate-glow-red shrink-0"
      aria-label="Submission failed"
    />
  )}
  <span className="shrink-0 text-tan-muted text-xs ml-auto font-mono">
    {formatTimestamp(group.trigger.ts)}
  </span>
</div>
```

### Child card structure (expanded state)

```typescript
// Source: 06-UI-SPEC.md Child Card Styling
{expanded && group.submission && (
  <div className="ml-2 mt-2 border border-charcoal-light bg-charcoal-darkest rounded-md pl-3 pr-3 pt-3 pb-3">
    <div className="flex items-center gap-2 min-w-0">
      <span className={clsx('shrink-0 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wide',
        group.submission.kind === 'submission_failed'
          ? 'bg-red-900/40 text-red-400'
          : 'bg-blue-900/40 text-blue-400'
      )}>
        {group.submission.kind === 'submission_failed' ? 'Failed' : 'Submit'}
      </span>
      <span className="shrink-0 text-tan-muted text-xs ml-auto font-mono">
        {formatTimestamp(group.submission.ts)}
      </span>
    </div>
    {group.submission.error && (
      <div className="mt-1 text-xs text-red-400">
        Error: {group.submission.error}
      </div>
    )}
    <button type="button" className="mt-2 text-xs text-tan-muted hover:text-beige-warm cursor-pointer select-none"
      onClick={() => setChildRawExpanded(p => !p)}>
      Raw {childRawExpanded ? '▲' : '▼'}
    </button>
    {childRawExpanded && (
      <div className="mt-2 p-3 rounded bg-charcoal-darkest text-beige-light/90 font-mono text-xs leading-relaxed overflow-x-auto max-h-80 overflow-y-auto">
        <pre className="whitespace-pre-wrap">
          {group.submission.error
            ? `// Error\n${group.submission.error}`
            : `// Submission\n${JSON.stringify(group.submission.triggerData, null, 2)}`}
          {group.submission.correlationId
            ? `\n\n// Correlation ID\n${group.submission.correlationId}`
            : ''}
        </pre>
      </div>
    )}
  </div>
)}
```

### Status filter tab array

```typescript
// Source: 06-UI-SPEC.md Filter Tabs
const STATUS_TABS = ['all', 'pending', 'failed', 'complete'] as const;
type StatusFilter = typeof STATUS_TABS[number];

const TAB_LABELS: Record<StatusFilter, string> = {
  all: 'All',
  pending: 'Pending',
  failed: 'Failed',
  complete: 'Complete',
};
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Kind-filter tabs (trigger/submission) | Status-based tabs (All/Pending/Failed/Complete) | Phase 6 | Filter operates on grouped events, not individual item kind |
| Flat `ActivityItem[]` in virtualizer | `GroupedActivityEvent[]` + orphan `ActivityItem[]` | Phase 6 | Single-pass Map grouping, expandedIds key becomes string |
| `expandedIds: Set<number>` | `expandedIds: Set<string>` (groupKey) | Phase 6 | Call sites must pass `group.groupKey` not `item.id` |
| `ActivityCard` renders all items | `GroupedActivityCard` for groups, `ActivityCard` for orphans | Phase 6 | ActivityCard retained unchanged for orphan submissions |
| Unconditional FIFO eviction | Eviction guards failed items | Phase 6 | `submission_failed` items never removed from list |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `animate-glow-amber` and `animate-glow-red` utility classes are generated by Tailwind from the declared keyframes and available as class names | Standard Stack | If not generated, dots will not animate — verify with `npx tailwindcss --content ... --dry-run` or visual check in dev |
| A2 | Phase 4 is complete and all ActivityItems in the live store already carry `correlationId` | Phase Requirements | If Phase 4 is not deployed, no grouping can occur — all triggers appear as standalone pending with no correlationId to match |
| A3 | The submission event always arrives after its corresponding trigger in the flat `activityList` (FIFO append order) | Architecture Patterns | If submissions arrive before triggers (due to timing), the single-pass grouping Map will place the submission as an orphan. The planner should consider a two-pass approach or a "late arrival" correction step. |

---

## Open Questions

1. **Submission arriving before its trigger (race condition)**
   - What we know: Triggers and submissions are independent Tauri events appended to `activityList` in arrival order. The trigger event fires first in the backend pipeline, but the frontend Tauri event bus provides no ordering guarantee.
   - What's unclear: Whether in practice submission events ever arrive at the frontend before their trigger event, given the backend pipeline (trigger → engine → aggregator → submission).
   - Recommendation: The backend pipeline makes same-event reordering unlikely, but if the planner wants defensive handling, a two-pass grouping (pass 1: collect all triggers, pass 2: match submissions) would handle it. This adds trivial cost. Single-pass is the UI-SPEC recommendation and is correct for expected ordering.

2. **Failed event count vs. 2000 cap interaction at scale**
   - What we know: Failed events are never evicted. If failures accumulate indefinitely, `activityList` may grow beyond 2000 items.
   - What's unclear: Whether the planner wants a separate cap for failed items (e.g., max 200 failed events retained).
   - Recommendation: For Phase 6, implement no-eviction for failed as specified. If the list grows unbounded in production, that is a future concern. The `clearActivity` button already exists.

---

## Environment Availability

Step 2.6: SKIPPED — Phase 6 is a pure frontend code change with no external tool dependencies beyond the existing Node/npm toolchain already verified present (`node v22.22.2`).

---

## Validation Architecture

`workflow.nyquist_validation` is explicitly `false` in `.planning/config.json`. Section skipped per config.

---

## Security Domain

This phase introduces no new network endpoints, no authentication logic, no user-supplied data written to storage, and no cryptographic operations. All changes are read-only display transformations of already-received Tauri events.

ASVS V5 (Input Validation) applies minimally: the `error` field from `SubmissionFailedEvent` is displayed as text content in JSX (not as HTML), so XSS is not a concern. No additional controls are needed.

---

## Sources

### Primary (HIGH confidence)
- `/workspace/app/src/components/activity/ActivityFeed.tsx` — full current implementation, 324 lines
- `/workspace/app/src/components/activity/ActivityCard.tsx` — full current implementation, 252 lines
- `/workspace/app/src/types/index.ts` — ActivityItem, ActivityKind, all event interfaces
- `/workspace/app/src/stores/appStore.ts` — addActivity, FIFO cap logic
- `/workspace/app/src/tauri/listeners.ts` — ActivityItem construction from all three event types
- `/workspace/app/tailwind.config.js` — keyframes, custom color tokens, font
- `/workspace/app/package.json` — verified package versions
- `/workspace/.planning/phases/06-unified-activity-frontend/06-CONTEXT.md` — locked decisions
- `/workspace/.planning/phases/06-unified-activity-frontend/06-UI-SPEC.md` — component anatomy, filter tabs, data model, copywriting
- `/workspace/.planning/phases/04-rust-event-foundation/04-01-SUMMARY.md` — Phase 4 completion confirmation, what was built

### Secondary (MEDIUM confidence)
- `/workspace/.planning/ROADMAP.md` — Phase 6 success criteria used to derive requirement IDs EVT-02 through ERR-04

---

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — package.json and tailwind.config.js directly verified; no new packages needed
- Architecture: HIGH — all patterns derived from reading actual source files and UI-SPEC
- Pitfalls: HIGH — identified from code inspection (existing truncate class on error in ActivityCard, numeric expandedIds Set, unconditional FIFO eviction)

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable codebase, no moving dependencies)
