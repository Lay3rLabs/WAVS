# Architecture Patterns

**Domain:** Tauri 2 desktop app -- BLS deployment, P2P dashboard, unified events, settings UX
**Researched:** 2026-03-23
**Confidence:** HIGH (all findings verified against codebase)

## Current Architecture Snapshot

### Layers (Backend to Frontend)

```
[WAVS Node]  <-- Dispatcher + Aggregator + HTTP API (Axum)
     |
     v
[Tauri Backend]  <-- commands.rs (invoke), state.rs (managed state), logger.rs (log forwarding)
     |               gui_shared/ (event.rs, settings.rs, error.rs)
     v
[Tauri Events]   <-- settings | log | trigger | submission | service
     |
     v
[React Frontend] <-- Zustand stores -> React pages/components
```

### Existing Tauri Commands (30 commands)

| Category | Commands | Pattern |
|----------|----------|---------|
| Settings | `get_settings`, `set_wavs_home`, `save_poa_registries`, `save_env_vars`, `save_mcp_settings` | Read/write `SettingsState` |
| WAVS lifecycle | `start_wavs`, `restart`, `get_health_status` | `WavsInstanceState` + `WavsConfigState` |
| Services | `get_services`, `add_service`, `remove_service`, `pause_service`, `resume_service`, `save_service_to_node` | `WavsInstanceState.dispatcher()` |
| Wallet | `has_mnemonic`, `store_mnemonic`, `get_mnemonic`, `delete_mnemonic` | OS keyring via `MnemonicCacheState` |
| Components | `get_component_digest`, `publish_component` | wkg registry client |
| IPFS | `upload_to_ipfs` | reqwest to IPFS/Pinata |
| MCP | `start_mcp_server`, `stop_mcp_server`, `get_mcp_status`, `save_mcp_settings`, `register_claude_mcp`, `get_mcp_binary_path`, `get_wavs_url` | `McpServerState` |
| TOML | `read_wavs_toml`, `write_wavs_toml` | File I/O |
| Storage | `list_kv_entries`, `list_fs_entries`, `read_fs_file` | HTTP to WAVS API |
| Reset | `clear_persisted_services` | Combined dispatcher + settings |

### Existing Zustand Stores (4 stores)

| Store | Purpose | Key State |
|-------|---------|-----------|
| `appStore` | Global app state | settings, logList, activityList, services Map |
| `walletStore` | Mnemonic/HD wallet | hasMnemonic, derivedAddresses, pendingMnemonic |
| `serviceBuilderStore` | Service creation wizard | step, workflows, deploy state |
| `poaStore` | POA registry connections | registries Map, operators, ownership |

### Existing Event Flow

```
Dispatcher (Rust) --emit_ext()--> TauriHandle --Emitter::emit()--> Frontend listeners.ts
                                                                        |
                                                                        v
                                                                   appStore actions
```

Events: `settings`, `log`, `trigger`, `submission`, `service`

### Key Type Gaps (Frontend vs Backend)

| Backend Type | Frontend Has | Gap |
|-------------|-------------|-----|
| `SignatureAlgorithm::Bls12381` | `SignatureAlgorithm = 'secp256k1'` only | Missing `'bls12381'` variant |
| `SignerResponse::Bls12381 { hd_index, g1_pubkey_hex }` | Not present | Missing entirely |
| `P2pStatus { enabled, local_peer_id, listen_addresses, connected_peers, peer_ids, subscribed_services }` | Not present | Missing entirely |
| `SubmissionEvent` | Has `service_id, workflow_id, trigger_data` | Missing result/error data |

---

## Recommended Architecture

### Overview: What Changes, What Stays

The architecture follows the existing patterns exactly. No new state management libraries, no new event transport mechanisms, no architectural shifts. The four features map to well-scoped additions:

```
NEW COMMANDS (5):     cmd_get_p2p_status, cmd_get_service_signer,
                      cmd_get_node_info, cmd_derive_bls_pubkey,
                      cmd_get_operator_keys

NEW EVENTS (1):       p2p_status (periodic push)

NEW STORES (1):       p2pStore

MODIFIED STORES (2):  appStore (enhanced ActivityItem), serviceBuilderStore (BLS algorithm)

NEW PAGES (1):        Operators.tsx (P2P/operator dashboard)

MODIFIED PAGES (2):   Settings.tsx (component extraction), Activity.tsx (unified cards)
```

