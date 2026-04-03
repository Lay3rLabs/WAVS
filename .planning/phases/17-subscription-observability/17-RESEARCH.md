# Phase 17: Subscription Observability - Research

**Researched:** 2026-04-03
**Domain:** Exposing per-service peer subscription state via the HTTP API for operator debugging
**Confidence:** HIGH

## Summary

Phase 17 is the final phase of the v1.3 milestone. It adds a single observability feature: the `/p2p/status` endpoint response includes a `peer_subscriptions` field showing per-service peer counts derived from the live `PeerSubscriptionMap` state. This is a pure read-only addition -- no changes to P2P protocol, data structures, or delivery logic.

The implementation is straightforward because all the infrastructure exists. `PeerSubscriptionMap` already tracks `service_to_peers: HashMap<[u8; 32], HashSet<ed25519::PublicKey>>` (forward index) and `peer_to_services: HashMap<ed25519::PublicKey, HashSet<[u8; 32]>>` (reverse index). The `GetStatus` command handler runs inside the same bridge loop `tokio::select!` arm where `peer_subscriptions` lives as a local variable, so no cross-thread sharing (Arc/RwLock) is needed. We add a snapshot method to `PeerSubscriptionMap`, add a field to the `P2pStatus` struct, populate it in both bridge loops' GetStatus handlers, and update the TypeScript type in the Tauri app.

The scope touches 4 files with approximately 25 lines of new code plus tests. The Tauri app TypeScript type should be updated for consistency, but the REQUIREMENTS.md scopes OBS-01 to the HTTP API only, and the project constraints explicitly state "Backend-only milestone; P2P status already displays in existing UI." The TypeScript update is a minor housekeeping item, not a hard requirement.

**Primary recommendation:** Add a `pub fn peer_subscription_counts(&self) -> HashMap<String, usize>` method to `PeerSubscriptionMap` that iterates `service_to_peers` and returns `{hex_service_id: peer_count}`. Add `peer_subscriptions: HashMap<String, usize>` to `P2pStatus`. Populate it in both bridge loops' GetStatus handlers by calling `peer_subscriptions.peer_subscription_counts()`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| OBS-01 | `/p2p/status` endpoint includes per-service peer counts (how many peers subscribe to each service) | Add `peer_subscriptions: HashMap<String, usize>` field to `P2pStatus` struct (packages/types/src/http.rs). Add `peer_subscription_counts()` method to `PeerSubscriptionMap` (packages/wavs/src/subsystems/aggregator/p2p.rs). Populate the new field in both GetStatus handlers (lookup line ~913, discovery line ~1392). |
</phase_requirements>

## Standard Stack

### Core

No new dependencies. All code uses existing types and methods already present in the codebase.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `PeerSubscriptionMap` | Phase 14 | Bidirectional service-to-peer subscription index | Already built and tested with 10+ unit tests |
| `P2pStatus` | Phase 3 | HTTP response struct for `/p2p/status` endpoint | Already defined with Serialize/Deserialize/ToSchema derives |
| `std::collections::HashMap` | stdlib | Return type for subscription counts | Already imported everywhere |
| `const_hex` | existing dep | Hex-encode service_id bytes for JSON | Already used in P2pStatus for peer_ids |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `HashMap<String, usize>` (service_id hex -> count) | `Vec<(String, usize)>` | HashMap is more natural for JSON serialization and client consumption; Vec loses key semantics |
| `HashMap<String, usize>` (counts only) | `HashMap<String, Vec<String>>` (full peer list) | Counts are sufficient for OBS-01 and avoid leaking peer identity data; full lists can be added later if needed |
| Hex-encoded service_id keys | Raw `[u8; 32]` keys | JSON requires string keys; hex encoding matches existing `subscribed_services` field pattern |

## Architecture Patterns

### Change Map

```
packages/types/src/http.rs            # Add peer_subscriptions field to P2pStatus
packages/wavs/src/subsystems/aggregator/p2p.rs  # (1) Add peer_subscription_counts() method to PeerSubscriptionMap
                                                 # (2) Populate field in lookup GetStatus handler (~line 913)
                                                 # (3) Populate field in discovery GetStatus handler (~line 1392)
                                                 # (4) Unit tests for new method
packages/wavs/src/subsystems/aggregator/p2p_status_tests.rs  # Update serialization test for new field
app/src/types/index.ts                 # Update TypeScript P2pStatus interface (housekeeping)
```

### Pattern: Snapshot from Local Variable

The `peer_subscriptions` (`PeerSubscriptionMap`) is a local `let mut` variable inside each bridge loop function (`run_lookup_network`, `run_discovery_network`). The `GetStatus` command handler runs in the SAME `tokio::select!` arm as all other command handlers, meaning it has direct access to `peer_subscriptions` with no synchronization needed. This is the same pattern used for `service_router.subscribed_services()` on the existing `subscribed_services` field.

