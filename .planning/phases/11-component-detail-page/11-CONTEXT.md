# Phase 11: Component Detail Page - Context

**Gathered:** 2026-04-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Create a per-component detail page at `/components/:digest` showing full interface schema, permissions, resource limits, and configuration. Consumes the two Tauri commands from Phase 10. No changes to the components list page (Phase 12).

</domain>

<decisions>
## Implementation Decisions

### Page Layout & Navigation
- Header section + tabbed content layout — follows existing ServiceDetailPage pattern with title/digest/badges at top, tabs below
- Navigate via clicking component card on ComponentsPage → `/components/:digest` route
- Back navigation via browser back button + breadcrumb ("Components > {name/digest}")
- Three tabs: Interface / Permissions / Configuration — groups related info logically

### Interface Display
- Expandable accordion per exported function — shows function name collapsed, expands to show input/output JSON Schema
- JSON Schema rendered as formatted tree view with type annotations — not raw JSON
- Source info displayed as colored badge (source type) in header + details in info grid below title (URI/registry/digest)
- "Used by" services shown as clickable service links in header area — links to their detail pages

### Empty & Error States
- Component not found: full-page "Component not found" with link back to components list
- No exports: "No exported functions" message in Interface tab
- Loading state: skeleton placeholders matching final layout

### Claude's Discretion
No items deferred to Claude's discretion — all areas accepted as recommended.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ServiceDetailPage.tsx` — detail page pattern with header/tabs layout to replicate
- `Tabs.tsx` atom — existing tab navigation component
- `Button.tsx` atom — size/color/variant system
- `AddressDisplay.tsx` — address rendering with copy (usable for digest display)
- `Toast` API — for error notifications
- `useServicePolling()` — pattern for Tauri data fetching (but may need new hook for schema/metadata)
- Tauri `invoke<T>()` in `commands.ts` — add new command wrappers

### Established Patterns
- Tailwind CSS with custom color palette (charcoal-*, tan-*, beige-*, cream-*, purple-1)
- Zustand for state management
- React Router v6 with nested routes in App.tsx
- Route params pattern: `/services/:chainId/:address` → replicate as `/components/:digest`

### Integration Points
- `App.tsx` — add new route `/components/:digest`
- `ComponentsPage.tsx` — make cards clickable (link to detail page)
- `commands.ts` — add `getComponentSchema(digest)` and `getComponentMetadata(digest)` wrappers
- Phase 10 Tauri commands: `cmd_get_component_schema`, `cmd_get_component_metadata`

</code_context>

<specifics>
## Specific Ideas

No specific requirements — follow existing ServiceDetailPage pattern adapted for component data.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