---

## Component Boundaries

### 1. P2P Status Data Flow

**Problem:** Frontend has no P2P visibility. Backend has `/p2p/status` HTTP endpoint and `dispatcher.aggregator.get_p2p_status()`.

**Solution:** Two-pronged approach -- a Tauri command for on-demand fetch, plus a periodic Tauri event for live updates.

#### New Tauri Command: `cmd_get_p2p_status`

```rust
// commands.rs
#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_p2p_status(
    wavs_instance: State<'_, WavsInstanceState>,
) -> AppResult<P2pStatus> {
    let dispatcher = wavs_instance.dispatcher()?;
    Ok(dispatcher.aggregator.get_p2p_status().await)
}
```

This mirrors `cmd_get_health_status` but calls the dispatcher directly rather than going through HTTP. More efficient and avoids the "WAVS HTTP server not bound yet" race condition.

#### New Tauri Event: `p2p_status` (periodic push)

```rust
// gui_shared/event.rs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct P2pStatusEvent {
    pub status: P2pStatus,
}

impl TauriEventExt for P2pStatusEvent {
    const NAME: &'static str = "p2p_status";
}
```

Emitted every 5 seconds from a background task spawned during `cmd_start_wavs`. This avoids frontend polling. The emitter runs alongside the existing log-forwarding tracing layer.

```rust
// In cmd_start_wavs, after dispatcher.start():
{
    let handle = app.clone();
    let dispatcher = dispatcher.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let status = dispatcher.aggregator.get_p2p_status().await;
            let _ = handle.emit_ext(P2pStatusEvent { status });
        }
    });
}
```

#### New Frontend Store: `p2pStore`

```typescript
// stores/p2pStore.ts
interface P2pState {
  status: P2pStatus | null;
  isLoading: boolean;
  setStatus: (status: P2pStatus) => void;
  fetchStatus: () => Promise<void>;
}
```

Populated by both the event listener (live) and the command (on-demand refresh). Separate store because P2P state has a different lifecycle than app/services/wallet state -- it only exists while WAVS is running.

#### New Frontend Types

```typescript
// types/index.ts additions
export interface P2pStatus {
  enabled: boolean;
  local_peer_id: string | null;
  listen_addresses: string[];
  connected_peers: number;
  peer_ids: string[];
  subscribed_services: string[];
}
```

### 2. BLS Key Derivation + Registration

**Problem:** Service builder only supports secp256k1. BLS services need: (a) algorithm selection, (b) BLS public key derivation, (c) on-chain registration of the BLS key via `updateOperatorSigningKey(blsKey, blsSigProof)`.

**Solution:** Three changes -- extend types, add backend command, extend service builder UI + POA operator registration.

#### Type Updates

```typescript
// types/index.ts -- extend existing types
export type SignatureAlgorithm = 'secp256k1' | 'bls12381';
export type SignaturePrefix = 'eip191';

// Add to SubmitDraft in serviceBuilderStore
export interface SubmitDraft {
  signatureAlgorithm: SignatureAlgorithm;  // was 'secp256k1' only
  // ... rest unchanged
}

// New type for signer response (matches Rust SignerResponse)
export type SignerResponse =
  | { secp256k1: { hd_index: number; evm_address: string } }
  | { bls12381: { hd_index: number; g1_pubkey_hex: string } };
```

