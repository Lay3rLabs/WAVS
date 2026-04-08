# Phase 12: Components List Page - Research

**Researched:** 2026-04-08
**Domain:** React + Tauri frontend enhancement — ComponentsPage.tsx
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Card Enhancement**
- Each card shows: source-type badge, digest, function count badge (e.g., "4 functions"), and permissions summary as icon row (network, filesystem, sockets)
- Entire card is clickable — wraps in React Router Link to `/components/:digest`
- Cards fetch schema/metadata from the Phase 10 Tauri commands to get function count and permissions data

**Search & Filter**
- Client-side text input filter — matches on component name/package and digest (component count is small)
- Horizontal pill/chip toggles for source-type filtering: Registry / Download / Digest / OCI
- Multi-select filter — toggle each source type on/off, default all selected
- Search and filter combine (AND logic): text filter AND source-type filter applied together

**Layout & Empty States**
- Responsive grid layout (CSS grid auto-fill) — adapts to viewport width
- No search results: "No components match your search" with clear filter button
- No components at all: "No components deployed yet" message

### Claude's Discretion

No items deferred to Claude's discretion — all areas accepted as recommended.

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| LIST-01 | User can see richer component cards showing function count, source type badge, and permissions summary | `ComponentSchema.exports` key count for functions; `ComponentMetadata.permissions` for permissions row; badge pattern from existing ComponentsPage |
| LIST-02 | User can search components by name or digest | `useState` controlled input + case-insensitive filter over componentMap keys/names; TextInput atom reuse |
| LIST-03 | User can filter components by source type (Registry/Download/Digest) | `useState<Set<string>>` toggled pills; joined-button pill pattern from ActivityFeed.tsx confirmed |
| LIST-04 | User can click a component card to navigate to its detail page | React Router `<Link to={/components/${digest}}>` wrapping each card; route already registered in App.tsx |
</phase_requirements>

---

## Summary

Phase 12 enhances the existing `ComponentsPage.tsx` — a single-file React component — with three complementary features: richer card content (function count + permissions), client-side search/filter, and card-level navigation to the detail page introduced in Phase 11.

All dependencies are already present in the codebase. `getComponentSchema` and `getComponentMetadata` are registered Tauri commands in `commands.ts`. `ComponentSchema` and `ComponentMetadata` types exist in `types/index.ts`. The `useComponentDetail` hook from Phase 11 provides the `Promise.allSettled` pattern to reuse. The `TextInput` atom, the joined-pill button pattern (from `ActivityFeed.tsx`), and the `Toast.error` call pattern are all established and verified.

The key implementation challenge is fetching schema and metadata for every component on the list page in parallel without a full-page loading state. The UI-SPEC specifies: fetch once on mount via `Promise.allSettled`, store results in a local `Map<digest, { schema, metadata }>`, render cards immediately from synchronous Zustand data, and progressively show badges when async data arrives. This avoids blocking the list while data loads.

**Primary recommendation:** Enhance `ComponentsPage.tsx` in place — add `useState` for search string and source-type filter set, fetch all component schemas/metadata on mount with `Promise.allSettled`, store in a local state map, then derive `filteredComponents` from the existing `componentMap` using both filters. Wrap each card in a React Router `<Link>`.

---

## Standard Stack

### Core (all already installed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19 (existing) | Component state, effects | Project uses React 19 |
| react-router-dom | existing | `<Link>` for card navigation | Already used on ComponentDetailPage |
| Tauri invoke | existing | `getComponentSchema`, `getComponentMetadata` | Already wired in commands.ts |
| Tailwind CSS | existing | All styling | Project design system |

[VERIFIED: codebase grep — all imports confirmed in ComponentsPage.tsx, commands.ts, App.tsx]

### Supporting Atoms (no modifications needed)

| Atom | File | Usage in Phase 12 |
|------|------|-------------------|
| `TextInput` | `app/src/components/atoms/TextInput.tsx` | Search box — pass `value`, `onChange`, `placeholder` |
| `AddressDisplay` | `app/src/components/atoms/AddressDisplay.tsx` | Digest display (already used in ComponentsPage) |
| `Toast` | `app/src/components/atoms/Toast.tsx` | `Toast.error(msg)` for batch fetch failure |

