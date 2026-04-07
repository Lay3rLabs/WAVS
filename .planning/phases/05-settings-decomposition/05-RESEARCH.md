# Phase 5: Settings Decomposition - Research

**Researched:** 2026-04-07
**Domain:** React component decomposition, Tauri event listeners, settings UI layout
**Confidence:** HIGH

## Summary

Phase 5 is a structural refactor of a 1,221-line monolithic `Settings.tsx` into a sidebar-navigated layout with six isolated section components. The work is purely frontend (TypeScript/React/Tailwind) with no Rust or API changes required. The primary technical challenges are: (1) lifting `hasUnsavedChanges` from a derived local value (`tomlContent !== savedContent`) into a proper callback-based prop system shared across sections, (2) preserving the OAuth event listener from `AgentApiKeyField` by keeping it in the parent Settings component rather than the AgentSection child, and (3) correctly distributing state and handlers from the current monolith across 6+1 new component files without leaking cross-section dependencies.

The entire codebase uses hand-rolled Tailwind components. No external component library is involved. All color tokens, spacing, and typographic conventions are already established in `tailwind.config.js` and verified from existing `Settings.tsx` patterns. The new `SettingsSidebar` component follows the same active-state visual pattern as the existing `Tabs.tsx` atom (border-b-2 becomes border-l-2 for vertical orientation).

**Primary recommendation:** One plan, one wave. Extract in section order (Wallet → Node → Environment → Agent → MCP → Reset), create `SettingsSidebar` last (after section interfaces are stabilized), then wire parent Settings to the new layout. All logic migrates to section props/callbacks; no Zustand changes required.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Navigation Pattern**
- Vertical left sidebar, fixed ~200px width — standard settings pattern
- Instant section swap on click, no animation
- Active section tracked via local useState (no URL hash or deep-linking)
- Sidebar always visible; content area fills remaining width

**Component Architecture**
- Each section keeps its own local useState hooks — no migration to Zustand
- `hasUnsavedChanges` state lifted to parent Settings component, passed to sections via props
- Restart/unsaved-changes banner positioned above the sidebar+content split (fixed, always visible)
- OAuth listener stays in parent Settings component (not in Agent section) so it survives section navigation without losing its listener

**Section Grouping & Ordering**
- 6 sidebar items: Wallet, Node, Environment, Agent, MCP, Reset
- Default selection on page load: Wallet (first item)
- Concise labels: "Wallet", "Node", "Environment", "Agent", "MCP", "Reset"
- Reset section has same visual style as other items (no special warning color)

### Claude's Discretion
- Exact CSS/Tailwind classes for sidebar styling
- Whether to use a wrapper component or just conditional rendering for section display
- Internal file organization (one file per section or grouped)

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

SET-01 through SET-06 are not formally entered in REQUIREMENTS.md; they map 1:1 to the Phase 5 success criteria defined in ROADMAP.md plus one additional structural requirement inferred from the phase goal.

| ID | Description | Research Support |
|----|-------------|------------------|
| SET-01 | Settings page displays a sidebar with labeled items for all 6 sections; clicking shows only that section's content | SettingsSidebar component + conditional section render in parent; section key union type drives active state |
| SET-02 | Currently active section is visually distinguished in the sidebar | `border-l-2 border-purple-2 bg-charcoal-medium` on active item; matches Tabs.tsx active border pattern |
| SET-03 | Restart / unsaved-changes banner remains visible at all times regardless of active section | Banner rendered above sidebar+content row — not inside content area; controlled by `hasUnsavedChanges` prop lifted to parent |
| SET-04 | OAuth agent API key flow survives navigating between sidebar sections without losing listener | `listen('agent:oauth', ...)` stays in parent Settings component, not in AgentSection; parent never unmounts on section switch |
| SET-05 | Each settings section is an isolated component with no cross-section local state reads | Each section file imports only its own Tauri commands, receives Zustand selectors and callbacks as props |
| SET-06 | Settings page is restructured as sidebar-navigated layout (the structural decomposition goal itself) | 7 new files under `app/src/components/settings/`; Settings.tsx becomes the orchestrating shell |
</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19.x (project-established) | Component model, useState, useEffect, useCallback | Already in project |
| TypeScript | project-established | Prop interface contracts for section components | Already in project |
| Tailwind CSS | project-established | All styling via utility classes | Project-wide convention — no CSS modules |

