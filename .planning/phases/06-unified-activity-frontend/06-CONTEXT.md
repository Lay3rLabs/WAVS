# Phase 6: Unified Activity Frontend - Context

**Gathered:** 2026-04-07
**Status:** Ready for planning

<domain>
## Phase Boundary

The activity feed on both the Activity page and the Service detail tab displays triggers and submissions as nested parent-child events, shows inline error messages for failed submissions, and replaces the kind-filter tabs with event-appropriate filtering.

</domain>

<decisions>
## Implementation Decisions

### Nesting & Grouping Model
- Client-side grouping by correlationId — group ActivityItems with matching correlationId into a single card (trigger is parent, submission is child)
- Grouping logic lives in ActivityFeed.tsx via useMemo — derive grouped items from the flat activity list
- Standalone triggers (no submission yet) show as single card with pending indicator (pulsing amber dot next to kind badge)
- Orphan submissions (no matching trigger) show as standalone cards — handle gracefully

### Error & Status Display
- Error badge: red dot badge next to kind pill on collapsed card — subtle but visible at a glance
- Error message: inline red text below submission details within the existing expand section
- Failed events are never auto-removed from the activity feed — successful events follow existing FIFO (2000 cap)
- Pending indicator: pulsing amber dot next to kind badge, same position as error dot

### Filtering Changes
- Replace kind-filter tabs (trigger/submission) with status-based tabs: All / Pending / Failed / Complete
- "Failed" filter shows grouped events where the submission has failed (whole card visible)
- "Pending" filter shows grouped events where trigger has no matching submission yet
- Search unchanged — searches service name, workflow ID, trigger data label across grouped events

### Claude's Discretion
- Internal data structure for grouped events (interface shape)
- Animation/transition details for expand/collapse
- Exact CSS for pulsing amber dot and red error dot
- Whether to add correlationId to search

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `app/src/components/activity/ActivityCard.tsx` (252 lines) — already expandable with raw JSON toggle
- `app/src/components/activity/ActivityFeed.tsx` (292 lines) — filtering, virtualizer, pause/resume
- `app/src/components/service/ServiceActivity.tsx` — per-service wrapper passing serviceId + workflowIds
- `app/src/types/index.ts` — ActivityKind already includes 'submission_failed', ActivityItem has correlationId and error fields (from Phase 4)

### Established Patterns
- Tauri event listeners in `app/src/tauri/listeners.ts` create ActivityItems
- Zustand store (appStore.ts) manages activity list with 2000-item FIFO cap
- ActivityFeed uses useMemo for filtering and TanStack Virtual for scrolling
- ActivityCard uses lifted expanded state (expandedIds Set in parent)

### Integration Points
- ActivityFeed.tsx — main filtering and rendering logic to modify
- ActivityCard.tsx — card display to modify for nested view and status indicators
- Both standalone Activity page and ServiceActivity tab use ActivityFeed

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
