# Phase 5: Settings Decomposition - Context

**Gathered:** 2026-04-07
**Status:** Ready for planning

<domain>
## Phase Boundary

The Settings page is restructured from a 1,221-line monolithic component into a sidebar-navigated layout with each section extracted into an isolated component, without breaking OAuth flows or the unsaved-changes banner.

</domain>

<decisions>
## Implementation Decisions

### Navigation Pattern
- Vertical left sidebar, fixed ~200px width — standard settings pattern
- Instant section swap on click, no animation
- Active section tracked via local useState (no URL hash or deep-linking)
- Sidebar always visible; content area fills remaining width

### Component Architecture
- Each section keeps its own local useState hooks — no migration to Zustand
- `hasUnsavedChanges` state lifted to parent Settings component, passed to sections via props
- Restart/unsaved-changes banner positioned above the sidebar+content split (fixed, always visible)
- OAuth listener stays in parent Settings component (not in Agent section) so it survives section navigation without losing its listener

### Section Grouping & Ordering
- 6 sidebar items: Wallet, Node, Environment, Agent, MCP, Reset
- Default selection on page load: Wallet (first item)
- Concise labels: "Wallet", "Node", "Environment", "Agent", "MCP", "Reset"
- Reset section has same visual style as other items (no special warning color)

### Claude's Discretion
- Exact CSS/Tailwind classes for sidebar styling
- Whether to use a wrapper component or just conditional rendering for section display
- Internal file organization (one file per section or grouped)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `app/src/components/atoms/Tabs.tsx` — existing tabs component (not used for sidebar, but shows pattern)
- `app/src/components/atoms/Toast.tsx` — toast notification system with Zustand store
- `AgentApiKeyField` component (lines 80-274 in Settings.tsx) — encapsulated OAuth flow

### Established Patterns
- State: local useState hooks per page (no Zustand for page-level UI state)
- Styling: Tailwind CSS utility classes, flex layouts
- Navigation: icon-based header nav in `Header.tsx`
- Tauri commands: async imports (`await import('../tauri/agent')`)

### Integration Points
- `app/src/pages/Settings.tsx` — the 1,221-line monolith to decompose
- `app/src/stores/appStore.ts` — reads settings, writes config
- `app/src/tauri/agent.ts` — OAuth commands (agentOAuthLogin, agentSetOauth, etc.)
- Event listener on `'agent:oauth'` channel for OAuth progress/success/error

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
