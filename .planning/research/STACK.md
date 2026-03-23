# Technology Stack

**Project:** WAVS v1.2 Tauri App -- New Feature Stack Additions
**Researched:** 2026-03-23
**Overall confidence:** HIGH

This document covers ONLY the stack additions/changes needed for the v1.2 milestone features: BLS service deployment UI, P2P operator dashboard, unified activity events, and settings UX overhaul. The existing stack (Tauri 2, React 19, Vite 7, Zustand 5, Viem 2, Tailwind 3, CodeMirror 6, @tanstack/react-virtual, @scure/bip39) is validated and NOT re-researched.

## Executive Summary

The existing stack is sufficient for nearly everything. The new features are primarily UI pages that consume data already available from the WAVS HTTP API (P2P status, signer info) and existing Tauri events (triggers, submissions). **No new frontend npm dependencies are needed.** The work is:

1. **New Tauri commands** (Rust backend) to proxy existing HTTP API endpoints (`/p2p/status`, `/services/signer`) into the Tauri IPC layer
2. **New TypeScript types** for P2P status, BLS signer responses, and updated `SignatureAlgorithm` enum
3. **New React pages and components** using existing patterns (Zustand stores, hand-rolled Tailwind components, `@tanstack/react-virtual` for lists)
4. **Updated service builder store** to support `bls12381` algorithm selection
5. **New BLS POAStakeRegistry ABI** in the frontend for BLS operator key registration

No new runtime dependencies. No architecture changes. The app already has all the plumbing.

## Recommended Stack -- Additions Only

### New Tauri Commands (Rust Backend)

| Command | Proxies | Purpose | Why Not Direct HTTP |
|---------|---------|---------|---------------------|
| `cmd_get_p2p_status` | `GET /p2p/status` | P2P dashboard data | Consistent with existing pattern (all data flows through Tauri IPC, not direct HTTP from renderer). The app's renderer process does not know the WAVS HTTP port; only the Rust backend does via `WavsConfigState`. |
| `cmd_get_service_signer` | `POST /services/signer` | BLS/ECDSA key display | Same reason. Returns `SignerResponse` (either `Secp256k1 { hd_index, evm_address }` or `Bls12381 { hd_index, g1_pubkey_hex }`). |

**Implementation pattern:** Follow `cmd_get_health_status` exactly -- read config from `WavsConfigState`, make `reqwest` call to local HTTP API, deserialize response, return to frontend.

**Alternative considered: direct Dispatcher access.** The P2P status could be fetched via `state.dispatcher.aggregator.get_p2p_status()` (like the HTTP handler does). This avoids the HTTP round-trip but couples the Tauri command to internal APIs. Both approaches work. The HTTP proxy approach is simpler and consistent with existing patterns.

### New TypeScript Types

These types mirror Rust structs already defined in `packages/types/src/http.rs` and `packages/types/src/service.rs`:

| Type | Source | Status |
|------|--------|--------|
| `P2pStatus` | `packages/types/src/http.rs` | New -- not yet in frontend |
| `SignerResponse` | `packages/types/src/http.rs` | New -- not yet in frontend |
| `SignatureAlgorithm: 'secp256k1' \| 'bls12381'` | `packages/types/src/service.rs` | **Update** -- currently hardcoded to `'secp256k1'` only in `app/src/types/index.ts` |

### BLS POAStakeRegistry ABI (Frontend)

The BLS `IPOAStakeRegistry` contract interface is different from the existing secp256k1 one:

| Difference | secp256k1 (existing) | BLS (new) |
|-----------|---------------------|-----------|
| `getLatestOperatorSigningKey` returns | `address` | `bytes` (128-byte G1 pubkey) |
| `updateOperatorSigningKey` args | `(address newSigningKey, bytes signingKeySignature)` | `(bytes blsKey, bytes blsSigProof)` |
| `SigningKeyUpdate` event | `newSigningKey: address` | `newKeyHash: bytes32` |
| New errors | -- | `InvalidBLSKeyLength`, `InvalidBLSKeyOwnershipProof`, `InvalidBLSSignature`, `InvalidBLSSignatureLength` |

**Action:** Create `app/src/contracts/POABlsStakeRegistry.ts` with the ABI from `packages/types/src/contracts/solidity/abi/bls/IPOAStakeRegistry.json`. The existing `POAStakeRegistry.ts` remains for secp256k1 registries.

