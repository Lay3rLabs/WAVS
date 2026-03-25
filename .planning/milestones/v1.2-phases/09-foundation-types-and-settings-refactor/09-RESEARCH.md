# Phase 9: Foundation Types and Settings Refactor - Research

**Researched:** 2026-03-23
**Domain:** Frontend type system widening, Tauri IPC commands, React component decomposition
**Confidence:** HIGH

## Summary

Phase 9 is a pure structural phase: widen frontend types for BLS/P2P support, add three new Tauri IPC commands (with no UI consumers yet), and decompose the 942-line Settings.tsx monolith into section components with a sticky sidebar navigation. No new user-facing features are introduced; the goal is laying clean foundations for Phases 10-12.

The backend infrastructure for all three new Tauri commands already exists. `P2pStatus` is defined in `packages/types/src/http.rs` and served at `GET /p2p/status`. `SignerResponse` is defined in the same file and served at `POST /services/signer`. BLS key derivation uses `utils::bls_signing::bls_private_key_from_mnemonic()` and `bls_g1_pubkey_bytes()`. The frontend work is pure type widening and IPC wiring -- no Rust-side feature implementation is required (though `cmd_derive_bls_pubkey` needs a new Tauri command handler that calls the existing utility).

The Settings decomposition is the largest surface area. The current file has 6 distinct sections (Wallet, WAVS Home, TOML Editor, Env Variables, MCP Server, Reset App State) with ~30 useState hooks and multiple useEffect blocks. Each section is self-contained in terms of state and side effects, making extraction straightforward with minimal prop drilling.

**Primary recommendation:** Split into three work streams: (1) type widening + store updates (pure type changes, zero runtime behavior change), (2) three new Tauri commands with TS wrappers, (3) Settings decomposition with sidebar nav.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Keep current section order: Wallet -> WAVS Home -> TOML Editor -> Env Variables -> MCP Server -> Reset App State
- **D-02:** Decompose Settings.tsx into section components -- Claude decides component structure (one file per section vs folder, etc.) based on existing codebase patterns
- **D-03:** Each section manages its own state and calls Tauri commands directly -- Claude decides the cleanest state management approach per section
- **D-04:** Add a sticky left sidebar navigation that scrolls to sections (anchor nav pattern, similar to VS Code/GitHub Settings)
- **D-05:** Claude polishes visual hierarchy -- consistent spacing, typography, section descriptions as needed -- without changing the design language
- **D-06:** Widen `SignatureAlgorithm` type to `'secp256k1' | 'bls12381'` and widen `SubmitDraft` type, but keep default as secp256k1. No UI selector -- that's Phase 11
- **D-07:** Don't touch store builder/reverse logic for BLS yet -- just the type definitions
- **D-08:** New Tauri commands (`cmd_get_p2p_status`, `cmd_get_service_signer`, `cmd_derive_bls_pubkey`) get TypeScript wrappers and types in commands.ts/types/index.ts, but no UI calls them in Phase 9