#### New Tauri Command: `cmd_get_service_signer`

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_service_signer(
    wavs_instance: State<'_, WavsInstanceState>,
    manager: ServiceManager,
) -> AppResult<SignerResponse> {
    let dispatcher = wavs_instance.dispatcher()?;
    let service_id = ServiceId::from(&manager);
    dispatcher
        .get_service_signer(service_id)
        .map_err(|e| AppError::Service(format!("Failed to get signer: {}", e)))
}
```

This wraps the existing `dispatcher.get_service_signer()` which already returns `SignerResponse::Bls12381 { hd_index, g1_pubkey_hex }` or `SignerResponse::Secp256k1 { hd_index, evm_address }`. The backend logic is complete -- we just need to expose it to Tauri.

#### New Tauri Command: `cmd_get_operator_keys`

Returns the signer info for all registered services, so the P2P/operator dashboard can show ECDSA addresses and BLS pubkeys.

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_operator_keys(
    wavs_instance: State<'_, WavsInstanceState>,
) -> AppResult<Vec<(String, SignerResponse)>> {
    let dispatcher = wavs_instance.dispatcher()?;
    // Iterate services, collect signer info
    let services = dispatcher.services.list(Bound::Unbounded, Bound::Unbounded)
        .map_err(|e| AppError::Service(e.to_string()))?;
    let mut keys = Vec::new();
    for service in &services {
        let service_id = ServiceId::from(&service.manager);
        if let Ok(signer) = dispatcher.get_service_signer(service_id) {
            keys.push((service_id.to_string(), signer));
        }
    }
    Ok(keys)
}
```

#### BLS Operator Registration (Frontend)

The BLS `updateOperatorSigningKey(blsKey, blsSigProof)` call differs from the secp256k1 `updateOperatorSigningKey(newSigningKey, signingKeySignature)`:

- **secp256k1**: `newSigningKey` is an address (20 bytes), `signingKeySignature` is ECDSA signature (65 bytes)
- **BLS**: `blsKey` is G1 public key (128 bytes), `blsSigProof` is G2 proof-of-possession signature (256 bytes)

The frontend currently calls `updateOperatorSigningKey` via viem in the POA operator management flows. The BLS variant needs:

1. Call `cmd_get_service_signer` to get the BLS G1 pubkey hex from the backend
2. The proof-of-possession signature must also come from the backend (it signs a domain-separated message proving ownership of the BLS key)

**New Tauri Command: `cmd_derive_bls_pubkey`**

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_derive_bls_pubkey(
    wavs_instance: State<'_, WavsInstanceState>,
    manager: ServiceManager,
) -> AppResult<BlsKeyInfo> {
    // Returns: { g1_pubkey_hex, proof_of_possession_hex }
    // The PoP is sign(BLS_POP_DST, pubkey_bytes) -- needed for updateOperatorSigningKey
}
```

This encapsulates all BLS crypto on the Rust side. The frontend never touches raw BLS keys -- it just passes the hex strings to the contract via viem.

#### Service Builder Changes

The `serviceBuilderStore` needs:

1. `SubmitDraft.signatureAlgorithm` changes from `'secp256k1'` to `SignatureAlgorithm` (union type)
2. `createDefaultSubmit()` keeps `'secp256k1'` as default
3. `SubmitEditor.tsx` gets a radio/select for algorithm choice
4. When `bls12381` is selected, signature prefix is forced to `'none'` (BLS uses hash-to-curve, not EIP-191)
5. `reverseSubmit()` correctly handles `'bls12381'` from existing service hydration

### 3. Unified Activity Events (Trigger + Submission Correlation)

**Problem:** Triggers and submissions appear as separate `ActivityItem` entries with kind `'trigger'` or `'submission'`. No correlation between a trigger and its resulting submission. No error/result display.

**Solution:** Extend `SubmissionEvent` payload, add correlation by `(serviceId, workflowId, triggerData)` composite key, and merge display.

#### Backend: Enrich SubmissionEvent

The current `SubmissionEvent` has `{ service_id, workflow_id, trigger_data }`. It needs:

```rust
// gui_shared/event.rs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmissionEvent {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub trigger_data: TriggerData,
    // NEW FIELDS:
    pub result: SubmissionResult,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionResult {
    Success {
        tx_hash: Option<String>,
        algorithm: String,  // "secp256k1" or "bls12381"
    },
    Error {
        message: String,
    },
}
```

The dispatcher already knows the result at the point it emits `SubmissionConfirmed`. The change is to propagate the tx hash and algorithm from the submission subsystem through `DispatcherCommand::SubmissionConfirmed`.

#### Frontend: Unified Activity Cards

The current `ActivityItem` type gets extended:

```typescript
export interface ActivityItem {
  id: number;
  ts: number;
  kind: ActivityKind;  // still 'trigger' | 'submission'
  serviceId: ServiceId;
  workflowId: WorkflowId;
  triggerData: TriggerData;
  triggerConfig?: TriggerConfig;
  // NEW:
  submissionResult?: SubmissionResult;
  correlatedTriggerId?: number;  // links submission back to its trigger
}

