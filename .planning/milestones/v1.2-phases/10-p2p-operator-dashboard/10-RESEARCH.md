# Phase 10: P2P Operator Dashboard - Research

**Researched:** 2026-03-24
**Domain:** Tauri 2 + React 19 frontend -- new P2P dashboard page with polling data, per-service operator keys, and on-chain registration checks
**Confidence:** HIGH

## Summary

Phase 10 builds a new P2P Operator Dashboard page in the WAVS Tauri desktop app. All backend infrastructure exists from Phase 9: three Tauri commands (`cmd_get_p2p_status`, `cmd_get_service_signer`, `cmd_derive_bls_pubkey`) are already registered and functional, the TypeScript types (`P2pStatus`, `SignerResponse`, `BlsPubkeyResponse`) are defined, and the frontend wrappers in `tauri/commands.ts` are wired. The work is purely frontend: a new route, a nav entry, and the page components that poll and display data.

One gap exists: the `P2pStatus` struct contains `enabled`, `local_peer_id`, `listen_addresses`, `connected_peers`, `peer_ids`, and `subscribed_services` -- but does NOT include the P2P discovery mode (Disabled/Local/Remote). Requirement P2P-01 asks for "discovery mode" display. The mode is available from `WavsConfigState.inner.p2p` (the `P2pConfig` enum variant name). A small Rust-side change is needed: either extend the existing `cmd_get_p2p_status` to also read from `WavsConfigState`, or derive the label from the `enabled` field plus listen addresses (less precise). The cleanest approach is to add a `discovery_mode: String` field to the `P2pStatus` Rust struct and populate it from the config during status construction.

For P2P-05 (on-chain registration status), the existing `POAStakeRegistry` ABI and `evm.ts` utilities already support `operatorRegistered()` and `getLatestOperatorSigningKey()` contract reads. The app has the `poaStore` with per-service registry connections. The P2P page can use the same `getPublicClient` + contract read pattern to check if a signing key is registered for each service.

**Primary recommendation:** Build the P2P page following the Health page pattern (local state + `useEffect` polling interval), add the route and nav item, and call the three existing Tauri commands. Extend `P2pStatus` with a `discovery_mode` field on the Rust side. For P2P-05 registration checks, read the on-chain signing key via the existing `POAStakeRegistry` ABI.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| P2P-01 | P2P page accessible from header nav showing Ed25519 identity, discovery mode, and listen addresses | Nav item pattern in `Header.tsx`, `P2pStatus` struct has `local_peer_id` and `listen_addresses`. Discovery mode requires a small Rust-side addition to `P2pStatus`. |
| P2P-02 | Connected peers list with peer IDs and connection status | `P2pStatus.peer_ids` and `connected_peers` fields exist; polling pattern from Health page |
| P2P-03 | Subscribed services list showing which services are active on P2P topics | `P2pStatus.subscribed_services` contains hex-encoded service ID hashes; match against `appStore.services` Map keyed by those same hashes to get human-readable names |
| P2P-04 | Per-service operator key display (BLS G1 pubkey or ECDSA address) with copy button | `cmd_get_service_signer` returns `SignerResponse` (secp256k1 or bls12381 variant); `AddressDisplay` component has copy-to-clipboard |
| P2P-05 | Operator key registration status indicator (registered/unregistered on-chain) | `POAStakeRegistryABI.getLatestOperatorSigningKey` + `operatorRegistered` contract reads via Viem; existing patterns in `utils/evm.ts` and `poaStore` |
| P2P-06 | *(Stretch)* Live quorum accumulation progress per service | No backend endpoint exists for this. The aggregator has quorum queues internally but no status API. Defer or show placeholder "No quorum data available". |
</phase_requirements>

## Standard Stack