### Frontend Patterns to Follow

| Pattern | Existing Example | Apply To |
|---------|-----------------|----------|
| Zustand store | `appStore.ts` | New `p2pStore.ts` for P2P status polling state |
| Tauri command wrapper | `tauri/commands.ts` | New `getP2pStatus()`, `getServiceSigner()` commands |
| Tauri event listener | `tauri/listeners.ts` | No new events needed -- P2P status uses polling, not push |
| Virtualized list | `ActivityFeed.tsx` | Peer list (if many peers, unlikely in practice) |
| Hand-rolled components | `components/atoms/` | New `KeyDisplay`, `PeerCard`, `QuorumProgress` components |
| Page layout | `pages/Health.tsx` | New `P2P.tsx` page, updated `Settings.tsx` |

### What NOT to Add

| Category | Temptation | Why Not |
|----------|-----------|---------|
| UI component library | Radix, Headless UI, shadcn | App uses hand-rolled Tailwind components (Button, Modal, Dropdown, Tabs, Toast, etc.). Adding a component library would create inconsistency. The existing atoms are sufficient. |
| Charting library | recharts, victory, d3 | Quorum progress is a simple bar/percentage. A full charting library is overkill. Use Tailwind width percentages. |
| WebSocket client | socket.io-client, ws | P2P status is infrequent (poll every 5-10s via Tauri command). The WAVS HTTP API does not expose a WebSocket endpoint for P2P status. Polling with `setInterval` + Zustand is the correct pattern. |
| State machine library | XState, Robot | BLS key registration is a 2-step flow (register operator + register BLS key). A simple state variable in Zustand is sufficient, like the existing `DeployState` pattern in `serviceBuilderStore.ts`. |
| Copy-to-clipboard library | clipboard-copy, react-copy-to-clipboard | Use `navigator.clipboard.writeText()` directly. All Tauri webview targets support it. |
| BLS crypto in frontend | noble-bls12-381 | BLS key derivation happens in the Rust backend only. The frontend displays hex strings, never computes BLS operations. |
| react-query / tanstack-query | Data fetching + caching | The app uses Zustand + manual `invoke()` calls. Adding react-query would create two competing data-fetching paradigms. Keep using Zustand. |
| Form library | react-hook-form, formik | Settings page uses controlled inputs with Zustand. Continue this pattern. |

## Detailed Component Analysis

### 1. P2P Dashboard Page

**Data source:** `GET /p2p/status` via new `cmd_get_p2p_status` Tauri command.

**Response shape** (from `packages/types/src/http.rs`):
```typescript
interface P2pStatus {
  enabled: boolean;
  local_peer_id: string | null;  // Ed25519 pubkey hex
  listen_addresses: string[];     // e.g. ["0.0.0.0:9000"]
  connected_peers: number;
  peer_ids: string[];            // Ed25519 pubkeys of connected peers
  subscribed_services: string[]; // hex service ID hashes
}
```

**Polling pattern:** `useEffect` with `setInterval(5000)`. Store in a `p2pStore.ts` (Zustand). Display:
- Node identity card (Ed25519 peer ID, listen addresses)
- Connected peers list (peer IDs, truncated with copy)
- Subscribed services (correlated with service names from `appStore.services`)

**No new dependencies needed.** Truncation + copy uses existing `AddressDisplay` component pattern. Periodic refetch uses `setInterval` like Health page already does.

### 2. BLS Key Display + Registration

**Key display data:** `POST /services/signer` via new `cmd_get_service_signer` Tauri command.

**Response shape** (from `packages/types/src/http.rs`):
```typescript
type SignerResponse =
  | { secp256k1: { hd_index: number; evm_address: string } }
  | { bls12381: { hd_index: number; g1_pubkey_hex: string } };
```

**Key registration flow for BLS:**
1. Owner calls `registerOperator(operatorAddress, weight)` -- same as secp256k1
2. Operator calls `updateOperatorSigningKey(blsKey, blsSigProof)` where:
   - `blsKey` = 128-byte G1 pubkey (from SignerResponse)
   - `blsSigProof` = 256-byte G2 signature proving key ownership

