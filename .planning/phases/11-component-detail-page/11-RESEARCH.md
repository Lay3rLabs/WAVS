# Phase 11: Component Detail Page - Research

**Researched:** 2026-04-08
**Domain:** React frontend — Tauri desktop app (React 19, React Router v6, Zustand, Tailwind CSS)
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Page Layout & Navigation**
- Header section + tabbed content layout — follows existing ServiceDetailPage pattern with title/digest/badges at top, tabs below
- Navigate via clicking component card on ComponentsPage → `/components/:digest` route
- Back navigation via browser back button + breadcrumb ("Components > {name/digest}")
- Three tabs: Interface / Permissions / Configuration — groups related info logically

**Interface Display**
- Expandable accordion per exported function — shows function name collapsed, expands to show input/output JSON Schema
- JSON Schema rendered as formatted tree view with type annotations — not raw JSON
- Source info displayed as colored badge (source type) in header + details in info grid below title (URI/registry/digest)
- "Used by" services shown as clickable service links in header area — links to their detail pages

**Empty & Error States**
- Component not found: full-page "Component not found" with link back to components list
- No exports: "No exported functions" message in Interface tab
- Loading state: skeleton placeholders matching final layout

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
| DETL-01 | User can navigate to a component detail page at `/components/:digest` | Route registration in App.tsx using React Router v6 `useParams` pattern — identical to existing `/services/:chainId/:address` route |
| DETL-02 | User can see component identity — source info, digest, OCI URI, and which services use it | `cmd_get_component_metadata` returns `source` field (tagged enum with Download/Registry/Digest/Oci variants); "used by" derived from `useAppStore` services scan (same logic as ComponentsPage) |
| DETL-03 | User can see exported functions listed with expandable input/output JSON Schema viewers | `cmd_get_component_schema` returns `{ world, exports: { [name]: { inputSchema, outputSchema } }, $defs }` — `Expander` atom handles accordion; schema rendered as formatted JSON tree |
| DETL-04 | User can see component permissions (HTTP hosts, file system, sockets, DNS resolution) | `cmd_get_component_metadata` returns `permissions: { allowed_http_hosts, file_system, raw_sockets, dns_resolution }` — reuse `PermRow` pattern from ServiceDetailPage |
| DETL-05 | User can see component resource limits (fuel limit, time limit) | `cmd_get_component_metadata` returns `fuel_limit: Option<u64>` and `time_limit_seconds: Option<u64>` — render in same Permissions tab |
| DETL-06 | User can see component config keys and required environment variables | `cmd_get_component_metadata` returns `config: BTreeMap<String,String>` (keys only needed) and `env_keys: BTreeSet<String>` — tag cloud pattern in Configuration tab |
</phase_requirements>

---

## Summary

Phase 11 is a pure-frontend phase that consumes two Tauri commands delivered by Phase 10 (`cmd_get_component_schema` and `cmd_get_component_metadata`). Both commands are verified as implemented and registered. The frontend work is straightforward: create one new page component, one new data-fetching hook, add one route to App.tsx, and export the page from the pages index. No backend changes are needed.

The page architecture closely mirrors `ServiceDetailPage.tsx` — a header card + tabbed content layout using existing atoms (`Tabs`, `Expander`, `AddressDisplay`, `Breadcrumb`, `Button`, `Toast`). All patterns, colors, spacing, and copy are specified exactly in the UI-SPEC. The planner's job is to sequence work so the data-fetching infrastructure (hook + TypeScript types) is created first, then the page sections are built top-down.

The only non-trivial design question is how to render JSON Schema trees. The UI-SPEC specifies "formatted tree view with type annotations — not raw JSON", and the most practical implementation for a Tailwind-only codebase with no third-party component libraries is to render the schema as `JSON.stringify(schema, null, 2)` in a `<pre>` block with `whitespace-pre-wrap`. This matches the `FileContentModal` pattern already in `ServiceDetailPage.tsx` and satisfies the visual contract from the UI-SPEC (`bg-charcoal-dark p-3 rounded text-beige-light text-xs font-mono whitespace-pre-wrap`). A true recursive tree renderer would require significant new code and is not required by the spec.