export type SubmissionResult =
  | { success: { tx_hash: string | null; algorithm: string } }
  | { error: { message: string } };
```

**Correlation logic** in `listeners.ts`:

When a `submission` event arrives, search the last N trigger items for a match on `(serviceId, workflowId, triggerData)`. If found, set `correlatedTriggerId`. The `ActivityCard` then renders merged:

- Trigger card shows "Trigger -> Submitted (success)" badge if it has a correlated submission
- Submission card shows the trigger context inline, plus tx hash / error

This is a display-layer correlation -- no backend changes to event timing. The trigger always arrives before its submission, so the search is backwards in the list.

#### ActivityCard Enhancement

`ActivityCard.tsx` gains:
- Result badge (green check for success, red X for error)
- Algorithm indicator pill ("BLS" or "ECDSA")
- Tx hash link (clickable, opens block explorer)
- Error message display (collapsible)
- Merged view: when a trigger has a correlated submission, show both in one card

### 4. Settings Page Restructuring

**Problem:** `Settings.tsx` is 942 lines -- a monolithic component with 20+ `useState` calls, mixing Wallet, WAVS Home, TOML Editor, Env Vars, MCP Server, and Reset sections.

**Solution:** Extract each section into its own component. No new stores needed -- each section component receives props or accesses stores directly.

#### Component Extraction Plan

| Current Section | New Component | Props/Store |
|----------------|---------------|-------------|
| Wallet (accounts, balances, export, reset) | `settings/WalletSection.tsx` | `useWalletStore`, `useAppStore` |
| WAVS Home Directory | `settings/WavsHomeSection.tsx` | `useAppStore` |
| Configuration (wavs.toml) | `settings/TomlEditorSection.tsx` | `useAppStore` |
| Environment Variables | `settings/EnvVarsSection.tsx` | `useAppStore` |
| MCP Server | `settings/McpSection.tsx` | `useAppStore` |
| Reset App State | `settings/ResetSection.tsx` | `usePOAStore`, `useAppStore` |

`Settings.tsx` becomes a thin layout shell:

```tsx
export function Settings() {
  const [changed, setChanged] = useState(false);
  return (
    <div className="flex flex-col gap-6 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {changed && <RestartBanner onRestart={handleRestart} />}
      <WalletSection />
      <WavsHomeSection onChanged={() => setChanged(true)} />
      <TomlEditorSection onChanged={() => setChanged(true)} />
      <EnvVarsSection />
      <McpSection />
      <ResetSection />
    </div>
  );
}
```

Each section manages its own local state (loading, error, form fields). The `onChanged` callback signals that a restart is needed.

---

## Data Flow Diagrams

### P2P Status Flow

```
Aggregator.get_p2p_status()
    |
    +--[every 5s]--> TauriHandle.emit_ext(P2pStatusEvent) --> listeners.ts --> p2pStore.setStatus()
    |
    +--[on demand]--> cmd_get_p2p_status --> invoke('cmd_get_p2p_status') --> p2pStore.setStatus()

p2pStore --> Operators.tsx (P2P dashboard)
             ServiceDetailPage.tsx (per-service peer count)
             HealthIndicator.tsx (peer count badge)
```

### BLS Registration Flow

```
User selects BLS in ServiceBuilder
    |
    v
Service deployed with algorithm: 'bls12381' via IPFS + on-chain
    |
    v
cmd_add_service adds service --> dispatcher derives BLS key for this service
    |
    v
cmd_get_service_signer(manager) --> returns { bls12381: { g1_pubkey_hex } }
    |
    v
cmd_derive_bls_pubkey(manager) --> returns { g1_pubkey_hex, proof_of_possession_hex }
    |
    v
Frontend calls BLS POA registry: updateOperatorSigningKey(blsKey, blsSigProof) via viem
```

### Unified Activity Event Flow

```
Dispatcher: trigger fires
    |
    v