### Claude's Discretion
- Settings decomposition strategy (file structure, component boundaries)
- State management approach per section (self-contained vs prop-passing)
- Visual polish details (section descriptions, spacing, typography standardization)
- P2pStatus and SignerResponse TypeScript type shapes (must match backend Rust structs)
- Component file naming conventions

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FND-01 | `SignatureAlgorithm` type updated to include `'bls12381'` alongside `'secp256k1'` in frontend types | Backend `SignatureAlgorithm` enum in `packages/types/src/service.rs:565` already has both variants with `#[serde(rename_all = "snake_case")]`. Frontend type at `app/src/types/index.ts:202` currently hardcoded to `'secp256k1'`. Also widen `SubmitDraft.signatureAlgorithm` in `serviceBuilderStore.ts:47`. |
| FND-02 | New Tauri commands for P2P status, service signer info, and BLS key derivation | All three backend APIs exist: `GET /p2p/status` (handlers/p2p.rs), `POST /services/signer` (handlers/service/key.rs), `bls_private_key_from_mnemonic` + `bls_g1_pubkey_bytes` (utils/bls_signing.rs). New Tauri command handlers needed in `app/src-tauri/src/commands.rs`, registered in `lib.rs`. |
| FND-03 | `P2pStatus` and `SignerResponse` TypeScript types matching backend Rust structs | `P2pStatus` struct at `packages/types/src/http.rs:134` has 6 fields. `SignerResponse` enum at `packages/types/src/http.rs:13` has two tagged variants (Secp256k1, Bls12381). Both use `#[serde(rename_all = "snake_case")]` for JSON serialization. |
| FND-04 | Settings.tsx decomposed from monolithic 940-line file into section components | Current file has 6 sections, each self-contained. Codebase pattern: `pages/services/` uses one file per sub-page with barrel `index.ts`. Recommendation: `pages/settings/` directory with one component per section. |
| SET-01 | Settings page reorganized into logical sections with clear visual hierarchy | Sticky left sidebar nav (D-04) provides section navigation. Each section becomes its own component with consistent card styling. |
| SET-02 | Visual polish -- consistent spacing, typography, and component styling across all settings sections | Established design tokens: `bg-charcoal-medium border border-charcoal-light rounded-lg p-4 gap-4` for cards, `text-beige-light text-lg font-semibold` for headers, `text-tan-muted text-xs` for descriptions. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19 | UI framework | Already in use -- app/package.json |
| Zustand | (existing) | State management | Already used for appStore, walletStore, serviceBuilderStore |
| Tauri IPC | @tauri-apps/api/core | Frontend-backend communication | Already in use -- invoke pattern established |
| Tailwind CSS | (existing) | Styling | Already in use with custom design tokens |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| viem | (existing) | EVM address/balance utilities | Already used in Settings for wallet display |
| react-router-dom | (existing) | Page routing | Settings route already at `/settings` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Scroll-to-anchor sidebar | React Router nested routes for settings | Over-engineering for within-page navigation; anchor pattern is simpler and matches VS Code/GitHub Settings UX |
| Shared settings context provider | Individual Zustand store access per section | Context provider would add unnecessary coupling; sections already use `useAppStore` directly |

**Installation:**
No new packages required. All dependencies already present.

## Architecture Patterns

### Recommended Project Structure
```
app/src/
  pages/
    settings/
      Settings.tsx              # Main container: sidebar nav + content scroll area
      WalletSection.tsx         # Wallet management (accounts, balances, export, reset)
      WavsHomeSection.tsx       # WAVS home directory picker
      TomlEditorSection.tsx     # wavs.toml TOML editor
      EnvVariablesSection.tsx   # WAVS_ENV_* environment variable management
      McpServerSection.tsx      # MCP server controls and Claude Code registration
      ResetAppSection.tsx       # Clear services & registries
      index.ts                  # Re-exports Settings (main container)
  types/
    index.ts                    # Add P2pStatus, SignerResponse, widen SignatureAlgorithm
  tauri/
    commands.ts                 # Add getP2pStatus, getServiceSigner, deriveBlsPubkey
  stores/
    serviceBuilderStore.ts      # Widen SubmitDraft.signatureAlgorithm type
```

**Rationale for directory structure:**
- Follows established codebase pattern from `pages/services/` (one file per sub-page, barrel index.ts)
- Each section file is self-contained (state + effects + render)
- Main `Settings.tsx` is the layout container (sidebar + scrollable content)
- PascalCase filenames match existing convention (Button.tsx, AddressDisplay.tsx, ServiceListPage.tsx)

### Pattern 1: Settings Section Component

**What:** Each settings section is a standalone React component that manages its own state.
**When to use:** For all 6 settings sections being extracted.
**Example:**
```typescript
// Source: Derived from existing Settings.tsx patterns
import { useState, useEffect } from 'react';
import { useAppStore } from '../../stores/appStore';
import { Button } from '../../components/atoms';
import { someCommand } from '../../tauri';
import { errorMessage } from '../../utils/error';

interface WavsHomeSectionProps {
  onChanged: () => void;  // Notify parent of changes requiring restart
}

export function WavsHomeSection({ onChanged }: WavsHomeSectionProps) {
  const settings = useAppStore((state) => state.settings);
  const [error, setError] = useState<string | null>(null);

  // Section-local state and handlers...

  return (
    <div id="wavs-home" className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h2 className="text-beige-light text-lg font-semibold">WAVS Home Directory</h2>
      {/* Section content */}
    </div>
  );
}
```

### Pattern 2: Sticky Sidebar Navigation