[VERIFIED: codebase scan of `app/src/pages/Settings.tsx`, `app/tailwind.config.js`]

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `@tauri-apps/api/event` — `listen`, `UnlistenFn` | project-established | Tauri event channel subscriptions | OAuth listener in parent Settings; each section imports own Tauri commands via `../tauri` |
| Zustand (`useAppStore`, `useWalletStore`, `usePOAStore`) | project-established | Global persistent state | Sections read Zustand via selectors; no new Zustand state needed |
| viem (`formatEther`, `Address`) | project-established | Ethereum address formatting | WalletSection only |

[VERIFIED: `app/src/pages/Settings.tsx` imports]

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Local useState per section | Zustand per section | Zustand adds boilerplate; local state is correct for transient UI state per CONTEXT.md decision |
| Conditional rendering for sections | React Router sub-routes | Router adds URL coupling; CONTEXT.md explicitly rejected deep-linking |
| Wrapper component per section slot | `{activeSection === 'wallet' && <WalletSection .../>}` inline | Wrapper adds indirection with no benefit; inline conditional is the simpler approach |

**Installation:** No new packages required. All dependencies are already installed.

---

## Architecture Patterns

### Recommended Project Structure
```
app/src/
├── components/
│   └── settings/              # new directory (does not yet exist)
│       ├── SettingsSidebar.tsx
│       ├── WalletSection.tsx
│       ├── NodeSection.tsx
│       ├── EnvironmentSection.tsx
│       ├── AgentSection.tsx
│       ├── McpSection.tsx
│       └── ResetSection.tsx
└── pages/
    └── Settings.tsx           # becomes a ~100-line orchestrating shell
```

[VERIFIED: `app/src/components/settings/` does not exist yet — confirmed by directory listing]

### Pattern 1: Section Key Union Type + Parent Active State

**What:** Parent Settings holds `activeSection` as `useState<SectionKey>('wallet')`. SectionKey is a union type.
**When to use:** This entire phase.

```typescript
// In Settings.tsx (parent shell)
type SectionKey = 'wallet' | 'node' | 'environment' | 'agent' | 'mcp' | 'reset';

const [activeSection, setActiveSection] = useState<SectionKey>('wallet');
```

[VERIFIED: CONTEXT.md navigation decisions; consistent with Tabs.tsx `key: string` pattern]

### Pattern 2: Prop Interface per Section

**What:** Each section component receives only the state/callbacks it needs. No section touches another section's local state.
**When to use:** All 6 section components.

```typescript
// Example: NodeSection props interface
interface NodeSectionProps {
  wavsHome: string | null;
  onUnsavedChange: (hasChanges: boolean) => void;
}

export function NodeSection({ wavsHome, onUnsavedChange }: NodeSectionProps) {
  // owns: tomlContent, savedContent, tomlLoading, tomlError, tomlSaveSuccess
  // derived: hasUnsavedChanges = tomlContent !== savedContent
  // calls onUnsavedChange(hasUnsavedChanges) via useEffect on change
  ...
}
```

[VERIFIED: CONTEXT.md component architecture — hasUnsavedChanges lifted to parent via props]

### Pattern 3: OAuth Listener in Parent Shell, Callback to AgentSection

**What:** The `listen('agent:oauth', ...)` subscription stays in parent Settings so it is never torn down during section navigation. AgentSection receives loading/status state and a trigger callback from the parent.

**Critical insight from codebase:** The current `AgentApiKeyField` at lines 115–145 of Settings.tsx calls `listen('agent:oauth', ...)` in its own `useEffect`. This component is currently embedded directly inside Settings. When extracted to `AgentSection`, if AgentSection is unmounted on section switch, the listener is torn down — breaking OAuth flows that span a browser redirect. The fix: move the `listen` call to the parent Settings component, pass `oauthLoading`/`oauthStatus` state down as props, and give AgentSection an `onOAuthStart` callback to trigger the flow.