```rust
// Inside GetStatus handler (both bridge loops):
Some(P2pCommand::GetStatus { response_tx }) => {
    let peers = connected_peers_tracker.read().unwrap().clone();
    let status = P2pStatus {
        enabled: true,
        // ... existing fields ...
        subscribed_services: service_router.subscribed_services(),
        peer_subscriptions: peer_subscriptions.peer_subscription_counts(), // NEW
    };
    let _ = response_tx.send(status);
}
```

### Pattern: Hex-Encoded Keys Consistent with Existing Fields

The existing `subscribed_services` field uses `Vec<String>` where each string is a hex-encoded service_id. The new `peer_subscriptions` field uses the same hex encoding for service_id keys, ensuring consistency in the JSON API.

### Anti-Patterns to Avoid

- **Arc/RwLock for subscription data**: NOT needed. The GetStatus handler runs inside the bridge loop where `peer_subscriptions` is a local variable. Adding shared state would introduce unnecessary complexity and synchronization overhead.
- **Exposing full peer lists per service**: The requirement asks for "per-service peer counts", not full peer identity lists. Counts are sufficient for debugging quorum issues without leaking operator identity information.
- **Separate snapshot thread**: Overkill. The snapshot method iterates a HashMap and counts set sizes -- this is O(N) where N is the number of services (typically < 100), completing in microseconds.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Subscription counting | Manual iteration over both maps | Single method on PeerSubscriptionMap that iterates forward index | Encapsulates internal data structure; testable independently |
| Hex encoding | Manual hex string formatting | `const_hex::encode()` | Already used everywhere in the codebase for peer_ids, service_ids |
| JSON serialization | Manual JSON building | serde derive on P2pStatus (already present) | Adding a HashMap<String, usize> field with serde(default) auto-serializes |

## Common Pitfalls

### Pitfall 1: Forgetting serde(default) on the new field

**What goes wrong:** Existing serialized P2pStatus from P2P-disabled nodes or older code paths that return `P2pStatus::default()` would fail to deserialize if the field is required.
**Why it happens:** The `P2pStatus` struct has `#[derive(Default)]` and is sometimes constructed via `Default::default()` (e.g., when P2P is disabled).
**How to avoid:** Add `#[serde(default)]` on the `peer_subscriptions` field. HashMap's Default is an empty map, which is correct for P2P-disabled nodes.
**Warning signs:** Deserialization errors in the CLI client or Tauri app when connecting to nodes with P2P disabled.

### Pitfall 2: Forgetting to update BOTH bridge loops

**What goes wrong:** The lookup loop GetStatus handler returns subscription data, but the discovery loop handler does not (or vice versa).
**Why it happens:** There are TWO nearly-identical bridge loops: `run_lookup_network` (~line 913) and `run_discovery_network` (~line 1392). Both construct P2pStatus independently.
**How to avoid:** Search for ALL `P2pStatus {` constructors in p2p.rs and update every one. There are exactly 2: one per bridge loop.
**Warning signs:** Subscription data appears in `/p2p/status` only when using one discovery mode.

### Pitfall 3: Not updating the TypeScript type

**What goes wrong:** The Tauri app receives the new `peer_subscriptions` field from the Rust backend but the TypeScript interface doesn't declare it, so it's silently ignored (TypeScript structural typing).
**Why it happens:** `P2pStatus` is mirrored manually in `app/src/types/index.ts`.
**How to avoid:** Update the TypeScript interface to include `peer_subscriptions: Record<string, number>`.
**Warning signs:** No runtime error, but the data is invisible in the desktop app's P2P status page.

### Pitfall 4: Returning empty map vs. not present

**What goes wrong:** Ambiguity between "no subscriptions tracked" (P2P active, no peers have announced) and "subscriptions not available" (P2P disabled).
**Why it happens:** Both cases produce an empty HashMap.
**How to avoid:** This is acceptable for OBS-01. The `enabled` field on P2pStatus already distinguishes these cases. An empty `peer_subscriptions` map combined with `enabled: true` means no peers have announced yet. An empty map with `enabled: false` means P2P is disabled.
**Warning signs:** None -- this is the correct behavior.

## Code Examples

### New method on PeerSubscriptionMap

```rust
// Source: packages/wavs/src/subsystems/aggregator/p2p.rs
// Add to impl PeerSubscriptionMap { ... }

/// Returns per-service peer counts for observability (OBS-01).
/// Keys are hex-encoded service_id bytes, values are the number of peers
/// subscribed to that service.
pub fn peer_subscription_counts(&self) -> HashMap<String, usize> {
    self.service_to_peers
        .iter()
        .map(|(service_id, peers)| (const_hex::encode(service_id), peers.len()))
        .collect()
}
```

### Updated P2pStatus struct