[VERIFIED: all atoms confirmed present by directory listing and file read]

### No New Dependencies

No npm packages need to be installed for this phase. All required libraries are already in the project.

---

## Architecture Patterns

### Current ComponentsPage Structure

The existing `ComponentsPage.tsx` [VERIFIED: file read]:
1. Calls `useServicePolling()` + reads `services` from Zustand store
2. Derives `componentMap: Map<digest, ComponentUsage[]>` in render (synchronous, no async)
3. Renders a flat `flex flex-col gap-4` list of card `<div>` elements
4. Cards are NOT clickable (no Link wrapping)
5. No search, no filter

### Pattern 1: Parallel Async Fetch on Mount

**What:** `Promise.allSettled` over all unique digests, results stored in local state Map.

**When to use:** When data for multiple independent items must be fetched and partial failures are acceptable (cards render without async data if fetch fails).

**Example (from `useComponentDetail.ts` — adapted for multi-digest):**

```typescript
// Source: /workspace/app/src/hooks/useComponentDetail.ts (verified)
const [componentDataMap, setComponentDataMap] = useState<
  Map<string, { schema: ComponentSchema | null; metadata: ComponentMetadata | null }>
>(() => new Map());

useEffect(() => {
  const digests = Array.from(componentMap.keys());
  if (digests.length === 0) return;

  Promise.allSettled(
    digests.map(async (digest) => {
      const [schemaResult, metaResult] = await Promise.allSettled([
        getComponentSchema(digest),
        getComponentMetadata(digest),
      ]);
      return {
        digest,
        schema: schemaResult.status === 'fulfilled' ? schemaResult.value : null,
        metadata: metaResult.status === 'fulfilled' ? metaResult.value : null,
        error: schemaResult.status === 'rejected' || metaResult.status === 'rejected',
      };
    })
  ).then((results) => {
    const newMap = new Map<string, { schema: ComponentSchema | null; metadata: ComponentMetadata | null }>();
    let hasError = false;
    for (const result of results) {
      if (result.status === 'fulfilled') {
        const { digest, schema, metadata, error } = result.value;
        newMap.set(digest, { schema, metadata });
        if (error) hasError = true;
      }
    }
    setComponentDataMap(newMap);
    if (hasError) Toast.error('Failed to load component data: some schema or metadata could not be fetched');
  });
}, []); // empty deps: fetch once on mount only
```

[VERIFIED: Promise.allSettled pattern confirmed in useComponentDetail.ts]

### Pattern 2: Source-Type Filter with Set State

**What:** `useState<Set<string>>` where empty set = "All" active. Toggle adds/removes from set.

**When to use:** Multi-select filter where "all" is the default state.

**Example (from UI-SPEC interaction contract — verified):**
```typescript
// Source: 12-UI-SPEC.md (verified)
const [activeSourceTypes, setActiveSourceTypes] = useState<Set<string>>(() => new Set());
// empty set = "All" active

const toggleSourceType = (type: string) => {
  setActiveSourceTypes(prev => {
    const next = new Set(prev);
    if (next.has(type)) {
      next.delete(type);
    } else {
      next.add(type);
    }
    // If set becomes empty, revert to "All" (empty set)
    return next;
  });
};

const clearFilters = () => {
  setSearch('');
  setActiveSourceTypes(new Set()); // reset to "All"
};
```

### Pattern 3: Joined Pill Button Group

**What:** Borderless button row wrapped in a single rounded container with outer border.

**When to use:** Status/type toggle filters. Confirmed pattern in ActivityFeed.tsx.

**Example (from ActivityFeed.tsx — verified):**
```tsx
// Source: /workspace/app/src/components/activity/ActivityFeed.tsx lines 204-219 (verified)
<div className="flex rounded-md overflow-hidden border border-charcoal-light">
  {TABS.map((tab) => (
    <button
      key={tab}
      type="button"
      className={`px-3 py-1.5 text-xs font-medium transition-colors cursor-pointer ${
        activeTab === tab
          ? 'bg-purple-1 text-cream-light'
          : 'bg-charcoal-dark text-tan-muted hover:text-beige-warm hover:bg-charcoal-medium'
      }`}
      onClick={() => setActiveTab(tab)}
    >
      {tab}
    </button>
  ))}
</div>
```