**What:** Left sidebar with section links that scroll to anchored sections.
**When to use:** Settings page main container.
**Example:**
```typescript
// Source: VS Code / GitHub Settings anchor nav pattern
const SECTIONS = [
  { id: 'wallet', label: 'Wallet' },
  { id: 'wavs-home', label: 'WAVS Home' },
  { id: 'toml-editor', label: 'Configuration' },
  { id: 'env-vars', label: 'Environment Variables' },
  { id: 'mcp-server', label: 'MCP Server' },
  { id: 'reset', label: 'Reset App State' },
] as const;

export function Settings() {
  const [activeSection, setActiveSection] = useState('wallet');

  const scrollToSection = (id: string) => {
    document.getElementById(id)?.scrollIntoView({ behavior: 'smooth' });
    setActiveSection(id);
  };

  return (
    <div className="flex gap-6 max-h-[calc(100vh-12rem)]">
      {/* Sticky sidebar */}
      <nav className="w-48 shrink-0 sticky top-0 self-start flex flex-col gap-1">
        {SECTIONS.map(({ id, label }) => (
          <button
            key={id}
            onClick={() => scrollToSection(id)}
            className={`text-left px-3 py-2 rounded text-sm transition-colors ${
              activeSection === id
                ? 'text-beige-light bg-charcoal-medium'
                : 'text-tan-muted hover:text-beige-warm'
            }`}
          >
            {label}
          </button>
        ))}
      </nav>
      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto pr-2 flex flex-col gap-6">
        <WalletSection />
        <WavsHomeSection onChanged={() => setChanged(true)} />
        {/* ... */}
      </div>
    </div>
  );
}
```

### Pattern 3: Tauri Command Wrapper (TypeScript)

**What:** Typed async function wrapping `invoke<T>()` for new commands.
**When to use:** For `getP2pStatus`, `getServiceSigner`, `deriveBlsPubkey`.
**Example:**
```typescript
// Source: Existing pattern from app/src/tauri/commands.ts
import { invoke } from '@tauri-apps/api/core';
import type { P2pStatus, SignerResponse, ServiceManager } from '../types';

export async function getP2pStatus(): Promise<P2pStatus> {
  return invoke<P2pStatus>('cmd_get_p2p_status');
}

export async function getServiceSigner(serviceManager: ServiceManager): Promise<SignerResponse> {
  return invoke<SignerResponse>('cmd_get_service_signer', { service_manager: serviceManager });
}

export async function deriveBlsPubkey(hdIndex: number): Promise<BlsPubkeyResponse> {
  return invoke<BlsPubkeyResponse>('cmd_derive_bls_pubkey', { hd_index: hdIndex });
}
```

### Pattern 4: Tauri Command Handler (Rust)

**What:** Rust-side Tauri command that proxies to existing dispatcher/HTTP methods.
**When to use:** For new backend commands.
**Example:**
```rust
// Source: Follows existing cmd_get_health_status pattern in commands.rs
#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_p2p_status(
    wavs_instance: State<'_, WavsInstanceState>,
) -> AppResult<wavs_types::P2pStatus> {
    let dispatcher = wavs_instance.dispatcher()?;
    Ok(dispatcher.aggregator.get_p2p_status().await)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_service_signer(
    wavs_instance: State<'_, WavsInstanceState>,
    service_manager: ServiceManager,
) -> AppResult<wavs_types::SignerResponse> {
    let service_id = ServiceId::from(&service_manager);
    wavs_instance
        .dispatcher()?
        .get_service_signer(service_id)
        .map_err(|e| AppError::Service(e.to_string()))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_derive_bls_pubkey(
    mnemonic_cache: State<'_, MnemonicCacheState>,
    hd_index: u32,
) -> AppResult<BlsPubkeyResponse> {
    let mnemonic = get_mnemonic_cached(&mnemonic_cache)
        .ok_or_else(|| AppError::Keychain("No mnemonic found".to_string()))?;
    let key = utils::bls_signing::bls_private_key_from_mnemonic(&mnemonic.to_string(), hd_index)
        .map_err(|e| AppError::Service(format!("BLS key derivation failed: {}", e)))?;
    let g1_bytes = utils::bls_signing::bls_g1_pubkey_bytes(&key)
        .map_err(|e| AppError::Service(format!("G1 pubkey derivation failed: {}", e)))?;
    Ok(BlsPubkeyResponse {
        g1_pubkey_hex: const_hex::encode(g1_bytes),
    })
}
```

### Anti-Patterns to Avoid

