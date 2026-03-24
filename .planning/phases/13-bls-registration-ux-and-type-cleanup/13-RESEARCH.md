# Phase 13: BLS Registration UX and Type Cleanup - Research

**Researched:** 2026-03-24
**Domain:** React/TypeScript frontend UX (Tauri desktop app)
**Confidence:** HIGH

## Summary

Phase 13 is a targeted gap-closure phase addressing two issues identified in the v1.2 milestone audit: (1) the BLS registration flow is unreachable when a service's POA registry is not added to the poaStore, and (2) the `SubmitDraft.signaturePrefix` field uses an inline `'eip191' | 'none'` union instead of the `SignaturePrefix` type alias. Both issues are LOW severity and purely frontend concerns.

The BLS registration UX gap is structural: `ServiceDetailPage.tsx` only checks BLS registration status when `registry !== null` (line 574). For services loaded from WAVS but not present as saved POA registries, the `registry` variable is null, so `checkBlsRegistrationStatus` never runs, `blsRegStatus` stays `'unknown'`, and the "Register BLS Key" button (gated on `blsRegStatus === 'unregistered'`) never appears. The fix is to show an informational hint/guidance banner in the BLS section when `serviceBls === true && registry === null`, explaining that the user needs to add the service manager as a POA registry before BLS key registration is possible.

The type drift fix is mechanical: widen `SignaturePrefix` from `'eip191'` to `'eip191' | 'none'`, then use it in `SubmitDraft.signaturePrefix` and the `SubmitEditor` local type.

**Primary recommendation:** Add a guidance banner in ServiceDetailPage's BLS section when no registry exists, and fix the SignaturePrefix type alias to include `'none'` so SubmitDraft and SubmitEditor can reference it directly.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BLS-03 | One-click BLS key registration on-chain (UX improvement) | ServiceDetailPage audit shows registry-null guard blocks registration flow; guidance banner pattern identified with exact insertion point at line 762-780 |
| FND-01 | SignaturePrefix type uses type alias instead of inline union | Type drift mapped across 3 files: types/index.ts (alias), serviceBuilderStore.ts (SubmitDraft), SubmitEditor.tsx (local SigPrefix) |
</phase_requirements>

## Standard Stack

### Core

No new libraries needed. This phase modifies existing React components and TypeScript types only.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19.1.0 | UI framework (already installed) | Project standard |
| TypeScript | 5.8.3 | Type system (already installed) | Project standard |
| Zustand | 5.0.0 | State management (already installed) | Project standard |
| TailwindCSS | 3.4.0 | Styling (already installed) | Project standard |

### Supporting

No new supporting libraries needed.

### Alternatives Considered

None -- this phase uses existing project infrastructure exclusively.

## Architecture Patterns

### Affected Files

```
app/src/
  types/index.ts                                    # SignaturePrefix type alias
  stores/serviceBuilderStore.ts                     # SubmitDraft interface
  components/service/SubmitEditor.tsx                # Local SigPrefix type
  pages/services/ServiceDetailPage.tsx               # BLS guidance banner
```

### Pattern 1: Guidance Banner for Missing Registry

**What:** An inline informational callout within the BLS Operator Key section that appears when `serviceBls === true && registry === null`. Guides the user to add the service manager address as a POA registry.

**When to use:** When a BLS service exists but no POA registry is connected for that address.

**Example:**
```typescript
// Inside ServiceDetailPage.tsx, within the BLS section (around line 762)
{serviceBls && !registry && (
  <div className="p-3 rounded bg-charcoal-dark border border-amber-700/50 mt-3 mb-1">
    <p className="text-amber-400 text-xs font-semibold mb-1">Registry Required for BLS Registration</p>
    <p className="text-tan-muted text-xs">
      To register your BLS key on-chain, add this service's contract address as a POA registry
      from the Services page. The registry enables on-chain key registration and operator management.
    </p>
  </div>
)}
```

### Pattern 2: Type Alias Consolidation

