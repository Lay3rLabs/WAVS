# Phase 9: Settings Scroll Refactor - Context

**Gathered:** 2026-04-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Convert the Settings page from conditional single-section rendering to a single scrollable page where all sections are visible. The sidebar tracks the currently visible section via IntersectionObserver and supports click-to-scroll navigation. OAuth listener and all page-level state must survive scrolling without unmounting.

</domain>

<decisions>
## Implementation Decisions

### Scroll Architecture
- Use IntersectionObserver on section `<div id="section-{key}">` elements for scroll tracking — native, performant, no scroll event spam
- Use `document.getElementById(key).scrollIntoView({ behavior: 'smooth' })` for click-to-scroll
- Render ALL sections always — remove `{activeSection === 'key' && ...}` conditional guards, all sections visible on one page
- Repurpose `activeSection` state as "highlighted section in sidebar" driven by IntersectionObserver, not click-to-navigate

### Section Layout & Separators
- Horizontal divider (`border-b border-charcoal-light`) between sections
- Add `<h2>` headings matching sidebar labels above each section for orientation while scrolling
- `py-8` (32px) padding per section for breathing room

### State Survival & Edge Cases
- OAuth listener stays in parent Settings.tsx — all sections rendered means parent never unmounts, listener persists naturally
- Sticky sidebar: `position: sticky; top: 0` so sidebar stays visible while content scrolls
- Reset scroll to top when navigating away and back — no scroll position persistence needed

### Claude's Discretion
- IntersectionObserver threshold value (0.5 vs 0.3 etc.)
- Exact heading styles (font size, color, spacing)
- Whether to extract the IntersectionObserver logic into a custom hook

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Settings.tsx` — parent page with OAuth listener, error state, restart banner
- `SettingsSidebar.tsx` — already has active state highlighting with `border-l-2 border-purple-2`
- Six section components: WalletSection, NodeSection, EnvironmentSection, AgentSection, McpSection, ResetSection
- `SIDEBAR_ITEMS` array in SettingsSidebar — ordered list of sections

### Established Patterns
- Sidebar is 200px fixed width, content is `flex-1` with `overflow-y-auto`
- Active section highlighted with left border + background color
- `SectionKey` type union constrains valid section identifiers
- Sections receive props from parent (settings, callbacks, OAuth state)

### Integration Points
- `app/src/pages/Settings.tsx` — main refactor target (lines 86-118 conditional rendering → always-render)
- `app/src/components/settings/SettingsSidebar.tsx` — change onClick from navigate to scrollIntoView
- Content container `max-h-[calc(100vh-12rem)]` with `overflow-y-auto` — scrolls the sections

</code_context>

<specifics>
## Specific Ideas

- The content container already has `overflow-y-auto` — it's the scroll container
- Each section already has a `SectionKey` identifier — use these as div IDs for IntersectionObserver targets
- SettingsSidebar `onSelect` callback currently sets `activeSection` state — change to call `scrollIntoView` instead
- IntersectionObserver callback should update `activeSection` (now meaning "visible section") to keep sidebar highlight in sync

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