- **Passing all settings as props:** Each section should access `useAppStore` directly, not receive settings through prop drilling. The store is the single source of truth.
- **Shared useState across sections:** The restart-needed banner is the ONLY cross-section state. All other state (toml content, mcp status, env vars, etc.) stays section-local.
- **BLS crypto in JavaScript:** All BLS operations MUST remain in Rust backend. The `cmd_derive_bls_pubkey` command returns hex strings. No JS BLS library.
- **Widening builder logic in Phase 9:** Decision D-07 explicitly says don't touch the `buildSubmit`/`reverseSubmit` logic yet. Only widen the type definition.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Scroll-to-section navigation | Custom scroll observer with IntersectionObserver | Simple `scrollIntoView({ behavior: 'smooth' })` + click handler | The sidebar is purely navigational; active-section tracking via IntersectionObserver adds complexity for minimal UX gain in a page with 6 sections |
| BLS key derivation | JavaScript BLS library | Rust `cmd_derive_bls_pubkey` Tauri command | Security: BLS private key must never leave the Rust process |
| Type generation from Rust | Manual TypeScript type definitions | Read Rust struct definitions and translate manually | The project already manually mirrors types (see `types/index.ts`). The Rust codebase has `ts-bindings` feature flag but it's not used in the Tauri app flow. Manual mirroring is the established pattern. |

**Key insight:** This phase is about wiring and decomposition, not building new systems. Every backend capability already exists; the work is connecting it to the frontend type system and making the Settings page maintainable.

## Common Pitfalls