**What:** Widen the `SignaturePrefix` type alias to include all valid values, then reference it everywhere instead of inline unions.

**When to use:** Whenever a type is duplicated as an inline union in multiple locations.

**Example:**
```typescript
// types/index.ts -- change from:
export type SignaturePrefix = 'eip191';
// to:
export type SignaturePrefix = 'eip191' | 'none';

// stores/serviceBuilderStore.ts -- change from:
signaturePrefix: 'eip191' | 'none';
// to:
signaturePrefix: SignaturePrefix;

// components/service/SubmitEditor.tsx -- change from:
type SigPrefix = 'eip191' | 'none';
// to: import SignaturePrefix from types and use it directly
```

### Anti-Patterns to Avoid

- **Overengineering the guidance:** Do not add navigation links or auto-connect logic. A static text hint is sufficient for this gap closure.
- **Changing runtime behavior:** The `registry === null` guard in `checkBlsRegistrationStatus` is correct defensive code. Do not remove it or try to run BLS registration without a registry.
- **Breaking the SignatureKind.prefix nullability:** The `SignatureKind.prefix` field is `SignaturePrefix | null` where `null` means "no prefix". The `SubmitDraft.signaturePrefix` uses `'none'` as a UI-friendly representation that maps to `null` at build time. After widening `SignaturePrefix`, the mapping logic in `buildSubmit` (`draft.signaturePrefix === 'none' ? null : draft.signaturePrefix`) remains correct and must not change.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| N/A | N/A | N/A | This phase is purely editing existing code -- no new functionality to build |

## Common Pitfalls

### Pitfall 1: Breaking SignatureKind.prefix Serialization

**What goes wrong:** Changing `SignaturePrefix` to `'eip191' | 'none'` could cause someone to set `SignatureKind.prefix` to `'none'` instead of `null`, which the Rust backend would reject.
**Why it happens:** `'none'` in the UI draft maps to `null` in the API type. They are different representations.
**How to avoid:** The `buildSubmit` function in `serviceBuilderStore.ts` already handles this mapping at line 203: `prefix: draft.signaturePrefix === 'none' ? null : draft.signaturePrefix`. This logic must remain unchanged. The `SignatureKind` interface `prefix` field stays as `SignaturePrefix | null` (where `null` means no prefix). Only `SubmitDraft.signaturePrefix` uses the widened type with `'none'`.
**Warning signs:** If `tsc` shows errors in `buildSubmit` after the type change, the mapping is broken.

### Pitfall 2: Guidance Banner Appearing for Non-BLS Services

**What goes wrong:** The banner shows up on ECDSA service detail pages that have no registry.
**Why it happens:** Missing the `serviceBls` guard.
**How to avoid:** The condition must be `serviceBls && !registry`, not just `!registry`.
**Warning signs:** Banner visible on any service without a registry, regardless of algorithm.

### Pitfall 3: Guidance Banner Overlapping with Existing BLS Section

**What goes wrong:** Two BLS-related sections render simultaneously, creating visual confusion.
**Why it happens:** The existing BLS section at line 762 renders when `serviceBls` is true. The guidance banner also renders when `serviceBls` is true.
**How to avoid:** The guidance banner should render within or adjacent to the existing `{serviceBls && (...)}` block. The most natural placement is: when `serviceBls && !registry`, show the guidance banner instead of (or alongside) the operator key section. The key display still works without a registry (it loads via `getServiceSigner`), so the banner can appear below the key display.
**Warning signs:** Visual clutter or duplicate BLS sections in the UI.

## Code Examples

### Current ServiceDetailPage BLS Section (lines 762-780)