Note: UI-SPEC specifies `font-normal` (weight 400) for filter pills in this phase, not `font-medium` as in ActivityFeed. Follow UI-SPEC.

### Pattern 4: Card as React Router Link

**What:** `<Link>` wrapping a `<div>` card, with `e.preventDefault()` on nested interactive elements.

**When to use:** Entire card surface is clickable, but inner buttons (service chips) navigate elsewhere.

**Example (from UI-SPEC — verified):**
```tsx
// Source: 12-UI-SPEC.md (verified)
<Link to={`/components/${digest}`} className="block">
  <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light
                  hover:border-purple-1 transition-colors cursor-pointer">
    {/* ...card content... */}
    <button
      onClick={(e) => { e.preventDefault(); navigate(`/services/${chain}/${address}`); }}
      className="..."
    >
      {usage.serviceName} — {usage.workflowId}
    </button>
  </div>
</Link>
```

[VERIFIED: `e.preventDefault()` pattern required per UI-SPEC note; React Router route `/components/:digest` confirmed registered in App.tsx]

### Pattern 5: Derived filteredComponents

**What:** Compute filtered list from `componentMap` using both search and source-type filter, memoized or derived in render (list is small).

**Example:**
```typescript
// Source: design — consistent with ActivityFeed filter pattern (verified)
const filteredComponents = Array.from(componentMap.entries()).filter(([digest, usages]) => {
  const source = usages[0].component.source;
  const sourceType = getSourceType(source).toLowerCase(); // 'registry'|'download'|'digest'|'oci'

  // Source-type filter (empty set = All)
  if (activeSourceTypes.size > 0 && !activeSourceTypes.has(sourceType)) return false;

  // Text search: match on digest or package/name
  if (search.trim()) {
    const q = search.trim().toLowerCase();
    const nameMatch = getComponentName(source).toLowerCase().includes(q);
    const digestMatch = digest.toLowerCase().includes(q);
    if (!nameMatch && !digestMatch) return false;
  }

  return true;
});
```

### Anti-Patterns to Avoid

- **Showing "0 functions" badge while loading:** Badge must be omitted entirely until schema data is available. Never show a zero or placeholder count. [VERIFIED: UI-SPEC States Contract]
- **Showing permissions row as skeleton while loading:** Omit the row entirely; render nothing until metadata arrives. [VERIFIED: UI-SPEC States Contract]
- **Refetching schema/metadata on re-render or polling:** Fetch once on mount only. Schema/metadata is static per digest. [VERIFIED: UI-SPEC Interaction Contract]
- **Debouncing search:** No debounce needed — all data is already in memory client-side. Instant filter on every keystroke. [VERIFIED: UI-SPEC]
- **Full-page loading state:** Do not block card render on async data. Cards render synchronously from Zustand store. [VERIFIED: UI-SPEC States Contract]
- **Using `font-medium` on filter pills:** UI-SPEC specifies `font-normal` to limit font weights to 400 and 600. [VERIFIED: UI-SPEC Typography]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Schema fetch per component | Custom fetch hook | Inline `Promise.allSettled` in `useEffect` (pattern from `useComponentDetail.ts`) | Phase 11 already solved this; adapt directly |
| Search input styling | Custom input | `TextInput` atom | Atom provides all correct Tailwind classes already |
| Toast on error | Custom notification | `Toast.error(msg)` | Global toast system already wired; `ToastContainer` in App.tsx |
| Source type label | Custom map | `getSourceType` helper (already in ComponentsPage) | Already present in file |

**Key insight:** This phase is an enhancement, not greenfield. Every pattern needed already exists — the work is composing them, not building new infrastructure.

---

## Runtime State Inventory

Step 2.5: SKIPPED — Phase 12 is a frontend enhancement (no renames, refactors, or migrations). No stored data, live service config, OS registrations, secrets, or build artifacts are affected.

---

## Environment Availability

Step 2.6: SKIPPED — Phase 12 is a pure frontend code change within the existing Tauri/React app. No new external tools, services, CLIs, or runtimes are required beyond the already-running development environment.

---

## Common Pitfalls