**Critical: BLS sig proof computation happens on the backend.** The frontend cannot compute BLS signatures. A new Tauri command `cmd_get_bls_key_proof` is needed that calls the Rust backend to sign `keccak256(abi.encode(operatorAddress))` with the BLS private key and return the G2 signature bytes. This is the ONLY new crypto operation needed.

**Contract interaction:** Uses existing Viem `writeContract()` pattern from `app/src/utils/evm.ts`. The BLS `updateOperatorSigningKey` takes `(bytes, bytes)` instead of `(address, bytes)` -- the ABI handles this.

### 3. Unified Activity Events

**Current state:** Activity events already unify triggers and submissions. Each `ActivityItem` has a `kind: 'trigger' | 'submission'` discriminator. The `ActivityCard` and `ActivityFeed` components already handle both.

**What needs to change:**
- Add submission result data (success/error) to `SubmissionEvent` -- currently only has `service_id`, `workflow_id`, `trigger_data`
- Correlate trigger-to-submission by matching `serviceId + workflowId + triggerData` (or EventId if available)
- Display error information when submission fails
- Show BLS/secp256k1 algorithm badge on submission cards

**Backend change needed:** The Rust `SubmissionEvent` (emitted via `app.emit_ext()`) needs to include:
- `success: boolean`
- `error: string | null`
- `signature_algorithm: string` (for badge display)
- Optional: `event_id: string` for correlation

**No new frontend dependencies.** Just type updates and conditional rendering in `ActivityCard`.

### 4. Settings Page Reorganization

**Current state:** `Settings.tsx` is 33K -- a single large file with all settings sections inlined.

**Recommended approach:** Extract sections into sub-components:
- `settings/GeneralSection.tsx` (wavs home, restart)
- `settings/WalletSection.tsx` (mnemonic management, derived addresses)
- `settings/McpSection.tsx` (MCP server config)
- `settings/EnvVarsSection.tsx` (environment variables)
- `settings/RegistrySection.tsx` (POA registries)
- `settings/AdvancedSection.tsx` (TOML editor, reset)

Use existing `Tabs` component from `components/atoms/Tabs.tsx` for navigation between sections.

**No new dependencies.** This is pure refactoring + UI reorganization.

### 5. Service Builder BLS Support

**Current state:** `serviceBuilderStore.ts` has `signatureAlgorithm: 'secp256k1'` hardcoded in `SubmitDraft`.

**Changes needed:**
- Update `SignatureAlgorithm` type to `'secp256k1' | 'bls12381'`
- Update `SubmitDraft.signatureAlgorithm` to accept both
- When `bls12381` is selected, set `signaturePrefix` to `'none'` (BLS uses hash-to-curve, not EIP-191)
- UI: Algorithm selector radio/toggle in the submit section of the service builder
- UI: When BLS selected, show the BLS G1 pubkey (from `cmd_get_service_signer`) and registration status

## New Tauri Commands Summary

| Command | Input | Output | Backend Implementation |
|---------|-------|--------|----------------------|
| `cmd_get_p2p_status` | (none) | `P2pStatus` | HTTP GET to `/p2p/status` |
| `cmd_get_service_signer` | `{ service_manager: ServiceManager }` | `SignerResponse` | HTTP POST to `/services/signer` |
| `cmd_get_bls_key_proof` | `{ service_manager: ServiceManager, operator_address: string }` | `{ g1_pubkey: string, g2_proof: string }` | Derive BLS key from mnemonic, sign keccak256(abi.encode(operator)) |

## New Zustand Store

```typescript
// p2pStore.ts -- minimal, follows appStore pattern
interface P2pState {
  status: P2pStatus | null;
  isPolling: boolean;
  error: string | null;

  fetchStatus: () => Promise<void>;
  startPolling: (intervalMs?: number) => void;
  stopPolling: () => void;
}
```

## New React Router Routes

```typescript
// In App.tsx, add:
<Route path="/p2p" element={<P2PPage />} />
```

## File Structure for New Code