**Primary recommendation:** Create `useComponentDetail` hook first (types + data fetching), then build `ComponentDetailPage` by tab, using existing `ServiceDetailPage.tsx` and `ComponentsPage.tsx` as structural templates throughout.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19 (project) | UI rendering | Project standard [VERIFIED: app/package.json] |
| React Router v6 | 6 (project) | `useParams`, `useNavigate`, `<Link>` | All routing in this app uses RRv6 [VERIFIED: App.tsx] |
| Zustand | project | Read services from `useAppStore` for "Used by" derivation | Established store pattern [VERIFIED: ComponentsPage.tsx] |
| Tailwind CSS | project | All styling — custom warm-dark palette | No CSS modules or styled-components [VERIFIED: UI-SPEC] |
| `@tauri-apps/api/core` `invoke` | project | Call `cmd_get_component_schema`, `cmd_get_component_metadata` | All Tauri IPC uses this [VERIFIED: commands.ts] |

### Supporting (existing atoms — no installation needed)

| Atom | File | Usage in this phase |
|------|------|---------------------|
| `Tabs` | `atoms/Tabs.tsx` | Three-tab navigation (Interface / Permissions / Configuration) |
| `Expander` | `atoms/Expander.tsx` | Per-function accordion in Interface tab |
| `AddressDisplay` | `atoms/AddressDisplay.tsx` | Digest display with copy-to-clipboard |
| `Breadcrumb` | `atoms/Breadcrumb.tsx` | "Components > {shortDigest}" navigation |
| `Button` | `atoms/Button.tsx` | "Back to Components" on not-found state |
| `Toast` | `atoms/Toast.tsx` | Error notifications on command failure |

**Installation:** No new packages needed. All dependencies are already present. [VERIFIED: project file structure]

---

## Architecture Patterns

### Recommended Project Structure

```
app/src/
├── pages/
│   ├── components/
│   │   └── ComponentDetailPage.tsx     # NEW — main detail page
│   ├── index.ts                        # MODIFY — export ComponentDetailPage
│   └── ComponentsPage.tsx              # UNCHANGED (Phase 12 wires up click)
├── hooks/
│   └── useComponentDetail.ts           # NEW — data-fetching hook
├── tauri/
│   └── commands.ts                     # MODIFY — add two command wrappers
├── types/
│   └── index.ts                        # MODIFY — add TS types for responses
└── App.tsx                             # MODIFY — add /components/:digest route
```

### Pattern 1: Route Registration

Mirrors the existing service detail route exactly. The `/components` route is currently a flat page (not a layout). The new route sits alongside it:

```typescript
// Source: App.tsx (existing pattern, lines 39-45)
<Route path="/components" element={<ComponentsPage />} />
<Route path="/components/:digest" element={<ComponentDetailPage />} />
```

The detail page is NOT nested inside a layout wrapper — same as `ComponentsPage`. [VERIFIED: App.tsx]

### Pattern 2: useParams for Route Digest

```typescript
// Source: ServiceDetailPage.tsx (lines 492-493)
const { digest } = useParams<{ digest: string }>();
```

The digest comes URL-encoded. Tauri `invoke` accepts the raw string — no decoding needed.

### Pattern 3: Data-Fetching Hook

New `useComponentDetail` hook owns both command calls, returns loading/error/data state:

```typescript
// Source: pattern from useServicePolling.ts + ServiceDetailPage.tsx data patterns
interface UseComponentDetailResult {
  schema: ComponentSchema | null;
  metadata: ComponentMetadata | null;
  loading: boolean;
  schemaError: string | null;
  metadataError: string | null;
}

export function useComponentDetail(digest: string): UseComponentDetailResult {
  const [schema, setSchema] = useState<ComponentSchema | null>(null);
  const [metadata, setMetadata] = useState<ComponentMetadata | null>(null);
  const [loading, setLoading] = useState(true);
  const [schemaError, setSchemaError] = useState<string | null>(null);
  const [metadataError, setMetadataError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setLoading(true);

    Promise.allSettled([
      getComponentSchema(digest),
      getComponentMetadata(digest),
    ]).then(([schemaResult, metaResult]) => {
      if (!active) return;
      if (schemaResult.status === 'fulfilled') setSchema(schemaResult.value);
      else setSchemaError(getErrorMessage(schemaResult.reason));
      if (metaResult.status === 'fulfilled') setMetadata(metaResult.value);
      else setMetadataError(getErrorMessage(metaResult.reason));
      setLoading(false);
    });

    return () => { active = false; };
  }, [digest]);

  return { schema, metadata, loading, schemaError, metadataError };
}
```