**What AgentSection STILL handles locally:** `apiKey`, `maskedKey`, `authType`, `saving`, `editing` — these are per-field state not shared across sections.

[VERIFIED: `app/src/pages/Settings.tsx` lines 114–145, CONTEXT.md OAuth decision]

### Pattern 4: SettingsSidebar as Pure Presentational Component

**What:** Sidebar receives items array (or hardcoded list), active key, and onChange callback. No internal state.

```typescript
// app/src/components/settings/SettingsSidebar.tsx
const SIDEBAR_ITEMS: { key: SectionKey; label: string }[] = [
  { key: 'wallet', label: 'Wallet' },
  { key: 'node', label: 'Node' },
  { key: 'environment', label: 'Environment' },
  { key: 'agent', label: 'Agent' },
  { key: 'mcp', label: 'MCP' },
  { key: 'reset', label: 'Reset' },
];

interface SettingsSidebarProps {
  activeSection: SectionKey;
  onSelect: (key: SectionKey) => void;
}
```

Active item classes: `text-beige-light font-semibold border-l-2 border-purple-2 bg-charcoal-medium`
Inactive item classes: `text-tan-muted font-normal hover:text-beige-warm hover:bg-charcoal-medium`

[VERIFIED: `app/tailwind.config.js` color tokens, `app/src/components/atoms/Tabs.tsx` active pattern, 05-UI-SPEC.md]

### Pattern 5: Parent Banner Above Layout Split

**What:** The restart/unsaved banner is rendered BEFORE the `flex` row that holds sidebar + content. It is controlled by `hasUnsavedChanges` boolean in the parent, which sections update via `onUnsavedChange(bool)` callbacks.

```typescript
// Settings.tsx parent shell layout
<div className="flex flex-col gap-0">
  {hasUnsavedChanges && (
    <div className="flex gap-4 mb-4 items-center p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <p className="text-lg text-beige-light flex-1">Restart for changes to take effect.</p>
      <Button text="Restart Application" color="red" onClick={handleRestart} />
    </div>
  )}
  <div className="flex flex-1 gap-0">
    <SettingsSidebar activeSection={activeSection} onSelect={setActiveSection} />
    <div className="flex-1 overflow-y-auto px-6 py-4 max-h-[calc(100vh-12rem)]">
      {activeSection === 'wallet' && <WalletSection ... />}
      {activeSection === 'node' && <NodeSection ... />}
      {/* etc. */}
    </div>
  </div>
</div>
```

[VERIFIED: 05-UI-SPEC.md layout contract, existing Settings.tsx banner at line 674]

### Anti-Patterns to Avoid

- **Moving the OAuth `listen()` call into AgentSection:** The listener would be torn down every time the user navigates away from Agent section, breaking in-flight OAuth flows.
- **Using Zustand for section UI state:** CONTEXT.md explicitly locked this as local useState only. Zustand is for global persistent app state.
- **Adding URL hash or query params for section tracking:** CONTEXT.md explicitly rejected deep-linking.
- **Sharing the `changed` state variable from the current monolith without lifting it:** The current `changed` bool in Settings.tsx is conflated with the TOML save state. In the refactor, `hasUnsavedChanges` in the parent must be a proper boolean that any section can set via its `onUnsavedChange` callback.
- **Mounting all 6 sections at once and using CSS display:none:** This would run all useEffects and all Tauri polls simultaneously, wasting resources. Use conditional rendering (`activeSection === 'x' &&`) instead.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Section isolation | Custom context or event bus | React prop drilling (callbacks) | Sections are shallow; prop drilling is correct at this depth |
| Active sidebar state | URL router with active detection | `useState<SectionKey>` in parent | Explicit, simple, matches CONTEXT.md decision |
| Sidebar styling | Custom CSS animation for transitions | Tailwind `transition-colors` on hover | Project pattern: all transitions via Tailwind utilities |

**Key insight:** This phase has no library gaps. Every pattern needed is already present in the codebase.

---

## Critical State Mapping

The current monolith has state that must be distributed correctly. This is the highest-risk area.