### Core (already installed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19.1.0 | UI framework | Already in use; all components use React 19 patterns |
| react-router-dom | 7.1.0 | Routing | Already in use; `BrowserRouter` + `Routes` in App.tsx |
| zustand | 5.0.0 | State management | Already in use for `appStore`, `walletStore`, `poaStore` |
| viem | 2.23.5 | Blockchain interaction | Already in use for on-chain reads in `utils/evm.ts` |
| @tauri-apps/api | 2.10.1 | Tauri IPC | Already in use for all `invoke()` calls |
| clsx | 2.1.0 | Conditional classnames | Already in use throughout components |
| Tailwind CSS | 3.4.0 | Styling | Already in use; custom color palette defined |

### No New Dependencies

This phase requires zero new npm packages. Everything is available in the existing stack.

## Architecture Patterns

### Recommended Project Structure

```
app/src/
  pages/
    p2p/
      P2pPage.tsx           # Main P2P dashboard page (self-contained)
      index.ts              # Barrel export
  pages/index.ts            # Add P2pPage export
  components/layout/
    Header.tsx              # Add "P2P" nav item
  App.tsx                   # Add /p2p route
  types/index.ts            # Add discovery_mode to P2pStatus
```

### Pattern 1: Polling Dashboard Page (follow Health.tsx)

**What:** A page that fetches data on mount and on a regular interval, storing results in local `useState`.
**When to use:** Dashboard pages that display live-ish status without needing cross-component state sharing.
**Source:** `app/src/pages/Health.tsx` (verified in codebase)

```typescript
const REFRESH_INTERVAL_MS = 15000; // 15 seconds for P2P, shorter than Health's 30s

export function P2pPage() {
  const [p2pStatus, setP2pStatus] = useState<P2pStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  const fetchP2pStatus = useCallback(async () => {
    setIsRefreshing(true);
    try {
      const status = await getP2pStatus();
      setP2pStatus(status);
      setError(null);
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setIsRefreshing(false);
    }
  }, []);

  useEffect(() => {
    fetchP2pStatus();
    const interval = setInterval(fetchP2pStatus, REFRESH_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [fetchP2pStatus]);

  // ... render
}
```

### Pattern 2: Nav Item Addition (follow Header.tsx)

**What:** Adding a new navigation entry to the header.
**Source:** `app/src/components/layout/Header.tsx` line 64-70

```typescript
const navItems: { path: string; label: string; icon: ReactNode }[] = [
  { path: '/services',    label: 'Services',    icon: <ServicesIcon /> },
  { path: '/components',  label: 'Components',  icon: <ComponentsIcon /> },
  { path: '/activity',    label: 'Activity',    icon: <ActivityIcon /> },
  { path: '/p2p',         label: 'P2P',         icon: <P2pIcon /> },  // NEW
  { path: '/logs',        label: 'Logs',        icon: <LogsIcon /> },
  { path: '/settings',    label: 'Settings',    icon: <SettingsIcon /> },
];
```

### Pattern 3: On-Chain Registration Check (follow evm.ts)

**What:** Read-only contract calls to check operator registration status.
**Source:** `app/src/utils/evm.ts` `fetchOperators()` function

For P2P-05, checking if an operator's signing key is registered on-chain for each service:

```typescript
// For each EVM service with a known registry address:
const signingKey = await publicClient.readContract({
  address: registryAddress,
  abi: POAStakeRegistryABI,
  functionName: 'getLatestOperatorSigningKey',
  args: [operatorAddress],
});
const isRegistered = signingKey !== '0x0000000000000000000000000000000000000000';
```

### Pattern 4: Service Hash ID Mapping

**What:** The `subscribed_services` in `P2pStatus` contains SHA-256 hex hashes of `ServiceId`. These are the same keys used in `appStore.services` Map.
**Source:** `app/src/types/index.ts` `computeServiceHash()` and `buildServiceMap()`

```typescript
const services = useAppStore((state) => state.services);
// subscribed_services from P2pStatus are hex-encoded ServiceId hashes
// These match the keys in the services Map
const serviceName = services.get(serviceHash)?.name ?? 'Unknown';
```

