# Phase 9: Foundation Types and Settings Refactor - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Update the frontend type system for BLS/P2P, add new Tauri command infrastructure, and decompose the monolithic Settings page into section components with visual polish. This is a foundation phase — no new user-facing features, just structural prerequisites for Phases 10-12.

</domain>

<decisions>
## Implementation Decisions

### Settings Reorganization
- **D-01:** Keep current section order: Wallet -> WAVS Home -> TOML Editor -> Env Variables -> MCP Server -> Reset App State
- **D-02:** Decompose Settings.tsx into section components — Claude decides component structure (one file per section vs folder, etc.) based on existing codebase patterns
- **D-03:** Each section manages its own state and calls Tauri commands directly — Claude decides the cleanest state management approach per section

### Settings Visual Polish
- **D-04:** Add a sticky left sidebar navigation that scrolls to sections (anchor nav pattern, similar to VS Code/GitHub Settings)
- **D-05:** Claude polishes visual hierarchy — consistent spacing, typography, section descriptions as needed — without changing the design language

### BLS Type Propagation
- **D-06:** Widen `SignatureAlgorithm` type to `'secp256k1' | 'bls12381'` and widen `SubmitDraft` type, but keep default as secp256k1. No UI selector — that's Phase 11
- **D-07:** Don't touch store builder/reverse logic for BLS yet — just the type definitions
- **D-08:** New Tauri commands (`cmd_get_p2p_status`, `cmd_get_service_signer`, `cmd_derive_bls_pubkey`) get TypeScript wrappers and types in commands.ts/types/index.ts, but no UI calls them in Phase 9

### Claude's Discretion
- Settings decomposition strategy (file structure, component boundaries)
- State management approach per section (self-contained vs prop-passing)
- Visual polish details (section descriptions, spacing, typography standardization)
- P2pStatus and SignerResponse TypeScript type shapes (must match backend Rust structs)
- Component file naming conventions

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Backend types (must match)
- `packages/types/src/signing.rs` — SignatureAlgorithm enum, SignatureKind, WavsSignature definitions
- `packages/types/src/service.rs` — Service, Workflow, Submit types that frontend mirrors
- `packages/wavs/src/http/handlers/` — HTTP API handlers that Tauri commands proxy to

### Frontend code being modified
- `app/src/types/index.ts` — Current frontend type definitions (SignatureAlgorithm on line 202)
- `app/src/pages/Settings.tsx` — 942-line monolith being decomposed
- `app/src/tauri/commands.ts` — Tauri command wrappers (new P2P/BLS commands go here)
- `app/src/stores/serviceBuilderStore.ts` — SubmitDraft.signatureAlgorithm type widening

### Tauri backend
- `app/src-tauri/src/commands.rs` — Rust-side Tauri command handlers (new commands added here)
- `app/src-tauri/src/state.rs` — App state for Tauri backend

### P2P status endpoint
- `packages/wavs/src/http/handlers/` — `/p2p/status` endpoint that `cmd_get_p2p_status` will proxy

### Codebase patterns
- `.planning/codebase/CONVENTIONS.md` — Naming and style conventions
- `.planning/codebase/STRUCTURE.md` — Where to add new code

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `atoms/Button` — Used throughout Settings for all actions
- `atoms/AddressDisplay` — Used in Wallet section for addresses
- `atoms/TomlEditor` — CodeMirror-based editor used in TOML section
- `stores/appStore` — Global settings state via Zustand
- `stores/walletStore` — Wallet state (mnemonic, addresses, balances)
- `stores/poaStore` — POA registry state
- `utils/error.ts` — `errorMessage()` helper for Tauri command errors
- `hooks/useViemClient` — `getPublicClient()` for balance fetching

### Established Patterns
- Tauri commands: `invoke<T>('cmd_name', { params })` with typed responses
- State: Zustand stores with actions, React local state for UI-only concerns
- Styling: Tailwind with custom tokens (charcoal-dark, beige-warm, tan-muted, etc.)
- Cards: `rounded-lg bg-charcoal-medium border border-charcoal-light` with `p-4 gap-4`
- Headers: `text-beige-light text-lg font-semibold`
- Scrollable pages: `max-h-[calc(100vh-12rem)] overflow-y-auto`

### Integration Points
- `App.tsx` — Route registration (Settings is at `/settings`)
- `components/layout/Header` — Nav links (sidebar nav will be within Settings page, not global)
- Settings page receives `settings` from `useAppStore` — section components will use the same store

</code_context>

<specifics>
## Specific Ideas

- Left sidebar nav should follow VS Code / GitHub Settings pattern — sticky on left, content scrolls on right
- Settings sections keep their current card styling — polish means consistency, not redesign

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 09-foundation-types-and-settings-refactor*
*Context gathered: 2026-03-23*