```rust
// Source: packages/types/src/http.rs

#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct P2pStatus {
    pub enabled: bool,
    #[serde(default)]
    pub discovery_mode: String,
    pub local_peer_id: Option<String>,
    pub listen_addresses: Vec<String>,
    pub connected_peers: usize,
    pub peer_ids: Vec<String>,
    pub subscribed_services: Vec<String>,
    /// Per-service peer subscription counts (OBS-01).
    /// Keys are hex-encoded service_id hashes, values are the number of
    /// peers that have announced subscription to that service.
    #[serde(default)]
    pub peer_subscriptions: HashMap<String, usize>,
}
```

### Updated GetStatus handler (both loops)

```rust
// Source: packages/wavs/src/subsystems/aggregator/p2p.rs (lookup loop ~line 911)
Some(P2pCommand::GetStatus { response_tx }) => {
    let peers = connected_peers_tracker.read().unwrap().clone();
    let status = P2pStatus {
        enabled: true,
        discovery_mode: "local".to_string(),
        local_peer_id: Some(const_hex::encode(own_pubkey.as_ref())),
        listen_addresses: vec![listen_addr.to_string()],
        connected_peers: peers.len(),
        peer_ids: peers,
        subscribed_services: service_router.subscribed_services(),
        peer_subscriptions: peer_subscriptions.peer_subscription_counts(),
    };
    let _ = response_tx.send(status);
}
```

### Updated TypeScript interface

```typescript
// Source: app/src/types/index.ts
export interface P2pStatus {
  enabled: boolean;
  discovery_mode: string;
  local_peer_id: string | null;
  listen_addresses: string[];
  connected_peers: number;
  peer_ids: string[];
  subscribed_services: string[];
  peer_subscriptions: Record<string, number>;  // OBS-01
}
```

### Unit test for new method

```rust
#[test]
fn test_peer_subscription_counts() {
    let peer_a = test_pubkey(1);
    let peer_b = test_pubkey(2);
    let svc_a = [0xAA; 32];
    let svc_b = [0xBB; 32];

    let mut map = PeerSubscriptionMap::new();

    // Empty map returns empty counts
    assert!(map.peer_subscription_counts().is_empty());

    // After subscriptions, counts reflect state
    map.set_peer_subscriptions(&peer_a, vec![svc_a, svc_b]);
    map.set_peer_subscriptions(&peer_b, vec![svc_a]);

    let counts = map.peer_subscription_counts();
    assert_eq!(counts.len(), 2);
    assert_eq!(*counts.get(&const_hex::encode(svc_a)).unwrap(), 2);
    assert_eq!(*counts.get(&const_hex::encode(svc_b)).unwrap(), 1);

    // After removing a peer, counts update
    map.remove_peer(&peer_b);
    let counts = map.peer_subscription_counts();
    assert_eq!(*counts.get(&const_hex::encode(svc_a)).unwrap(), 1);
}
```

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test + cargo test |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p wavs --lib -- p2p` |
| Full suite command | `cargo test -p wavs --lib` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OBS-01 | peer_subscription_counts returns correct counts from PeerSubscriptionMap | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_counts -x` | Wave 0 |
| OBS-01 | P2pStatus serializes peer_subscriptions field correctly | unit | `cargo test -p wavs --lib -- p2p_status_tests::p2p_status_format -x` | Existing (update needed) |
| OBS-01 | P2pStatus default has empty peer_subscriptions | unit | `cargo test -p wavs --lib -- p2p_status_tests -x` | Existing (update needed) |

### Sampling Rate

- **Per task commit:** `cargo test -p wavs --lib -- p2p`
- **Per wave merge:** `just lint && cargo test -p wavs --lib`
- **Phase gate:** `just lint` clean + all unit tests pass

### Wave 0 Gaps

- [ ] `test_peer_subscription_counts` -- new unit test in p2p_broadcast_tests module
- [ ] Update `p2p_status_format` test to verify `peer_subscriptions` field exists in serialized output

## Sources

### Primary (HIGH confidence)

- Direct code inspection of `packages/wavs/src/subsystems/aggregator/p2p.rs` -- PeerSubscriptionMap struct, GetStatus handlers at lines 911-923 and 1390-1402
- Direct code inspection of `packages/types/src/http.rs` -- P2pStatus struct definition at line 132-151
- Direct code inspection of `packages/wavs/src/http/handlers/p2p.rs` -- HTTP handler
- Direct code inspection of `app/src/types/index.ts` -- TypeScript P2pStatus mirror at line 214-222
- Direct code inspection of `packages/wavs/src/subsystems/aggregator.rs` -- get_p2p_status() at line 120-126

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all existing code
- Architecture: HIGH -- direct code inspection confirms GetStatus runs inside bridge loop with local access to peer_subscriptions
- Pitfalls: HIGH -- identified from direct code inspection of both bridge loops and existing patterns

**Research date:** 2026-04-03
**Valid until:** 2026-05-03 (stable -- internal code, no external dependency changes)