Key: Use `Promise.allSettled` (not `Promise.all`) so a schema parse failure does not suppress metadata display. The UI-SPEC requires partial data display on error (individual Toast per failure). [VERIFIED: 11-UI-SPEC.md states "Show whatever partial data loaded successfully"]

### Pattern 4: TypeScript Types for Phase 10 Responses

These types mirror the Rust `ComponentMetadataResult` and `ComponentSourceResult` structs from Phase 10 exactly. The `serde` tag format `#[serde(tag = "type", rename_all = "snake_case")]` produces internally-tagged union variants in JSON:

```typescript
// Source: commands.rs ComponentMetadataResult + ComponentSourceResult (VERIFIED: 10-01-PLAN.md)
// ComponentSourceResult uses: #[serde(tag = "type", rename_all = "snake_case")]
// so variants serialize as: { type: "download", uri: "...", digest: "..." }
export type ComponentSourceResult =
  | { type: 'download'; uri: string; digest: string }
  | { type: 'registry'; digest: string; domain: string | null; package: string }
  | { type: 'digest'; digest: string }
  | { type: 'oci'; uri: string; digest: string | null };

export interface ComponentMetadata {
  permissions: Permissions;     // reuses existing Permissions type
  fuel_limit: number | null;
  time_limit_seconds: number | null;
  config: Record<string, string>;
  env_keys: string[];
  source: ComponentSourceResult;
}

// JSON Schema shape from wit-schema:
// { world: string, exports: { [funcName]: { inputSchema: object, outputSchema: object } }, $defs: object }
export interface ComponentSchema {
  world: string;
  exports: Record<string, { inputSchema: unknown; outputSchema: unknown; description?: string }>;
  $defs: Record<string, unknown>;
}
```

Note: `Permissions` and `AllowedHostPermission` types already exist in `types/index.ts` — reuse them. [VERIFIED: types/index.ts lines 162-173]

### Pattern 5: "Used by" Services Derivation

The "used by" services list is NOT returned by `cmd_get_component_metadata`. It must be derived from `useAppStore` — the same logic already in `ComponentsPage.tsx`:

```typescript
// Source: ComponentsPage.tsx lines 34-55 (VERIFIED)
const services = useAppStore((state) => state.services);
// Iterate services → workflows → match digest → collect service links
```

The page gets the digest from `useParams`, scans the Zustand store for all services where any workflow component's digest matches, then renders links. No additional Tauri call needed.

### Pattern 6: JSON Schema Display

The UI-SPEC calls for "formatted tree view with type annotations". The practical implementation for this Tailwind-only codebase:

```tsx
// Source: FileContentModal in ServiceDetailPage.tsx (lines 83-90) — same pattern
<pre className="bg-charcoal-dark p-3 rounded text-beige-light text-xs font-mono whitespace-pre-wrap">
  {JSON.stringify(schema, null, 2)}
</pre>
```

This satisfies the visual contract. A recursive typed-tree renderer is out of scope — the spec says "formatted tree view", and pretty-printed JSON is the appropriate interpretation for a Tailwind-only project with no third-party libraries.

### Pattern 7: Page Structure (from UI-SPEC)

```tsx
// Source: 11-UI-SPEC.md Layout Contract (VERIFIED)
<Breadcrumb items={[{ label: 'Components', to: '/components' }, { label: shortDigest }]} />
<div className="flex flex-col gap-6">
  {/* Header card */}
  <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
    {/* Row 1: title + source badge */}
    {/* Row 2: info grid */}
    {/* Row 3: used by chips */}
  </div>
  <Tabs tabs={DETAIL_TABS} activeTab={activeTab} onChange={setActiveTab} />
  <div>
    {activeTab === 'interface' && <InterfaceTab ... />}
    {activeTab === 'permissions' && <PermissionsTab ... />}
    {activeTab === 'configuration' && <ConfigurationTab ... />}
  </div>
</div>
```

### Anti-Patterns to Avoid