### State That Moves TO Sections

| State Variable(s) | Moves To | Notes |
|-------------------|----------|-------|
| `showMnemonic`, `exportedMnemonic`, `mnemonicCopied`, `showResetConfirm`, `balances` | WalletSection | Reads `hasMnemonic`, `isLoading`, `derivedAddresses`, `walletError` from useWalletStore |
| `tomlContent`, `savedContent`, `tomlLoading`, `tomlError`, `tomlSaveSuccess` | NodeSection | Reads `settings.wavs_home` from appStore; calls `setChanged(true)` via callback on save |
| `envVars`, `newEnvKey`, `newEnvValue`, `visibleEnvKeys`, `envSaving`, `envSaveSuccess`, `envError`, `newEnvValueRef` | EnvironmentSection | Reads `settings.saved_services`, `settings.env_vars` from appStore |
| `oauthLoading`, `oauthStatus` | **Parent Settings** (OAuth listener) + AgentSection (display only) | AgentSection receives oauthLoading/oauthStatus as props, calls onOAuthStart callback |
| `mcpStatus`, `mcpBinaryPath`, `wavsUrl`, `mcpAutoStart`, `mcpToken`, `mcpLoading`, `mcpError`, `claudeProjectPath`, `claudeRegisterResult`, `claudeRegisterLoading`, `claudeRegisterError` | McpSection | Reads `settings.mcp_auto_start`, `settings.mcp_token` from appStore |
| `showClearServicesConfirm` | ResetSection | — |

### State That STAYS in Parent Settings

| State Variable(s) | Reason |
|-------------------|--------|
| `changed` / `hasUnsavedChanges` (boolean) | Controls banner visibility; sections call `onUnsavedChange(bool)` |
| `oauthLoading`, `oauthStatus` | OAuth listener lives in parent — see Pattern 3 |
| `activeSection` | Drives sidebar navigation |
| `error` / `displayError` | Top-level error display; sections report errors via callbacks or display locally |
| `handleRestart` | Referenced by banner button |

[VERIFIED: Complete read of Settings.tsx lines 276–1221]

### `changed` Variable Semantics Change

The current `changed` variable in Settings.tsx (line 289) is set to `true` by `handleSaveToml` and `handleBrowse`, but never reset. In the new design, this should become `hasUnsavedChanges` (a proper boolean that sections toggle), or be replaced by tracking TOML dirty state from NodeSection via callback. The TOML `hasUnsavedChanges = tomlContent !== savedContent` (line 319) is the primary source of truth for the banner.