### Pattern 5: Copy-to-Clipboard Button (follow AddressDisplay)

**What:** The existing `AddressDisplay` component handles click-to-copy for Ethereum addresses.
**Source:** `app/src/components/atoms/AddressDisplay.tsx`

For BLS keys (128-byte hex strings), `AddressDisplay` with `full={false}` will truncate using `shortenAddress()` which shows `first6...last4`. This works but may need adjustment for BLS keys that are longer than ETH addresses. The existing component can be reused as-is since it takes any string.

### Anti-Patterns to Avoid

- **Don't create a Zustand store for P2P state:** The Health page proves that local `useState` + polling is the right pattern for status dashboards. P2P status doesn't need cross-component sharing.
- **Don't poll faster than 10 seconds:** The P2P status command goes through the Tauri IPC bridge to the aggregator's P2P handle. 15-second intervals match the Health page pattern.
- **Don't fetch on-chain registration status on every poll:** Contract reads should be triggered manually (refresh button) or on initial mount only, not on every 15-second P2P status poll. They are RPC calls that may be slow or rate-limited.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Copy-to-clipboard | Custom clipboard logic | `AddressDisplay` component | Already handles copy + visual feedback |
| Status polling | Custom hook | `useEffect` + `setInterval` pattern from Health.tsx | Battle-tested in same codebase |
| Contract reads | Custom ABI encoding | `viem` `readContract` + existing `POAStakeRegistryABI` | Already wired in `evm.ts` |
| Service name resolution | Manual hash lookup | `appStore.services.get(hashId)?.name` | `buildServiceMap` already computes hash-to-service mapping |
| Conditional styling | CSS classes | `clsx` utility | Already used everywhere |

## Common Pitfalls

### Pitfall 1: Discovery Mode Not in P2pStatus

**What goes wrong:** The TypeScript `P2pStatus` type has no `discovery_mode` field, but P2P-01 requires showing it.
**Why it happens:** The Rust `P2pStatus` struct was designed for the HTTP API where `InfoResponse` separately includes `p2p_config`. The Tauri command only returns `P2pStatus`.
**How to avoid:** Extend the `P2pStatus` Rust struct to include a `discovery_mode: String` field, populated from the `P2pConfig` variant name ("disabled", "local", or "remote"). Update the TypeScript type to match. Alternatively, add a new Tauri command like `cmd_get_p2p_config_mode` that reads from `WavsConfigState`.
**Warning signs:** If you try to display discovery mode from P2pStatus and the field doesn't exist.

### Pitfall 2: Subscribed Services Are Hex Hashes, Not Names

**What goes wrong:** `P2pStatus.subscribed_services` contains raw SHA-256 hex strings like `"a1b2c3..."`, not human-readable service names.
**Why it happens:** The WAVS backend hashes `ServiceManager` (chain + address) to produce `ServiceId`. These hashes are the P2P topic identifiers.
**How to avoid:** Use `appStore.services` Map (keyed by the same hash IDs) to look up `.name`. If a hash isn't found in the Map, show the truncated hash as fallback.
**Warning signs:** Displaying raw 64-character hex strings instead of service names.

### Pitfall 3: BLS Keys Are Much Longer Than ETH Addresses

**What goes wrong:** BLS G1 pubkeys in EIP-2537 uncompressed format are 128 bytes = 256 hex chars. Displaying them full-width breaks layouts.
**Why it happens:** ETH addresses are 20 bytes (42 hex chars with 0x prefix). BLS keys are 6x longer.
**How to avoid:** Always use `AddressDisplay` with `full={false}` (default) for BLS keys. The truncation to `first6...last4` works but consider increasing to `first8...last6` for BLS keys to avoid collisions.
**Warning signs:** Layout overflow or text wrapping in the operator key display.