```
app/src/
  types/index.ts          -- UPDATE: add P2pStatus, SignerResponse, update SignatureAlgorithm
  tauri/commands.ts        -- UPDATE: add getP2pStatus(), getServiceSigner(), getBlsKeyProof()
  stores/p2pStore.ts       -- NEW: P2P status polling store
  stores/serviceBuilderStore.ts -- UPDATE: BLS algorithm support
  pages/P2P.tsx            -- NEW: P2P dashboard page
  pages/Settings.tsx       -- REFACTOR: extract into settings/ sub-components
  contracts/POABlsStakeRegistry.ts -- NEW: BLS registry ABI + helpers
  utils/evm.ts             -- UPDATE: add BLS operator registration functions
  components/p2p/          -- NEW: PeerCard, NodeIdentity, QuorumProgress
  components/activity/ActivityCard.tsx -- UPDATE: submission result display, algorithm badge
```

## Rust Backend (src-tauri/)

```
app/src-tauri/src/
  commands.rs              -- UPDATE: add cmd_get_p2p_status, cmd_get_service_signer, cmd_get_bls_key_proof
  lib.rs                   -- UPDATE: register new commands in handler macro
```

**Cargo.toml change:** None needed. The `wavs` workspace dependency already includes all BLS and P2P functionality. The Tauri commands proxy to the HTTP API or access the dispatcher directly.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| P2P data fetching | Tauri command + setInterval polling | Tauri event push from backend | P2P status changes infrequently (peers join/leave). Polling every 5s is simpler than adding a new Rust event emitter to the P2P subsystem. Avoids coupling Tauri to aggregator internals. |
| BLS key proof | New Tauri command `cmd_get_bls_key_proof` | Frontend BLS library (noble-bls12-381) | BLS private key only exists in Rust backend (derived from mnemonic via blst). Sending private key material to the renderer is a security risk. Keep all crypto in Rust. |
| Settings page | Tab-based sections via existing Tabs atom | Accordion layout | Tabs are clearer for discrete sections. App already has a Tabs component. |
| Activity correlation | Client-side match by serviceId+workflowId+triggerData hash | Backend-emitted correlation ID | Backend EventId computation uses RIPEMD160 which is not available in the browser WebCrypto API. Client-side matching by composite key is simpler and sufficient for display purposes. |
| BLS ABI source | Separate `POABlsStakeRegistry.ts` file | Extend existing `POAStakeRegistry.ts` with conditional types | The ABIs are different contracts with different function signatures. Separate files are cleaner. The secp256k1 registry returns `address` for signing keys; the BLS registry returns `bytes`. |

## Installation

**No new npm packages to install.** All features use existing dependencies.

**No new Cargo dependencies.** The Tauri backend already has access to everything needed via workspace dependencies (`wavs`, `wavs-types`, `wavs-gui-shared`).

## Verification Notes

- `P2pStatus` struct verified in `packages/types/src/http.rs` lines 132-147 (HIGH confidence)
- `SignerResponse` enum verified in `packages/types/src/http.rs` lines 13-26 (HIGH confidence)
- `SignatureAlgorithm::Bls12381` variant verified in `packages/types/src/service.rs` lines 565-568 (HIGH confidence)
- BLS `IPOAStakeRegistry` ABI verified in `packages/types/src/contracts/solidity/abi/bls/IPOAStakeRegistry.json` (HIGH confidence)
- BLS `updateOperatorSigningKey(bytes blsKey, bytes blsSigProof)` signature verified in same ABI (HIGH confidence)
- Existing Tauri command pattern verified in `app/src-tauri/src/commands.rs` (HIGH confidence)
- Frontend type mismatch: `SignatureAlgorithm` is `'secp256k1'` only in `app/src/types/index.ts` but Rust has both variants (confirmed, needs update)

## Sources

- `packages/types/src/http.rs` -- P2pStatus, SignerResponse structs
- `packages/types/src/service.rs` -- SignatureAlgorithm, SignatureKind
- `packages/types/src/contracts/solidity/abi/bls/IPOAStakeRegistry.json` -- BLS registry contract ABI
- `app/src-tauri/src/commands.rs` -- existing Tauri command patterns
- `app/src/types/index.ts` -- current frontend type definitions
- `app/src/stores/` -- existing Zustand store patterns
- `app/src/utils/evm.ts` -- existing operator registration helpers
- `packages/wavs/src/http/handlers/p2p.rs` -- P2P status endpoint
- `packages/wavs/src/http/handlers/service/key.rs` -- signer endpoint