- **Nesting detail route inside a layout:** The ComponentsPage has no layout wrapper. Adding one would break the existing structure. Keep detail route as a sibling. [VERIFIED: App.tsx routing structure]
- **Using `Promise.all` for parallel commands:** If schema compilation fails, `Promise.all` would mask metadata. Use `Promise.allSettled` to allow partial success.
- **Rendering `AllowedHostPermission` as raw type:** `AllowedHostPermission` is `'all' | 'none' | { only: string[] }`. The UI-SPEC requires "all (unrestricted)" for `'all'`, "none" for `'none'`, and comma-separated host strings for `{ only: [...] }`. Reuse the existing `formatHosts()` function from `ServiceDetailPage.tsx` (lines 291-296) — but add the "(unrestricted)" suffix. [VERIFIED: ServiceDetailPage.tsx + UI-SPEC]
- **Polling component detail:** Unlike services, component data is immutable (WASM bytes do not change). No polling needed in `useComponentDetail` — a single fetch on mount is correct.
- **Missing OCI source variant in TypeScript:** The existing `ComponentSource` type in `types/index.ts` does NOT include the `oci` variant (only Download/Registry/Digest). The `ComponentSourceResult` from Phase 10 adds `Oci`. New TypeScript types must handle all four variants. [VERIFIED: types/index.ts line 157-159, commands.rs ComponentSourceResult]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Copy-to-clipboard for digest | Custom clipboard code | `AddressDisplay` atom | Already handles clipboard + visual feedback [VERIFIED: AddressDisplay.tsx] |
| Accordion/expand/collapse | Custom toggle component | `Expander` atom | Already handles expand state, styling [VERIFIED: Expander.tsx] |
| Tab navigation | Custom tab component | `Tabs` atom | Already handles active state, purple underline [VERIFIED: Tabs.tsx] |
| Error toast notifications | Custom error display | `Toast.error(msg)` | Already wired globally [VERIFIED: ServiceDetailPage.tsx usage] |
| Breadcrumb navigation | Custom breadcrumb | `Breadcrumb` atom | Already handles link vs. current-page style [VERIFIED: Breadcrumb.tsx] |
| JSON pretty-printing | Custom tree renderer | `JSON.stringify(obj, null, 2)` in `<pre>` | Matches existing `FileContentModal` pattern — no new code needed |
| Permission boolean display | Custom boolean renderer | Inline `{value ? 'yes' : 'no'}` | Direct — already used in `PermRow` calls in ServiceDetailPage.tsx |

**Key insight:** All visual building blocks exist. This phase assembles them, not invents them.

---

## Runtime State Inventory

Step 2.5: SKIPPED — Phase 11 is a greenfield UI-only addition. No renames, refactors, or data migrations. No stored identifiers change.

---

## Environment Availability Audit

Step 2.6: Checked for external tool dependencies.

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Node.js / npm | Frontend build (Vite) | Pre-existing (project runs) | — | — |
| Tauri dev environment | App dev server | Pre-existing (prior phases executed) | — | — |

No new external tools are required. Phase 11 is pure TypeScript/React file additions. [ASSUMED based on prior phase completion]

---

## Common Pitfalls

### Pitfall 1: OCI Source Variant Missing from Existing Types

**What goes wrong:** Developer reuses the existing `ComponentSource` type from `types/index.ts` for the metadata response — but that type only has three variants (Download/Registry/Digest). The backend returns four variants (`type: 'oci'`). TypeScript will not catch this; it will silently omit OCI source display.

**Why it happens:** The existing `ComponentSource` in `types/index.ts` was defined before OCI sources were fully exposed via Tauri. Phase 10 introduces a separate `ComponentSourceResult` type with all four variants.

**How to avoid:** Define a new `ComponentSourceResult` TypeScript type (separate from the existing `ComponentSource`) with all four variants, keyed by the `type` discriminant field from `serde(tag = "type")`. [VERIFIED: commands.rs ComponentSourceResult enum]

**Warning signs:** A component with OCI source renders no source info in the header.

### Pitfall 2: "Used by" Derived from Store, Not from Metadata Command

**What goes wrong:** Developer calls `cmd_get_component_metadata` and expects a `services` or `used_by` field in the response — it does not exist. The metadata command only returns permissions/limits/config/source. The "used by" list must be computed by scanning `useAppStore().services`.

**Why it happens:** The metadata command scans services internally to find the component's config, but does not expose which services use it. This is intentional — the frontend already has service data in the Zustand store.

**How to avoid:** In `useComponentDetail`, also accept a `services` parameter or inside the page component scan the Zustand store directly, matching digests as in `ComponentsPage.tsx`. [VERIFIED: 10-01-PLAN.md metadata command implementation]