### Pitfall 4: On-Chain Registration Check Requires Service Registry Address

**What goes wrong:** To check if an operator key is registered on-chain (P2P-05), you need the POAStakeRegistry contract address and RPC URL for each service.
**Why it happens:** The `P2pStatus` doesn't include registry addresses. The service's `ServiceManager` has chain + address, but you need the RPC URL from `ChainConfigs` or `poaStore.registries`.
**How to avoid:** For EVM services, use `appStore.services` to get the `ServiceManager`, extract chain + address, then use `poaStore.registries` to find the cached registry data (which includes `rpcUrl` and `chainId`). If no registry is cached, use `getChainConfigs()` to get the RPC URL and create a `publicClient`.
**Warning signs:** Attempting on-chain reads without knowing the RPC endpoint.

### Pitfall 5: P2P Disabled State

**What goes wrong:** When `P2pStatus.enabled === false`, all other fields are empty/default. The page should show a clear "P2P is disabled" message instead of empty lists.
**Why it happens:** Single-operator setups use `P2pConfig::Disabled` and have no P2P network.
**How to avoid:** Check `p2pStatus.enabled` first and render a prominent disabled state card. Only render the peers/services sections when enabled.
**Warning signs:** Empty page with no explanation when P2P is off.

### Pitfall 6: WAVS Not Running

**What goes wrong:** `cmd_get_p2p_status` requires the WAVS node to be running (accesses the dispatcher). If WAVS hasn't started, it returns `AppError::WavsNotRunning`.
**Why it happens:** The P2P page may be navigated to before WAVS has finished starting.
**How to avoid:** Catch the error and show "WAVS node not running" state, similar to how Health.tsx handles the offline state.
**Warning signs:** Unhandled promise rejection on page load.

## Code Examples

### P2P Status Card (identity + discovery + addresses)

```typescript
// Source: codebase pattern from Health.tsx
function IdentityCard({ status }: { status: P2pStatus }) {
  return (
    <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h2 className="text-xl font-semibold text-beige-light mb-4">Node Identity</h2>
      <div className="grid grid-cols-2 gap-4">
        <div className="flex flex-col gap-1">
          <span className="text-tan-muted text-xs font-medium">Peer ID (Ed25519)</span>
          {status.local_peer_id ? (
            <AddressDisplay address={status.local_peer_id} />
          ) : (
            <span className="text-tan-muted italic">Not available</span>
          )}
        </div>
        <div className="flex flex-col gap-1">
          <span className="text-tan-muted text-xs font-medium">Discovery Mode</span>
          <span className="text-beige-warm">{status.discovery_mode ?? 'unknown'}</span>
        </div>
        <div className="flex flex-col gap-1 col-span-2">
          <span className="text-tan-muted text-xs font-medium">Listen Addresses</span>
          {status.listen_addresses.length > 0 ? (
            <div className="flex flex-col gap-1">
              {status.listen_addresses.map((addr, i) => (
                <span key={i} className="font-mono text-sm text-beige-warm">{addr}</span>
              ))}
            </div>
          ) : (
            <span className="text-tan-muted italic">None</span>
          )}
        </div>
      </div>
    </div>
  );
}
```

### Subscribed Service with Operator Key

```typescript
// Source: codebase pattern from ServiceDetailPage.tsx header section
function ServiceOperatorCard({
  serviceHash,
  serviceName,
  signer,
  registrationStatus,
}: {
  serviceHash: string;
  serviceName: string;
  signer: SignerResponse | null;
  registrationStatus: 'registered' | 'unregistered' | 'unknown';
}) {
  const isSecp = signer && 'secp256k1' in signer;
  const isBls = signer && 'bls12381' in signer;
  const keyDisplay = isSecp
    ? signer.secp256k1.evm_address
    : isBls
      ? signer.bls12381.g1_pubkey_hex
      : null;
  const algorithmLabel = isSecp ? 'ECDSA' : isBls ? 'BLS' : 'N/A';

  return (
    <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <div className="flex items-center gap-2 mb-3">
        <h4 className="text-beige-light font-medium">{serviceName}</h4>
        <span className="px-1.5 py-0.5 text-xs font-medium bg-charcoal-light text-beige-warm rounded">
          {algorithmLabel}
        </span>
        <RegistrationBadge status={registrationStatus} />
      </div>
      {keyDisplay && (
        <div className="flex items-center gap-2">
          <span className="text-tan-muted text-xs">Operator Key:</span>
          <AddressDisplay address={keyDisplay} />
        </div>
      )}
    </div>
  );
}
```

