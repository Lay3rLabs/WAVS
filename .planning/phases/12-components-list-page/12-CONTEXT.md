# Phase 12: Components List Page - Context

**Gathered:** 2026-04-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Enhance the existing ComponentsPage with richer cards (function count, source-type badge, permissions summary), client-side search and source-type filtering, and make cards clickable to navigate to the detail page from Phase 11.

</domain>

<decisions>
## Implementation Decisions

### Card Enhancement
- Each card shows: source-type badge, digest, function count badge (e.g., "4 functions"), and permissions summary as icon row (network, filesystem, sockets)
- Entire card is clickable — wraps in React Router Link to `/components/:digest`
- Cards fetch schema/metadata from the Phase 10 Tauri commands to get function count and permissions data

### Search & Filter
- Client-side text input filter — matches on component name/package and digest (component count is small)
- Horizontal pill/chip toggles for source-type filtering: Registry / Download / Digest / OCI
- Multi-select filter — toggle each source type on/off, default all selected
- Search and filter combine (AND logic): text filter AND source-type filter applied together

### Layout & Empty States
- Responsive grid layout (CSS grid auto-fill) — adapts to viewport width
- No search results: "No components match your search" with clear filter button
- No components at all: "No components deployed yet" message

### Claude's Discretion
No items deferred to Claude's discretion — all areas accepted as recommended.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ComponentsPage.tsx` — existing page to enhance (currently shows digest, source type, service usage)
- `ComponentDetailPage.tsx` — Phase 11 detail page at `/components/:digest` (navigation target)
- `useComponentDetail.ts` — hook pattern for Tauri command calls
- `getComponentSchema()` / `getComponentMetadata()` — Phase 10 command wrappers in commands.ts
- `ComponentSourceResult`, `ComponentMetadata`, `ComponentSchema` types from Phase 11
- Source-type badge pattern already in ComponentsPage
- `useServicePolling()` — existing 5s refresh for service/component data

### Established Patterns
- Tailwind CSS with charcoal/tan/beige/purple color palette
- Cards with `bg-charcoal-medium rounded-lg p-4` pattern
- React Router `Link` for navigation
- Client-side state filtering (similar to ActivityFeed status tabs)

### Integration Points
- `ComponentsPage.tsx` — primary file to modify
- `commands.ts` — may need batch command or reuse individual calls per component
- Phase 11's `/components/:digest` route — click target for cards

</code_context>

<specifics>
## Specific Ideas

No specific requirements — enhance the existing page following established patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