emit_ext(TriggerEvent) --> listeners.ts --> appStore.addActivity({ kind: 'trigger', ... })
    |
    v
Engine executes WASM component
    |
    v
Aggregator collects + aggregates signatures
    |
    v
Submission submits on-chain
    |
    v
DispatcherCommand::SubmissionConfirmed { result: Success { tx_hash, algorithm } }
    |
    v
emit_ext(SubmissionEvent) --> listeners.ts --> correlate with trigger --> appStore.addActivity({ kind: 'submission', correlatedTriggerId, submissionResult })
```

---

## Patterns to Follow

### Pattern 1: Tauri Command for Direct Dispatcher Access

**What:** Wrap `dispatcher` methods as Tauri commands instead of going through the HTTP API.

**When:** For all new commands that need WAVS node state. The existing pattern (`cmd_get_services`) accesses `dispatcher.services.list()` directly, while `cmd_get_health_status` goes through HTTP. Prefer direct access.

**Why:** Avoids the HTTP server startup race. The HTTP API may not be bound yet when the frontend first renders. Direct dispatcher access works as soon as `cmd_start_wavs` completes.

**Example:**

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_p2p_status(
    wavs_instance: State<'_, WavsInstanceState>,
) -> AppResult<P2pStatus> {
    wavs_instance.dispatcher()?.aggregator.get_p2p_status().await
}
```

### Pattern 2: Event Push for Live-Updating Data

**What:** Use Tauri events (not polling commands) for data that changes frequently.

**When:** P2P status, trigger/submission activity, service state changes.

**Why:** Tauri events are zero-cost when no listener is attached. Frontend listeners update Zustand stores atomically. Already proven with `log`, `trigger`, `submission`, `service`, `settings` events.

**Example:** The existing `TriggerEvent` / `SubmissionEvent` pattern, extended for `P2pStatusEvent`.

### Pattern 3: Frontend-Only Correlation

**What:** Correlate related events on the frontend rather than in the backend.

**When:** Matching triggers to submissions. The backend emits them as separate events with overlapping data (same `service_id`, `workflow_id`, `trigger_data`).

**Why:** No backend coupling between trigger and submission lifecycles. The correlation is purely a display concern. The backend might emit triggers that never get submissions (engine errors), or submissions for triggers the frontend missed (catch-up). Frontend correlation is best-effort and non-blocking.

**Example:**

```typescript
// In listeners.ts submission handler:
const triggerMatch = activityList.findLast(
  item => item.kind === 'trigger'
    && item.serviceId === payload.service_id
    && item.workflowId === payload.workflow_id
    && JSON.stringify(item.triggerData) === JSON.stringify(payload.trigger_data)
);
```

### Pattern 4: Section Components for Complex Pages

**What:** Extract page sections into standalone components that manage their own local state.

**When:** A page exceeds ~300 lines or has 10+ `useState` hooks.

**Why:** Prevents re-render cascades (each section only re-renders when its own state changes). Makes the page scannable. Each section can be independently tested.

**Example:** Settings.tsx split into WalletSection, TomlEditorSection, etc.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: HTTP Proxy Commands

**What:** Tauri commands that make HTTP requests to the local WAVS server.

**Why bad:** Race condition on startup (server not bound yet). Extra network hop. Bypasses Tauri's managed state. Currently `cmd_get_health_status`, `cmd_list_kv_entries`, `cmd_list_fs_entries`, `cmd_read_fs_file`, `cmd_save_service_to_node` use this pattern.

**Instead:** Access the dispatcher directly when possible. For dev-only endpoints (KV, FS, logs), the HTTP proxy pattern is acceptable since those APIs are specifically designed for external tooling and may not have direct dispatcher methods.

### Anti-Pattern 2: Polling from Frontend

**What:** `setInterval` in React components to periodically call Tauri commands.

**Why bad:** Wasted IPC when nothing changed. Difficult to coordinate intervals across components. The MCP status poll (`useEffect` with `setInterval(poll, 3000)` in Settings.tsx) is an existing example.

**Instead:** Backend pushes events. Frontend subscribes. For MCP status specifically, consider adding a `McpStatusEvent` later.

### Anti-Pattern 3: Storing Derived State in Zustand