### SignerResponse Variant Handling

```typescript
// Source: types/index.ts SignerResponse type definition
// The SignerResponse is externally tagged: {"secp256k1": {...}} or {"bls12381": {...}}
function getSignerInfo(signer: SignerResponse): {
  algorithm: string;
  key: string;
  hdIndex: number;
} {
  if ('secp256k1' in signer) {
    return {
      algorithm: 'ECDSA (secp256k1)',
      key: signer.secp256k1.evm_address,
      hdIndex: signer.secp256k1.hd_index,
    };
  }
  return {
    algorithm: 'BLS (bls12381)',
    key: signer.bls12381.g1_pubkey_hex,
    hdIndex: signer.bls12381.hd_index,
  };
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No P2P visibility | P2P status endpoint + Tauri command | Phase 9 (2026-03-24) | Backend ready, frontend pending |
| Single algorithm (secp256k1) | secp256k1 + bls12381 dual support | Phase 9 (2026-03-24) | SignerResponse discriminated union |
| Monolithic Settings page | Decomposed section components | Phase 9 (2026-03-24) | Pattern to follow for page organization |

## Rust-Side Change Needed

### Add `discovery_mode` to `P2pStatus`

The `P2pStatus` struct in `packages/types/src/http.rs` needs a new field:

```rust
// packages/types/src/http.rs
pub struct P2pStatus {
    pub enabled: bool,
    pub discovery_mode: String,  // NEW: "disabled", "local", or "remote"
    pub local_peer_id: Option<String>,
    pub listen_addresses: Vec<String>,
    pub connected_peers: usize,
    pub peer_ids: Vec<String>,
    pub subscribed_services: Vec<String>,
}
```

The Tauri `cmd_get_p2p_status` command should read the `P2pConfig` variant from `WavsConfigState` and populate this field. Alternatively, the P2P handle's `get_status()` method can be updated to include it -- but the simpler approach is to set it in the Tauri command since it already has access to the config via the dispatcher.

The field can be populated by pattern-matching on `P2pConfig`:

```rust
let mode = match &config.p2p {
    P2pConfig::Disabled => "disabled",
    P2pConfig::Local { .. } => "local",
    P2pConfig::Remote { .. } => "remote",
};
```

**Impact:** TypeScript `P2pStatus` interface in `types/index.ts` must add `discovery_mode: string`.

## Open Questions

1. **Discovery mode field location**
   - What we know: `P2pStatus` has no discovery mode; `P2pConfig` enum has it; `WavsConfigState` is accessible from Tauri commands
   - What's unclear: Whether to add to `P2pStatus` struct (cleanest) or add a separate Tauri command (more isolated). Adding to `P2pStatus` is simpler for the frontend but changes a shared types crate.
   - Recommendation: Add `discovery_mode: String` to `P2pStatus`. It's a read-only status field and logically belongs there. Use `#[serde(default)]` for backward compatibility.

2. **P2P-06 (Stretch) feasibility**
   - What we know: The aggregator has `QuorumQueue` structs in `subsystems/aggregator/queue.rs` but no status API to expose them. The `get_p2p_status()` method doesn't expose quorum progress.
   - What's unclear: How much work it would take to expose quorum accumulation status
   - Recommendation: Show "Quorum data not available" placeholder. Implementing the backend endpoint is out of scope for Phase 10 (as noted in STATE.md: "Research flag: P2P-06 requires /aggregator/status endpoint that does not exist yet").