**Decision needed by planner (Claude's discretion):** Rename `changed` to `hasUnsavedChanges` in parent, driven by a single `onUnsavedChange(bool)` callback. NodeSection calls `onUnsavedChange(tomlContent !== savedContent)` via `useEffect`.

---

## Common Pitfalls

### Pitfall 1: OAuth Listener Torn Down on Section Switch

**What goes wrong:** If `AgentApiKeyField` is moved into `AgentSection` with the `listen('agent:oauth', ...)` call, and AgentSection unmounts when the user clicks another sidebar item, the listener cleanup function runs — unsubscribing from the event. An OAuth callback completing after the user navigated away would be silently dropped.

**Why it happens:** React's `useEffect` cleanup runs on component unmount. Conditional rendering (`activeSection === 'agent' && <AgentSection />`) unmounts AgentSection when a different section is selected.

**How to avoid:** The OAuth `listen` call must remain in the parent Settings component (which is never unmounted during section navigation). AgentSection receives `oauthLoading` and `oauthStatus` as props and calls an `onOAuthStart(provider)` prop to initiate the flow.

**Warning signs:** If you see the `useEffect` with `listen('agent:oauth', ...)` inside AgentSection, it's wrong.

[VERIFIED: Settings.tsx lines 114–145, React useEffect cleanup behavior — ASSUMED: standard React behavior]

### Pitfall 2: hasUnsavedChanges Stuck True After Section Switch

**What goes wrong:** NodeSection tracks `tomlContent !== savedContent`. If the user edits TOML (banner appears), then switches to Wallet section, the NodeSection unmounts — but `hasUnsavedChanges` in the parent stays `true`. This is correct behavior (changes are pending). However, if NodeSection also resets its state on unmount, the `tomlContent` is lost.

**How to avoid:** Do NOT reset section state on unmount. Use conditional rendering so the section re-mounts with fresh state from the Tauri store. The `hasUnsavedChanges` in the parent is set by NodeSection's `useEffect` watching `tomlContent`/`savedContent`; on remount, NodeSection re-fetches from Tauri (calling `readWavsToml()`), which resets both values to the same saved content — correctly clearing `hasUnsavedChanges` if the user navigated away without saving.

**Warning signs:** Banner persists after switching away from Node section even though no changes were made.

### Pitfall 3: Missing Prop Types on Section Interfaces

**What goes wrong:** TypeScript prop interfaces that are too loose (using `any` or optional props for actually-required data) create runtime errors when parent forgets to pass a prop.

**How to avoid:** All required data from parent (callbacks, Zustand-derived values, OAuth props) should be required (non-optional) in the prop interface. Optional props only for things with defaults.

### Pitfall 4: AgentApiKeyField Needs `oauthLoading`/`oauthStatus` Access

**What goes wrong:** The current `AgentApiKeyField` renders its loading/status UI based on its own local `oauthLoading`/`oauthStatus` state — which it updates from the OAuth listener. After the refactor, these must come from props (since the listener moves to parent).

**How to avoid:** Update `AgentApiKeyField` to accept `oauthLoading: boolean`, `oauthStatus: string | null`, and `onOAuthStart: () => void` as props instead of managing them locally. The component's internal `handleOAuthLogin` becomes a pass-through to the prop callback.

[VERIFIED: Settings.tsx lines 80–274 — full AgentApiKeyField implementation read]

---

## Code Examples

### Sidebar Item (active state)
```tsx
// Source: 05-UI-SPEC.md + tailwind.config.js color tokens
<button
  onClick={() => onSelect(item.key)}
  className={`w-full text-left px-3 py-2 text-sm transition-colors ${
    isActive
      ? 'text-beige-light font-semibold border-l-2 border-purple-2 bg-charcoal-medium'
      : 'text-tan-muted font-normal hover:text-beige-warm hover:bg-charcoal-medium border-l-2 border-transparent'
  }`}
>
  {item.label}
</button>
```

### Parent Shell Layout Skeleton
```tsx
// Source: 05-UI-SPEC.md layout contract
<div className="flex flex-col gap-0">
  {hasUnsavedChanges && (
    <div className="flex gap-4 mb-4 items-center p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <p className="text-lg text-beige-light flex-1">Restart for changes to take effect.</p>
      <Button text="Restart Application" color="red" onClick={handleRestart} />
    </div>
  )}
  <div className="flex flex-1 gap-0">
    <SettingsSidebar activeSection={activeSection} onSelect={setActiveSection} />
    <div className="flex-1 overflow-y-auto px-6 py-4 max-h-[calc(100vh-12rem)]">
      {activeSection === 'wallet' && <WalletSection ... />}
      {activeSection === 'node' && <NodeSection wavsHome={settings.wavs_home} onUnsavedChange={setHasUnsavedChanges} />}
      {activeSection === 'environment' && <EnvironmentSection ... />}
      {activeSection === 'agent' && <AgentSection oauthLoading={oauthLoading} oauthStatus={oauthStatus} onOAuthStart={handleOAuthStart} />}
      {activeSection === 'mcp' && <McpSection ... />}
      {activeSection === 'reset' && <ResetSection ... />}
    </div>
  </div>
</div>
```

### OAuth Listener in Parent Shell
```tsx
// Stays in Settings.tsx — NOT in AgentSection
// Source: Settings.tsx lines 114-145 — adapted for parent context
useEffect(() => {
  let unlisten: UnlistenFn | null = null;
  listen<{ type: string; url?: string; message?: string; provider?: string }>(
    'agent:oauth',
    (event) => {
      const data = event.payload;
      switch (data.type) {
        case 'open_url':   setOauthStatus('Waiting for browser authorization…'); break;
        case 'progress':   setOauthStatus(data.message ?? 'Working…'); break;
        case 'success':
          setOauthStatus(null); setOauthLoading(false);
          // AgentSection refreshes its own auth display via its own useEffect
          break;
        case 'error':
          setOauthStatus(null); setOauthLoading(false);
          Toast.error(data.message ?? 'OAuth login failed');
          break;
      }
    }
  ).then((fn) => { unlisten = fn; });
  return () => { unlisten?.(); };
}, []);
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 1,221-line monolithic Settings.tsx | 7-file decomposed settings module | This phase | Navigation isolation, maintainability |
| Vertical scroll through all sections | Sidebar navigation — one section at a time | This phase | Better discoverability, less scroll |
| `changed` boolean never reset | `hasUnsavedChanges` driven by section callbacks | This phase | Accurate banner visibility |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | NodeSection re-fetching from Tauri on remount resets dirty state correctly (hasUnsavedChanges clears) | Common Pitfalls #2 | Banner may persist after section switch; low risk — existing loadToml() pattern handles this |
| A2 | AgentSection being unmounted on section switch would break the OAuth listener | Common Pitfalls #1 | If React keeps unmounted subtrees alive (it does not in conditional render), this is moot. Standard React behavior confirmed via training |
| A3 | SET-01 through SET-06 map to the 5 success criteria + structural decomposition goal | Phase Requirements | If requirements have different scope, planner may miss something; low risk — derived directly from ROADMAP.md |

---

## Open Questions

1. **Does `AgentApiKeyField` remain a separate sub-component within AgentSection, or is it inlined?**
   - What we know: It's already encapsulated at lines 80–274 of Settings.tsx
   - What's unclear: Whether to keep it as a named sub-component in AgentSection.tsx or merge it
   - Recommendation: Keep it as a named function in `AgentSection.tsx` (same file) to preserve the encapsulation without creating a fourth nesting level

2. **Node section boundary: does "WAVS Home Directory" (Browse button) belong in NodeSection or stay in parent?**
   - What we know: WAVS Home browse calls `setWavsHome()` (Tauri command) and updates `settings.wavs_home` in appStore. TOML editor depends on `settings.wavs_home`.
   - What's unclear: CONTEXT.md groups Node as one section — it logically owns both home dir and TOML config
   - Recommendation: WAVS Home + TOML Editor both live in NodeSection. NodeSection calls `onUnsavedChange(true)` when home changes.

3. **`displayError` (line 669 of Settings.tsx) — where does this live in the new layout?**
   - What we know: It's `error || walletError` — a top-level error fallback
   - What's unclear: Whether to render it in the parent or in each section
   - Recommendation: Keep in parent as a fallback; individual sections show their own errors locally

---

## Environment Availability

Step 2.6: SKIPPED — This phase is a frontend-only React component refactor with no external CLI tools, databases, or services beyond the existing Tauri app dev infrastructure.

---

## Sources

### Primary (HIGH confidence)
- `app/src/pages/Settings.tsx` (full read, lines 1–1221) — complete state/handler inventory
- `app/src/components/atoms/Tabs.tsx` — active sidebar state pattern
- `app/tailwind.config.js` — all color tokens verified
- `app/src/stores/appStore.ts` — Zustand shape, settings fields
- `app/src/tauri/agent.ts` — OAuth commands, AgentAuthInfo type
- `.planning/phases/05-settings-decomposition/05-CONTEXT.md` — all locked decisions
- `.planning/phases/05-settings-decomposition/05-UI-SPEC.md` — layout contract, color/typography spec

### Secondary (MEDIUM confidence)
- `.planning/ROADMAP.md` — SET-* requirement IDs and success criteria

### Tertiary (LOW confidence)
- None.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in project, verified by direct file reads
- Architecture: HIGH — patterns derived directly from existing code and CONTEXT.md locked decisions
- Pitfalls: HIGH — OAuth pitfall verified by reading the actual listener code; unsaved-changes pitfall derived from current `changed` semantics

**Research date:** 2026-04-07
**Valid until:** 90 days (stable React patterns; no external library dependencies)