**What:** Storing computed values that can be derived from other store state.

**Why bad:** Stale computed values, sync bugs, unnecessary updates.

**Instead:** Use Zustand selectors or `useMemo` in components. Example: service labels are already computed via `getServiceLabel(serviceId)` rather than stored per-activity-item.

### Anti-Pattern 4: BLS Crypto in Frontend

**What:** Using a JavaScript BLS library (e.g., `@noble/bls12-381`) to derive keys or sign proofs.

**Why bad:** The backend already has correct, tested BLS key derivation with specific DST, HKDF parameters, and G1/G2 coordinate handling. Reimplementing in JS risks subtle crypto mismatches (wrong DST, wrong endianness, incompatible point formats).

**Instead:** All BLS operations go through Tauri commands that call the Rust backend. The frontend only handles hex-encoded keys as opaque strings.

---

## New/Modified Components Summary

### Backend (Rust)

| File | Change | Type |
|------|--------|------|
| `gui_shared/event.rs` | Add `P2pStatusEvent`, enrich `SubmissionEvent` with `SubmissionResult` | Modified |
| `app/src-tauri/src/commands.rs` | Add `cmd_get_p2p_status`, `cmd_get_service_signer`, `cmd_get_operator_keys`, `cmd_derive_bls_pubkey` | Modified |
| `app/src-tauri/src/lib.rs` | Register new commands in `generate_handler![]` | Modified |
| `app/src-tauri/src/commands.rs` | P2P status background emitter in `cmd_start_wavs` | Modified |
| `packages/wavs/src/dispatcher.rs` | Enrich `SubmissionConfirmed` with result/tx_hash/algorithm | Modified |

### Frontend (TypeScript/React)

| File | Change | Type |
|------|--------|------|
| `types/index.ts` | Add `P2pStatus`, `SignerResponse`, `SubmissionResult`; extend `SignatureAlgorithm`, `ActivityItem` | Modified |
| `stores/p2pStore.ts` | New store for P2P network state | New |
| `stores/appStore.ts` | No structural changes (enhanced `ActivityItem` type handled transparently) | Unchanged |
| `stores/serviceBuilderStore.ts` | `SubmitDraft.signatureAlgorithm` widened to `SignatureAlgorithm` union | Modified |
| `tauri/commands.ts` | Add `getP2pStatus`, `getServiceSigner`, `getOperatorKeys`, `deriveBlsPubkey` | Modified |
| `tauri/listeners.ts` | Add `p2p_status` listener, correlation logic for submissions | Modified |
| `pages/Operators.tsx` | New page: P2P dashboard + operator keys | New |
| `pages/Settings.tsx` | Thin layout shell (extract sections) | Modified (major refactor) |
| `components/settings/WalletSection.tsx` | Extracted from Settings | New |
| `components/settings/WavsHomeSection.tsx` | Extracted from Settings | New |
| `components/settings/TomlEditorSection.tsx` | Extracted from Settings | New |
| `components/settings/EnvVarsSection.tsx` | Extracted from Settings | New |
| `components/settings/McpSection.tsx` | Extracted from Settings | New |
| `components/settings/ResetSection.tsx` | Extracted from Settings | New |
| `components/activity/ActivityCard.tsx` | Unified trigger+submission display, result/error badges | Modified |
| `components/service/SubmitEditor.tsx` | Algorithm selector (secp256k1 / bls12381) | Modified |
| `components/layout/Header.tsx` | Add "Operators" nav item | Modified |
| `App.tsx` | Add `/operators` route | Modified |

### Contract Interactions (Frontend via Viem)

| Contract | Change | Type |
|----------|--------|------|
| BLS POAStakeRegistry ABI | Need `BlsPOAStakeRegistryABI` in `contracts/` | New file |
| Existing POAStakeRegistry ABI | Unchanged (secp256k1 path) | Unchanged |

The BLS `updateOperatorSigningKey(blsKey, blsSigProof)` has different parameter types than the secp256k1 variant (bytes vs address). The frontend needs the BLS ABI to call this correctly.

---

## Suggested Build Order

The features have dependencies that constrain ordering:

### Phase 1: Foundation Types + Settings Refactor

**Build:** Type extensions, settings page decomposition.