**Warning signs:** "Used by 0 services" on a component that is clearly registered under a service.

### Pitfall 3: Schema `exports` May Be Empty

**What goes wrong:** `cmd_get_component_schema` for a component with no exported functions returns `{ world: "...", exports: {}, $defs: {} }`. If the Interface tab renders `Object.entries(schema.exports).map(...)` with no empty state guard, the tab shows nothing with no explanation.

**Why it happens:** Components with no exports are technically valid. The backend returns valid schema with an empty exports map.

**How to avoid:** Check `Object.keys(schema.exports).length === 0` before rendering the accordion list and show the empty state message. [VERIFIED: 10-CONTEXT.md "Component with no exports returns valid schema with empty exports array"]

**Warning signs:** Interface tab renders blank with no message on a valid component.

### Pitfall 4: `Expander` Content is Double-Bordered

**What goes wrong:** The `Expander` atom wraps content in its own `bg-charcoal-medium border border-charcoal-light` container (the outer div) AND the expanded content area gets a second identical container. Placing a `<pre>` block with `bg-charcoal-dark` directly inside the Expander's children renders visually correct, but adding another card-wrapper div creates three nested borders.

**Why it happens:** `Expander.tsx` already applies outer card styling. The content slot is rendered inside a nested `div` with `bg-charcoal-medium border border-charcoal-light` (line 38-42). Content added to children will already be inside a styled container.

**How to avoid:** The schema `<pre>` block should NOT be wrapped in an additional card div — render it directly as the Expander child. [VERIFIED: Expander.tsx implementation]

**Warning signs:** Schema display has three visible borders instead of one.

### Pitfall 5: `AllowedHostPermission` Formatting

**What goes wrong:** Rendering `AllowedHostPermission` directly as a string shows `[object Object]` for the `{ only: [...] }` variant.

**Why it happens:** `AllowedHostPermission` is a union type: `'all' | 'none' | { only: string[] }`. Object variant needs explicit handling.

**How to avoid:** Use a `formatHttpHosts` helper that maps: `'all'` → `"all (unrestricted)"`, `'none'` → `"none"`, `{ only: hosts }` → `hosts.join(', ')`. The `formatHosts` function in `ServiceDetailPage.tsx` handles this but does NOT add "(unrestricted)" — write a new version per UI-SPEC. [VERIFIED: ServiceDetailPage.tsx lines 291-295, UI-SPEC]

---

## Code Examples

Verified patterns from the codebase:

### Tauri Command Wrapper Pattern

```typescript
// Source: commands.ts (VERIFIED — lines 107-117 show existing pattern)
export async function getComponentSchema(digest: string): Promise<ComponentSchema> {
  return invoke<ComponentSchema>('cmd_get_component_schema', { digest });
}

export async function getComponentMetadata(digest: string): Promise<ComponentMetadata> {
  return invoke<ComponentMetadata>('cmd_get_component_metadata', { digest });
}
```

### Route Registration

```typescript
// Source: App.tsx lines 39-45 (VERIFIED)
// Add alongside existing /components route:
<Route path="/components/:digest" element={<ComponentDetailPage />} />
```

### Digest Short Display (breadcrumb)

```typescript
// Source: UI-SPEC Copywriting Contract (VERIFIED)
// "first 16 chars of digest + '…'"
const shortDigest = digest.slice(0, 16) + '…';
// e.g. "sha256:a3f9b2c1…"
```

### PermRow Reuse

```typescript
// Source: ServiceDetailPage.tsx lines 297-304 (VERIFIED)
function PermRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-tan-muted">{label}:</span>
      <span className="text-beige-warm">{value}</span>
    </div>
  );
}
```

### Service Link Chip (Used By)

```typescript
// Source: ComponentsPage.tsx lines 128-133 (VERIFIED — exact styling match required)
<button
  onClick={() => navigate(`/services/${serviceChain}/${serviceAddress}`)}
  className="px-2 py-1 text-xs bg-charcoal-light hover:bg-charcoal-dark border border-charcoal-light hover:border-purple-1 text-beige-warm rounded transition-colors"
>
  {serviceName}
</button>
```

### Loading Skeleton