3. **On-chain registration check for Cosmos services**
   - What we know: The POAStakeRegistry is EVM-only. Cosmos services don't have the same signing key registration pattern.
   - What's unclear: Whether Cosmos services should show "N/A" for registration status or be omitted
   - Recommendation: Show "N/A -- EVM only" for Cosmos service registration status.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Vite dev server + manual visual verification |
| Config file | None -- no automated frontend test suite exists |
| Quick run command | `just app-dev-frontend` |
| Full suite command | `just app-dev` (full Tauri dev) |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P2P-01 | P2P page in nav, shows identity/mode/addresses | manual-only | Visual: navigate to /p2p, verify card content | N/A |
| P2P-02 | Peers list updates on interval | manual-only | Visual: start multi-operator, verify peer count updates | N/A |
| P2P-03 | Services list with human-readable names | manual-only | Visual: deploy a service, verify it appears in subscribed list | N/A |
| P2P-04 | Operator key with copy button | manual-only | Visual: verify key displays and copy button works | N/A |
| P2P-05 | Registration status badge | manual-only | Visual: register operator on-chain, verify badge changes | N/A |
| P2P-06 | Quorum progress (stretch) | manual-only | Visual: check placeholder displays correctly | N/A |

**Justification for manual-only:** This phase is pure UI construction with no business logic. The Tauri commands are already tested via the E2E suite (`just test-wavs-e2e`). Frontend component rendering is best verified visually with `just app-dev`.

### Sampling Rate

- **Per task commit:** `just app-build-frontend` (Vite build succeeds = no TS errors)
- **Per wave merge:** `just app-dev` visual smoke test
- **Phase gate:** Full visual walkthrough of all 5 success criteria

### Wave 0 Gaps

None -- existing build infrastructure (`just app-dev-frontend`, `just app-build-frontend`) covers all validation needs.

## Sources

### Primary (HIGH confidence)

- Codebase inspection: `app/src/pages/Health.tsx` -- polling dashboard pattern
- Codebase inspection: `app/src/components/layout/Header.tsx` -- nav item structure
- Codebase inspection: `app/src/tauri/commands.ts` -- `getP2pStatus()`, `getServiceSigner()`, `deriveBlsPubkey()` wrappers
- Codebase inspection: `app/src/types/index.ts` -- `P2pStatus`, `SignerResponse`, `BlsPubkeyResponse` types
- Codebase inspection: `packages/types/src/http.rs:134` -- Rust `P2pStatus` struct (no discovery_mode field)
- Codebase inspection: `packages/wavs/src/subsystems/aggregator/p2p.rs:56` -- `P2pConfig` enum (Disabled/Local/Remote)
- Codebase inspection: `app/src/contracts/POAStakeRegistry.ts` -- ABI with `operatorRegistered`, `getLatestOperatorSigningKey`
- Codebase inspection: `app/src/utils/evm.ts` -- on-chain read patterns
- Codebase inspection: `app/src-tauri/src/commands.rs` -- Tauri command implementations
- Codebase inspection: `app/src-tauri/src/state.rs` -- `WavsConfigState` holds `Config` with `p2p: P2pConfig`

### Secondary (MEDIUM confidence)

- STATE.md accumulated context: P2P-06 flagged as requiring nonexistent `/aggregator/status` endpoint

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, zero new dependencies
- Architecture: HIGH -- direct pattern reuse from Health.tsx, Header.tsx, ServiceDetailPage.tsx
- Pitfalls: HIGH -- verified by reading actual Rust types and Tauri command implementations
- Rust-side change: HIGH -- verified P2pStatus struct lacks discovery_mode, P2pConfig has the data

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable -- no external dependency changes expected)