**Why first:** Types are imported everywhere. Settings refactor is a low-risk prerequisite that reduces the surface area for subsequent changes. No backend changes needed.

**Deliverables:**
1. Extended `types/index.ts` with `P2pStatus`, `SignerResponse`, `SubmissionResult`, widened `SignatureAlgorithm`
2. Settings.tsx decomposed into 6 section components
3. `serviceBuilderStore.ts` algorithm field widened

### Phase 2: P2P Status + Operators Page

**Build:** Backend `cmd_get_p2p_status`, `P2pStatusEvent`, `p2pStore`, `Operators.tsx` page, event listener.

**Why second:** P2P status is a standalone read-only feature with no dependencies on BLS or activity changes. Provides immediate user-visible value.

**Deliverables:**
1. `cmd_get_p2p_status` and `cmd_get_operator_keys` Tauri commands
2. `P2pStatusEvent` background emitter
3. `p2pStore.ts`
4. `Operators.tsx` page with Ed25519 identity, connected peers, subscribed services, operator key display
5. Nav item added, route added

### Phase 3: BLS Service Builder + Registration

**Build:** Backend `cmd_get_service_signer`, `cmd_derive_bls_pubkey`, `SubmitEditor` algorithm selector, BLS POA registry ABI, operator registration flow.

**Why third:** Depends on Phase 1 types. The BLS registration flow touches the service builder, POA contract interactions, and the new Tauri commands.

**Deliverables:**
1. `cmd_get_service_signer` and `cmd_derive_bls_pubkey` Tauri commands
2. Algorithm selector in `SubmitEditor.tsx`
3. BLS POAStakeRegistry ABI
4. `updateOperatorSigningKey` call with BLS key + proof

### Phase 4: Unified Activity Events

**Build:** Enriched `SubmissionEvent`, correlation logic, enhanced `ActivityCard`.

**Why last:** Depends on Phase 1 types. Touches the dispatcher's submission pipeline (moderate risk). Can be built in parallel with Phase 3 if resources allow, but sequential is safer since both touch `types/index.ts`.

**Deliverables:**
1. Enriched `SubmissionEvent` with `SubmissionResult`
2. Dispatcher pipeline change to propagate tx_hash + algorithm
3. Frontend correlation logic in `listeners.ts`
4. Enhanced `ActivityCard` with result badges, algorithm pills, merged view

---

## Scalability Considerations

| Concern | Current (10 services) | At 100 services | At 1000 services |
|---------|----------------------|-----------------|------------------|
| P2P status event size | ~500 bytes/5s | ~2KB/5s (more peer_ids, subscribed_services) | 10-20KB/5s; consider throttling or only sending diffs |
| Activity correlation | O(n) scan, n < 2000 | Fine, ring buffer caps at 2000 | Fine -- ring buffer prevents unbounded growth |
| Operator keys | One command returns all | Paginate if >50 services | Not realistic for desktop app |
| Service builder BLS | One derivation per deploy | Fine | Fine |

---

## Sources

All findings verified against codebase:
- `app/src-tauri/src/commands.rs` -- existing Tauri command patterns
- `app/src-tauri/src/state.rs` -- managed state patterns
- `packages/gui/shared/src/event.rs` -- event emission pattern
- `packages/wavs/src/subsystems/aggregator.rs` -- `get_p2p_status()` method
- `packages/wavs/src/http/handlers/info.rs` -- P2pStatus in HTTP response
- `packages/types/src/http.rs` -- `P2pStatus` struct, `SignerResponse` enum
- `packages/types/src/service.rs` -- `SignatureAlgorithm` enum
- `packages/types/src/solidity_types/bls.rs` -- BLS contract ABI bindings
- `packages/types/src/contracts/solidity/abi/bls/IPOAStakeRegistry.json` -- BLS `updateOperatorSigningKey(blsKey, blsSigProof)`
- `app/src/stores/` -- all 4 existing stores
- `app/src/tauri/listeners.ts` -- event listener pattern
- `app/src/types/index.ts` -- all frontend types
- `app/src/pages/Settings.tsx` -- 942-line monolith to decompose
- `app/src/components/activity/ActivityCard.tsx` -- current activity rendering