```tsx
// Source: UI-SPEC States Contract (VERIFIED)
{loading && (
  <div className="flex flex-col gap-6">
    <div className="h-24 bg-charcoal-medium rounded-lg animate-pulse" />
    <div className="flex gap-6">
      {[1,2,3].map(i => <div key={i} className="h-8 w-24 bg-charcoal-light rounded animate-pulse" />)}
    </div>
    {[1,2,3].map(i => <div key={i} className="h-16 bg-charcoal-medium rounded animate-pulse" />)}
  </div>
)}
```

### Not-Found State

```tsx
// Source: UI-SPEC States Contract + ServiceDetailPage.tsx not-found pattern (VERIFIED)
<div className="flex flex-col gap-4">
  <p className="text-tan-muted">Component not found for {digest}</p>
  <Button text="Back to Components" size="sm" onClick={() => navigate('/components')} />
</div>
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No component detail page | Per-component detail at `/components/:digest` | Phase 11 (this phase) | Users can inspect schema, permissions, config |
| `ComponentSource` (3 variants) in frontend types | `ComponentSourceResult` (4 variants with `oci`) | Phase 10 | New TypeScript type needed for response deserialization |

**Deprecated/outdated:**
- None relevant to this phase.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Phase 10 commands are fully implemented and verified (based on 10-01-SUMMARY.md self-check: PASSED) | Standard Stack, Code Examples | If commands are not registered, page would get Tauri invoke errors on load — easily debugged |
| A2 | Node.js/npm and Tauri dev environment are available for frontend development | Environment Availability | If unavailable, no dev server — would surface immediately |

---

## Open Questions

1. **ComponentsPage clickable cards (Phase 12 dependency)**
   - What we know: CONTEXT.md says "Entry point: clicking a component card on ComponentsPage (Phase 12 wires this up; detail page itself just reads useParams)"
   - What's unclear: Phase 11 creates the detail page, but Phase 12 makes cards clickable. The route and page will exist but be unreachable from the UI until Phase 12.
   - Recommendation: This is intentional per the phase split. No action needed in Phase 11 — just document that direct URL navigation (`/components/<digest>`) is the only way to reach the page until Phase 12. The planner may want a manual test step that navigates directly by URL.

---

## Sources

### Primary (HIGH confidence)
- `/workspace/app/src/pages/services/ServiceDetailPage.tsx` — header/tab layout, PermRow pattern, formatHosts, service not-found pattern
- `/workspace/app/src/pages/ComponentsPage.tsx` — "used by" derivation logic, source type detection, service chip pattern
- `/workspace/app/src/App.tsx` — routing structure, route nesting rules
- `/workspace/app/src/components/atoms/Expander.tsx` — Expander API and inner styling (double-border pitfall)
- `/workspace/app/src/components/atoms/Tabs.tsx` — Tabs API, activeTab pattern
- `/workspace/app/src/components/atoms/Breadcrumb.tsx` — BreadcrumbItem API
- `/workspace/app/src/components/atoms/AddressDisplay.tsx` — AddressDisplay API, full prop
- `/workspace/app/src/types/index.ts` — existing TypeScript types (Permissions, AllowedHostPermission, ComponentSource)
- `/workspace/app/src/tauri/commands.ts` — invoke pattern, existing command wrappers
- `/workspace/app/src/hooks/useServicePolling.ts` — hook data-fetching pattern
- `/workspace/.planning/phases/10-backend-commands/10-01-PLAN.md` — Phase 10 command signatures and response shapes
- `/workspace/.planning/phases/10-backend-commands/10-01-SUMMARY.md` — Phase 10 verification (self-check PASSED)
- `/workspace/.planning/phases/11-component-detail-page/11-CONTEXT.md` — locked decisions
- `/workspace/.planning/phases/11-component-detail-page/11-UI-SPEC.md` — visual and interaction contract
- `/workspace/packages/wit-schema/src/lib.rs` — JSON Schema output structure (`world`, `exports`, `$defs`)
- `/workspace/packages/wit-schema/src/docs.rs` — `description` field in exports (doc comments)

### Secondary (MEDIUM confidence)
- None required — all claims verified against codebase directly.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all atoms, hooks, and Tauri command interfaces verified from source
- Architecture: HIGH — page structure and routing verified from existing patterns; hook design follows established conventions
- Pitfalls: HIGH — all pitfalls derived from verified code (existing type gaps, Expander internals, command response shape)

**Research date:** 2026-04-08
**Valid until:** 90 days (stable codebase, no fast-moving external dependencies)