### Pitfall 1: componentMap Dependency in useEffect

**What goes wrong:** Including `componentMap` in the `useEffect` dependency array for the schema/metadata fetch causes refetching every time services poll (every 5 seconds via `useServicePolling`).

**Why it happens:** `componentMap` is derived in render from `services` — it is a new `Map` instance on every render even if contents haven't changed. Putting it in deps causes infinite fetch loops.

**How to avoid:** Use an empty dependency array `[]` — fetch once on mount. The component count in a typical WAVS deployment is small (< 100) and schema/metadata is static per digest.

**Warning signs:** Network tab showing repeated Tauri IPC calls to `cmd_get_component_schema` at 5-second intervals.

### Pitfall 2: Toast Spam on Batch Failure

**What goes wrong:** Calling `Toast.error()` once per failed component digest floods the user with notifications if multiple fetches fail (e.g., WAVS is down).

**Why it happens:** Naive loop over `Promise.allSettled` results calls Toast per rejection.

**How to avoid:** Collect all errors into a single boolean flag; emit one Toast for the entire batch. UI-SPEC error copy: `"Failed to load component data: {reason}"`.

**Warning signs:** Multiple stacked toasts appearing simultaneously.

### Pitfall 3: `e.preventDefault()` Missing on Service Chips

**What goes wrong:** Clicking a service workflow chip inside a card navigates to BOTH the detail page (from the `<Link>`) AND the service page (from the chip's `onClick`).

**Why it happens:** Click events bubble up through the `<Link>` wrapper.

**How to avoid:** Always call `e.preventDefault()` before `navigate()` on all nested interactive elements inside the card `<Link>`.

**Warning signs:** Two navigation events fire in quick succession; browser history shows an extra entry.

### Pitfall 4: `getSourceType` Returns 'Digest' but Filter Key is 'digest'

**What goes wrong:** The filter Set stores lowercase source type keys ('registry', 'download', 'digest', 'oci') but `getSourceType()` returns title-case strings ('Registry', 'Download', 'Digest').

**Why it happens:** Inconsistent casing between display labels and filter keys.

**How to avoid:** Store filter values in lowercase; compare with `.toLowerCase()`. Or add a `getSourceTypeKey()` helper returning lowercase. The existing `getSourceType()` can stay as-is for display.

**Warning signs:** Source-type filter pills have no effect on the list despite matching components being present.

### Pitfall 5: Source-Type Pill List Showing 'OCI' When No OCI Components Exist

**What goes wrong:** The filter pills show "OCI" even when no components of that type are deployed, confusing users.

**Why it happens:** Hardcoding all four source types instead of deriving from actual `componentMap` contents.

**How to avoid:** Compute the set of source types present in `componentMap`, then render only those plus "All". UI-SPEC: "The filter pill group renders only the source types that exist in the current component list."

**Warning signs:** OCI pill visible but clicking it produces a "No components match" state with no way to see OCI components (they don't exist).

---

## Code Examples

### Function Count from Schema

```typescript
// Source: types/index.ts (verified)
// ComponentSchema.exports is Record<string, { inputSchema, outputSchema, description? }>
const functionCount = schema ? Object.keys(schema.exports).length : null;
// Render: omit badge when null (not yet loaded)
{functionCount !== null && (
  <span className="ml-auto px-1.5 py-0.5 text-xs font-normal bg-charcoal-light text-beige-warm rounded">
    {functionCount} {functionCount === 1 ? 'function' : 'functions'}
  </span>
)}
```

### Permissions Derivation from Metadata

```typescript
// Source: types/index.ts + UI-SPEC (both verified)
// Permissions interface: { allowed_http_hosts: AllowedHostPermission; file_system: boolean; raw_sockets: boolean; dns_resolution: boolean }
// AllowedHostPermission: 'all' | { only: string[] } | 'none'

const hasNetworkAccess = metadata
  ? metadata.permissions.allowed_http_hosts !== 'none'
  : false;
const hasFileSystem = metadata?.permissions.file_system ?? false;
const hasRawSockets = metadata?.permissions.raw_sockets ?? false;

// Row omitted entirely if metadata === null
{metadata && (
  <div className="flex items-center gap-3 text-xs text-tan-muted mb-3">
    {hasNetworkAccess && <span>Network</span>}
    {hasFileSystem && <span>Filesystem</span>}
    {hasRawSockets && <span>Sockets</span>}
    {!hasNetworkAccess && !hasFileSystem && !hasRawSockets && (
      <span className="italic">No special permissions</span>
    )}
  </div>
)}
```

### Empty State: No Components

```tsx
// Source: UI-SPEC States Contract + CONTEXT.md copy (verified)
// NOTE: existing ComponentsPage.tsx says "No services registered yet." — this must change to:
<p className="text-tan-muted italic">
  No components deployed yet.{' '}
  <button className="text-purple-1 hover:underline" onClick={() => navigate('/services')}>
    Add a service
  </button>{' '}
  to see its components.
</p>
```

### Empty State: No Search Results

```tsx
// Source: UI-SPEC States Contract (verified)
<div className="flex flex-col gap-3 py-6">
  <p className="text-tan-muted text-sm">No components match your search.</p>
  <button
    className="text-xs text-tan-muted hover:text-beige-warm underline self-start"
    onClick={clearFilters}
  >
    Clear filters
  </button>
</div>
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Cards not clickable | Wrap entire card in `<Link>` | Phase 12 | Enables detail page navigation (LIST-04) |
| No search or filter | Client-side `useState` search + pill filter | Phase 12 | Enables LIST-02, LIST-03 |
| Digest + source type only | + function count badge + permissions row | Phase 12 | Enables LIST-01 |
| Empty state: "No services registered yet" | "No components deployed yet." | Phase 12 | Matches UI-SPEC copy contract |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `componentMap` keys are always the same string digest used in the `/components/:digest` route | Architecture Patterns, Card Navigation | If digests differ in format (e.g., hash prefix vs full), Link navigation breaks. Low risk — same `getDigest()` helper is used in both ComponentsPage and ComponentDetailPage. [VERIFIED: both files call same helper function] |

**All other claims are VERIFIED against the codebase in this session.**

---

## Open Questions

None. All decisions are locked in CONTEXT.md and verified against the existing codebase.

---

## Validation Architecture

Skipped — `workflow.nyquist_validation` is explicitly `false` in `.planning/config.json`.

---

## Security Domain

Not applicable — this phase is a read-only frontend list page with no authentication, data mutation, user input sent to backend, or cryptographic operations. The search/filter state is ephemeral client-side state only.

---

## Sources

### Primary (HIGH confidence)

- `/workspace/app/src/pages/ComponentsPage.tsx` — existing page structure, card pattern, helper functions verified
- `/workspace/app/src/pages/components/ComponentDetailPage.tsx` — Phase 11 detail page, Link target route, type usage patterns
- `/workspace/app/src/hooks/useComponentDetail.ts` — Promise.allSettled pattern for schema+metadata fetch
- `/workspace/app/src/tauri/commands.ts` — `getComponentSchema`, `getComponentMetadata` command signatures verified
- `/workspace/app/src/types/index.ts` — `ComponentSchema`, `ComponentMetadata`, `Permissions`, `AllowedHostPermission` type shapes verified
- `/workspace/app/src/components/atoms/TextInput.tsx` — TextInput API and Tailwind classes verified
- `/workspace/app/src/components/activity/ActivityFeed.tsx` — joined pill button pattern verified (lines 204-219)
- `/workspace/app/src/App.tsx` — `/components/:digest` route confirmed registered
- `.planning/phases/12-components-list-page/12-UI-SPEC.md` — layout, states, interaction, and copy contracts
- `.planning/phases/12-components-list-page/12-CONTEXT.md` — locked decisions
- `.planning/config.json` — `nyquist_validation: false` confirmed

### Secondary (MEDIUM confidence)

None required — all relevant decisions and patterns are verified directly from codebase.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries confirmed present in codebase
- Architecture: HIGH — all patterns verified in existing files
- Pitfalls: HIGH — derived from code inspection of actual types, existing patterns, and UI-SPEC
- Type shapes: HIGH — verified directly from `types/index.ts`

**Research date:** 2026-04-08
**Valid until:** 2026-05-08 (stable frontend codebase, patterns unlikely to change)