### Pitfall 1: Serde Tagging Mismatch for SignerResponse
**What goes wrong:** TypeScript type doesn't match Rust's serde serialization format, causing runtime deserialization failures.
**Why it happens:** `SignerResponse` in Rust uses `#[serde(rename_all = "snake_case")]` as an internally tagged enum. The JSON output is `{"secp256k1": {"hd_index": 0, "evm_address": "0x..."}}` (adjacently tagged by default with no explicit tag attribute -- actually it uses Serde's default external tagging).
**How to avoid:** The Rust `SignerResponse` enum has NO `#[serde(tag = "...")]` attribute, so Serde uses **externally tagged** format: `{"secp256k1": {"hd_index": 0, "evm_address": "0x..."}}`. The TypeScript type must use discriminated union with object keys, not a `type` field.
**Warning signs:** `invoke<SignerResponse>()` returns `undefined` fields or throws parse errors.

### Pitfall 2: SubmitDraft Type Widening Breaking Builder Logic
**What goes wrong:** Changing `signatureAlgorithm: 'secp256k1'` to `signatureAlgorithm: SignatureAlgorithm` (union) causes TypeScript errors in `buildSubmit()` and `reverseSubmit()` because those functions use the value directly.
**Why it happens:** Decision D-07 says "don't touch store builder/reverse logic." But the type widening in `SubmitDraft` may cause TypeScript to infer wider types in downstream usage.
**How to avoid:** Widen only the type definition (`signatureAlgorithm: SignatureAlgorithm`). The `createDefaultSubmit()` function still defaults to `'secp256k1'`. The `buildSubmit()` function passes `draft.signatureAlgorithm` directly to `SignatureKind.algorithm`, which is already typed as `SignatureAlgorithm` in the `types/index.ts` after widening. This should be seamless.
**Warning signs:** TypeScript compilation errors in `serviceBuilderStore.ts`.

### Pitfall 3: Breaking Settings Page During Decomposition
**What goes wrong:** Extracting sections introduces bugs because shared state or side effects are missed during extraction.
**Why it happens:** The current file has 30+ useState hooks and 6 useEffect blocks. Dependencies between sections are subtle.
**How to avoid:** Map all state dependencies BEFORE extracting:
  - Cross-section state: `changed` (restart banner) -- must be lifted to parent
  - `displayError` combines `error` and `walletError` -- wallet error comes from `useWalletStore`, section error is local
  - Each section's state is otherwise independent
  - Extract one section at a time, verify the page renders identically after each extraction
**Warning signs:** State not updating, effects running at wrong time, restart banner not showing.

### Pitfall 4: cmd_derive_bls_pubkey Missing from lib.rs Registration
**What goes wrong:** New Tauri command works in Rust compilation but fails at runtime with "command not found."
**Why it happens:** Tauri requires explicit command registration in `tauri::generate_handler![]` macro in `lib.rs`.
**How to avoid:** For each new command: (1) add handler in `commands.rs`, (2) add to import list in `lib.rs`, (3) add to `generate_handler![]` macro. All three steps are required.
**Warning signs:** Frontend `invoke()` call rejects with Tauri error about unknown command.

### Pitfall 5: P2pStatus Available Only When WAVS Is Running
**What goes wrong:** `cmd_get_p2p_status` fails when WAVS node hasn't started yet.
**Why it happens:** The command needs the dispatcher, which only exists after `cmd_start_wavs`. This is fine for Phase 9 (no UI calls), but Phase 10 will need to handle this gracefully.
**How to avoid:** The Tauri command should use `wavs_instance.dispatcher()?` which returns `AppError::WavsNotRunning` when the node isn't started. The TS wrapper should handle this error. Document this behavior in the type comments.
**Warning signs:** N/A for Phase 9 (no UI consumers), but worth noting for downstream phases.

## Code Examples

### TypeScript Type Definitions (must match backend)

```typescript
// Source: packages/types/src/service.rs:565 (Rust)
// #[serde(rename_all = "snake_case")]
export type SignatureAlgorithm = 'secp256k1' | 'bls12381';

// Source: packages/types/src/http.rs:134 (Rust)
// #[derive(Debug, Clone, Default, Serialize, Deserialize)]
export interface P2pStatus {
  enabled: boolean;
  local_peer_id: string | null;
  listen_addresses: string[];
  connected_peers: number;
  peer_ids: string[];
  subscribed_services: string[];
}

// Source: packages/types/src/http.rs:13 (Rust)
// #[serde(rename_all = "snake_case")] -- externally tagged enum
export type SignerResponse =
  | { secp256k1: { hd_index: number; evm_address: string } }
  | { bls12381: { hd_index: number; g1_pubkey_hex: string } };

// New type for BLS pubkey derivation response
export interface BlsPubkeyResponse {
  g1_pubkey_hex: string;
}
```

### Widened SubmitDraft (serviceBuilderStore.ts)

```typescript
// Source: Current code at serviceBuilderStore.ts:44-49
export interface SubmitDraft {
  type: 'none' | 'aggregator';
  component: ComponentDraft;
  signatureAlgorithm: SignatureAlgorithm;  // Was: 'secp256k1' (hardcoded)
  signaturePrefix: 'eip191' | 'none';
}

function createDefaultSubmit(): SubmitDraft {
  return {
    type: 'none',
    component: createDefaultComponent(),
    signatureAlgorithm: 'secp256k1',  // Default unchanged
    signaturePrefix: 'eip191',
  };
}
```

### Settings Section Extraction Pattern

```typescript
// Source: Extracted from Settings.tsx wallet section (lines 486-585)
import { useState, useEffect } from 'react';
import { formatEther, type Address } from 'viem';
import { AddressDisplay, Button } from '../../components/atoms';
import { useWalletStore } from '../../stores/walletStore';
import { getPublicClient } from '../../hooks/useViemClient';
import { getChainConfigs } from '../../tauri';

interface WalletSectionProps {
  onChanged: () => void;
}

export function WalletSection({ onChanged }: WalletSectionProps) {
  const {
    hasMnemonic, isLoading, error: walletError,
    derivedAddresses, getMnemonic, deleteMnemonic, loadAddresses, clearError,
  } = useWalletStore();

  // All wallet-specific state moves here
  const [showMnemonic, setShowMnemonic] = useState(false);
  const [exportedMnemonic, setExportedMnemonic] = useState<string | null>(null);
  // ... etc

  return (
    <div id="wallet" className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h2 className="text-beige-light text-lg font-semibold">Wallet</h2>
      {/* ... wallet content ... */}
    </div>
  );
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `SignatureAlgorithm = 'secp256k1'` (singleton) | `SignatureAlgorithm = 'secp256k1' \| 'bls12381'` (union) | v1.1 backend (Phase 5-8), v1.2 frontend (this phase) | Frontend can now represent BLS services |
| No P2P visibility in frontend | `P2pStatus` type + `cmd_get_p2p_status` command | This phase | Enables Phase 10 P2P Dashboard |
| No service signer info in frontend | `SignerResponse` type + `cmd_get_service_signer` command | This phase | Enables Phase 11 BLS key display |
| Monolithic Settings.tsx (942 lines) | Section components with sidebar nav | This phase | Maintainable settings, better UX |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust cargo test (backend) + TypeScript strict mode (frontend) |
| Config file | Cargo.toml (Rust), tsconfig.json (TS) |
| Quick run command | `cargo check -p wavs-app` (Rust), `cd app && npx tsc --noEmit` (TS) |
| Full suite command | `cargo test -p wavs-types` (type tests) |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FND-01 | SignatureAlgorithm type includes bls12381 | type-check | `cd app && npx tsc --noEmit` | N/A (type system) |
| FND-02 | New Tauri commands callable | compilation | `cargo check -p wavs-app` | N/A (new code) |
| FND-03 | P2pStatus/SignerResponse types match backend | type-check + unit | `cargo test -p wavs-types serde` | Existing backend tests |
| FND-04 | Settings decomposed into sections | visual | Manual -- page renders identically | N/A |
| SET-01 | Settings reorganized with visual hierarchy | visual | Manual -- sidebar nav works | N/A |
| SET-02 | Visual polish consistent | visual | Manual -- spacing/typography consistent | N/A |

### Sampling Rate
- **Per task commit:** `cd app && npx tsc --noEmit` + `cargo check -p wavs-app`
- **Per wave merge:** Full type check + visual review
- **Phase gate:** Both type checks green, Settings page renders identically with sidebar nav

### Wave 0 Gaps
None -- existing test infrastructure covers all phase requirements. FND-01 through FND-03 are validated by TypeScript strict mode compilation and existing Rust serde tests. FND-04, SET-01, SET-02 are visual verification (no automated test needed for "renders identically").

## Open Questions

1. **IntersectionObserver for active section tracking**
   - What we know: Simple click-to-scroll with manual active state works. IntersectionObserver would auto-highlight the sidebar as user scrolls.
   - What's unclear: Whether the added complexity is worth it for 6 sections.
   - Recommendation: Start with click-based active state. Add IntersectionObserver only if the UX feels lacking -- it's a nice-to-have, not a requirement.

2. **BlsPubkeyResponse shape for cmd_derive_bls_pubkey**
   - What we know: The function returns a 128-byte G1 pubkey. Phase 11 will also need a G2 proof-of-possession for on-chain registration.
   - What's unclear: Whether to include the proof-of-possession in the Phase 9 response type (future-proofing) or add it in Phase 11.
   - Recommendation: Include only `g1_pubkey_hex: string` for now. Phase 11 can extend the response type (or add a separate command). The STATE.md research flag about proof-of-possession encoding is a Phase 11 concern.

3. **BalanceRow component extraction**
   - What we know: `BalanceRow` is already a small component defined inside Settings.tsx (lines 57-74). It's used only by WalletSection.
   - What's unclear: Whether to keep it inside WalletSection.tsx or extract to its own file.
   - Recommendation: Keep it inside WalletSection.tsx as a private component. It's 17 lines and used nowhere else.

## Sources

### Primary (HIGH confidence)
- `packages/types/src/service.rs:565` -- Rust `SignatureAlgorithm` enum definition with serde attributes
- `packages/types/src/http.rs:13-26` -- Rust `SignerResponse` enum definition
- `packages/types/src/http.rs:134-147` -- Rust `P2pStatus` struct definition
- `packages/utils/src/bls_signing.rs:29-32` -- `bls_private_key_from_mnemonic` function signature
- `app/src/types/index.ts:202` -- Current frontend `SignatureAlgorithm` type
- `app/src/pages/Settings.tsx` -- Full 942-line source analyzed for decomposition
- `app/src-tauri/src/commands.rs` -- All existing Tauri command patterns
- `app/src-tauri/src/lib.rs:96-130` -- Command registration in `generate_handler![]`
- `app/src/tauri/commands.ts` -- All existing TypeScript command wrappers
- `app/src/stores/serviceBuilderStore.ts:44-49` -- Current `SubmitDraft` type

### Secondary (MEDIUM confidence)
- `.planning/codebase/CONVENTIONS.md` -- Naming and style conventions
- `.planning/codebase/STRUCTURE.md` -- Where to add new code
- `.planning/research/SUMMARY.md` -- Prior research on BLS command architecture
- `.planning/research/ARCHITECTURE.md` -- Tauri command architecture decisions

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, zero new dependencies
- Architecture: HIGH -- decomposition strategy follows existing `pages/services/` pattern, backend APIs verified in source
- Pitfalls: HIGH -- all pitfalls derived from direct code analysis of the files being modified

**Research date:** 2026-03-23
**Valid until:** 2026-04-23 (stable -- no external dependencies, all code is project-internal)