```typescript
{/* BLS Operator Key Section */}
{serviceBls && (
  <div className="p-3 rounded bg-charcoal-dark border border-charcoal-light mt-3 mb-1">
    {blsLoading ? (
      <p className="text-tan-muted text-xs">Loading operator key...</p>
    ) : (
      <>
        <div className="flex items-center justify-between mb-2">
          <span className="text-tan-muted text-xs font-semibold">BLS Operator Key</span>
          <RegistrationBadge status={blsRegStatus} />
        </div>
        {blsPubkey ? (
          <AddressDisplay address={`0x${blsPubkey}`} />
        ) : (
          <p className="text-red-3 text-xs">Failed to load BLS operator key.</p>
        )}
      </>
    )}
  </div>
)}
```

### Current Register BLS Key Button (lines 795-803)

```typescript
{serviceBls && blsRegStatus === 'unregistered' && (
  <Button
    text={blsRegistering ? 'Registering...' : 'Register BLS Key'}
    size="sm"
    color="purple"
    disabled={blsRegistering}
    onClick={handleBlsRegister}
  />
)}
```

### Current SignaturePrefix Type (types/index.ts:211)

```typescript
export type SignaturePrefix = 'eip191';
```

### Current SubmitDraft.signaturePrefix (serviceBuilderStore.ts:49)

```typescript
export interface SubmitDraft {
  type: 'none' | 'aggregator';
  component: ComponentDraft;
  signatureAlgorithm: SignatureAlgorithm;
  signaturePrefix: 'eip191' | 'none';  // Should use SignaturePrefix
}
```

### Current SubmitEditor Local Type (SubmitEditor.tsx:20)

```typescript
type SigPrefix = 'eip191' | 'none';
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `SignaturePrefix = 'eip191'` (single value) | Should be `'eip191' \| 'none'` | Phase 11 added BLS but didn't update alias | Low -- runtime correct, type safety gap |
| No guidance for registry requirement | Needs banner when BLS service lacks registry | Phase 11 audit found this gap | Low -- defensive code is correct, UX is incomplete |

## Open Questions

None. Both issues are well-characterized with clear fixes identified.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | None (no test framework configured for app/) |
| Config file | None |
| Quick run command | `cd app && npx tsc --noEmit` (type-check only) |
| Full suite command | `cd app && npx tsc --noEmit && npx vite build` (type-check + build) |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BLS-03 | Guidance banner renders when BLS service has no registry | manual-only | `cd app && npx tsc --noEmit` (verifies no type errors) | N/A -- no test framework |
| FND-01 | SubmitDraft.signaturePrefix uses SignaturePrefix type | unit (type check) | `cd app && npx tsc --noEmit` | N/A -- no test framework |

**Manual-only justification:** The app has no test framework (no jest/vitest/playwright). Both requirements can be verified by (1) TypeScript type checking (`tsc --noEmit`) for compile-time correctness and (2) visual inspection in the running app for the UX banner.

### Sampling Rate
- **Per task commit:** `cd /Users/jacobhartnell/Dev/projects/Layer/WAVS/app && npx tsc --noEmit`
- **Per wave merge:** `cd /Users/jacobhartnell/Dev/projects/Layer/WAVS/app && npx tsc --noEmit && npx vite build`
- **Phase gate:** Full type check + build green before `/gsd:verify-work`

### Wave 0 Gaps
None -- no test infrastructure to set up. TypeScript strict mode is already configured and serves as the automated validation layer.

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis of:
  - `app/src/pages/services/ServiceDetailPage.tsx` (866 lines)
  - `app/src/types/index.ts` (487 lines)
  - `app/src/stores/serviceBuilderStore.ts` (397 lines)
  - `app/src/components/service/SubmitEditor.tsx` (84 lines)
  - `app/src/stores/poaStore.ts` (131 lines)
  - `app/src/utils/evm.ts` (466 lines)
- `.planning/v1.2-MILESTONE-AUDIT.md` -- audit findings that motivated this phase

### Secondary (MEDIUM confidence)
- None needed -- all findings are from direct code reading

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new libraries, using existing project stack
- Architecture: HIGH -- exact line numbers identified for all changes, clear insertion points
- Pitfalls: HIGH -- runtime behavior well-understood from code reading, mapping logic verified

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable -- no external dependencies changing)
