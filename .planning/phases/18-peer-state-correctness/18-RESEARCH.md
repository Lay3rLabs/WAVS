# Phase 18: Peer State Correctness - Research

**Researched:** 2026-04-04
**Domain:** Peer subscription cleanup on disconnect and backward-compatible targeted delivery for pre-v1.3 nodes
**Confidence:** HIGH

## Summary

Phase 18 is a gap-closure phase identified by the v1.3 milestone audit (`v1.3-MILESTONE-AUDIT.md`). Two requirements were flagged as "partial": SUB-03 (peer disconnect cleanup) and COMPAT-03 (backward compatibility with pre-v1.3 nodes). Both gaps are in `packages/wavs/src/subsystems/aggregator/p2p.rs` (~2747 lines) and require changes only to that file.

**SUB-03 gap:** `PeerSubscriptionMap.remove_peer()` is implemented and fully unit-tested (7+ tests) but never called from the bridge loops when a peer departs. commonware-p2p does not expose a peer-disconnect callback or event channel. However, the heartbeat mechanism (every 2 seconds) already broadcasts a probe to all peers and receives back the set of currently-connected peers via the broadcast acknowledgment. This gives us a reliable "who is connected right now" signal on every heartbeat tick. The fix is to compare the heartbeat ack recipients against the `peer_subscriptions` tracked peers and call `remove_peer()` for any peer that is tracked but no longer in the ack recipients.

**COMPAT-03 gap:** `has_announced()` is implemented and tested but never called in production code (dead_code warning). `get_recipients()` falls back to `Recipients::All` when the subscriber set is empty (zero subscribers for a service), which handles the case where no v1.3 peers exist. However, when at least one v1.3 peer has subscribed to a service, `get_recipients()` returns `Recipients::Some(v1.3_peers_only)`, excluding pre-v1.3 peers that have never sent a subscription announcement. The fix is to modify `get_recipients()` to accept a set of all connected peers and include any connected peer that `has_announced()` returns false for, ensuring un-announced peers are always in the recipient set.

**Primary recommendation:** In each bridge loop's heartbeat tick arm (after the ack updates `connected_peers_tracker`), compute the set difference between `peer_subscriptions`-tracked peers and heartbeat ack recipients, and call `remove_peer()` for each departed peer. Modify `get_recipients()` to accept a reference to connected peers (or a closure/set of un-announced peers) and include un-announced connected peers in the result. Both changes are identical in `run_lookup_network` and `run_discovery_network`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SUB-03 | When a peer disconnects, all its subscription entries are removed from both maps | Heartbeat ack (every 2s) provides the current connected peer set. Compare against `peer_subscriptions.peer_to_services.keys()` and call `remove_peer()` for peers present in subscriptions but absent from heartbeat ack. The `remove_peer()` method already handles both forward and reverse index cleanup (verified by 3 unit tests). |
| COMPAT-03 | Nodes without v1.3 targeting are treated as subscribed-to-all (backward compatible during rolling updates) | Modify `get_recipients()` to accept the set of all connected peers. For any connected peer where `has_announced()` returns false, include that peer unconditionally in the recipient set. This makes `has_announced()` a production-called method (removing `dead_code` warning). |
</phase_requirements>

## Standard Stack

### Core

No new dependencies. All changes use existing types and methods in p2p.rs.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `PeerSubscriptionMap::remove_peer()` | Phase 14 | Remove all subscription entries for a departed peer | Already implemented and tested; 3+ unit tests cover forward/reverse cleanup |
| `PeerSubscriptionMap::has_announced()` | Phase 15 | Check if peer has ever sent a subscription announcement | Already implemented and tested; 2 unit tests cover it |
| `PeerSubscriptionMap::get_recipients()` | Phase 14 | Resolve service_id to Recipients enum | Needs modification to accept connected peers for COMPAT-03 |
| `commonware-p2p::Recipients` | 2026.3.0 | `Recipients::All` / `Recipients::Some(Vec<P>)` / `Recipients::One(P)` | Already imported and used |
| `std::collections::HashSet<ed25519::PublicKey>` | stdlib | Set operations for peer comparison | Already imported |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Heartbeat-based pruning | commonware peer disconnect callback | commonware-p2p does not expose disconnect events -- heartbeat is the only mechanism |
| Modifying `get_recipients()` signature | Separate wrapper function | Modifying the signature is cleaner; all 6 callers already have access to the connected peers set |
| Passing `HashSet<PublicKey>` to `get_recipients()` | Passing `&[ed25519::PublicKey]` (slice from heartbeat ack) | HashSet enables O(1) lookups for `has_announced()` check; Vec requires O(n) scan per peer |

## Architecture Patterns

### Recommended File Structure

All changes in one existing file:
```
packages/wavs/src/subsystems/aggregator/p2p.rs
```

No new files. No changes outside this file.

### Pattern 1: Heartbeat-Driven Peer Pruning (SUB-03)

**What:** After the heartbeat broadcast ack updates `connected_peers_tracker`, compare the ack recipients against the set of peers tracked in `peer_subscriptions`. For any peer present in subscriptions but absent from the heartbeat ack, call `remove_peer()`.

**When to use:** In the heartbeat tick arm of both `run_lookup_network` and `run_discovery_network`, immediately after the `Ok(recipients) if !recipients.is_empty()` block that updates `connected_peers_tracker`.

**Why heartbeat is reliable:** The heartbeat broadcasts `Recipients::All` every 2 seconds. The ack returns the set of peers that received the message -- this is the authoritative "currently connected" set from commonware-p2p's perspective. Any peer not in this set is unreachable.

**Edge case -- empty heartbeat ack:** When `recipients.is_empty()` (no peers connected), the code enters the `Ok(_) => tracing::trace!("Heartbeat: no peers connected yet")` arm. In this case, ALL peers should be pruned from subscriptions. However, this is already handled correctly by the next heartbeat with peers -- once peers reconnect and send heartbeat announcements (ANN-03), their subscriptions are re-established via `set_peer_subscriptions()`. No explicit prune-all is needed for the empty case because the "no peers connected" path does not update `connected_peers_tracker` -- it leaves the tracker unchanged (empty from init or from last write). The prune logic should only run in the `!recipients.is_empty()` branch where we have a definitive peer set.

**Implementation approach -- add method to PeerSubscriptionMap:**

```rust
/// Returns the set of peers that have subscription entries in this map.
/// Used by heartbeat pruning to compare against connected peers (SUB-03).
pub fn tracked_peers(&self) -> HashSet<ed25519::PublicKey> {
    self.peer_to_services.keys().cloned().collect()
}
```

Then in the heartbeat tick arm:

```rust
// SUB-03: Prune departed peers from subscription map.
// The heartbeat ack `recipients` is the authoritative connected peer set.
let connected: HashSet<ed25519::PublicKey> = recipients.iter().cloned().collect();
let tracked = peer_subscriptions.tracked_peers();
for departed in tracked.difference(&connected) {
    peer_subscriptions.remove_peer(departed);
    tracing::debug!(
        "Pruned departed peer from subscriptions: {}",
        const_hex::encode(departed.as_ref()),
    );
}
```

**Also prune from `known_peers`:** When a peer departs, it should also be removed from the `known_peers` HashSet so that when it reconnects, it receives a fresh hello announcement (ANN-04). This is important because after reconnection, the new peer's subscription state may have changed.

```rust
for departed in tracked.difference(&connected) {
    peer_subscriptions.remove_peer(departed);
    known_peers.remove(departed);
    tracing::debug!(
        "Pruned departed peer from subscriptions: {}",
        const_hex::encode(departed.as_ref()),
    );
}
```

### Pattern 2: Connected-Peer-Aware Recipient Resolution (COMPAT-03)

**What:** Modify `get_recipients()` to include un-announced connected peers in the recipient set. This ensures pre-v1.3 nodes (which never send subscription announcements) are included in targeted delivery.

**When to use:** All 6 `get_recipients()` call sites in both bridge loops (3 per loop: Publish handler, retry drain in Publish, retry drain in heartbeat).

**Signature change:**

```rust
/// Get the recipient set for targeted delivery.
/// Includes un-announced connected peers for backward compatibility (COMPAT-03).
/// Returns Recipients::Some(peers) if peers are known, or Recipients::All as fallback.
pub fn get_recipients(
    &self,
    service_id: &[u8; 32],
    connected_peers: &HashSet<ed25519::PublicKey>,
) -> Recipients<ed25519::PublicKey> {
    let mut recipients: HashSet<ed25519::PublicKey> = HashSet::new();

    // Add subscribed peers for this service
    if let Some(peers) = self.service_to_peers.get(service_id) {
        recipients.extend(peers.iter().cloned());
    }

    // COMPAT-03: Include connected peers that have never announced
    // (pre-v1.3 nodes treated as subscribed-to-all)
    for peer in connected_peers {
        if !self.has_announced(peer) {
            recipients.insert(peer.clone());
        }
    }

    if recipients.is_empty() {
        Recipients::All
    } else {
        Recipients::Some(recipients.into_iter().collect())
    }
}
```

**Connected peers source:** In the bridge loops, the heartbeat ack `recipients` already provides `Vec<ed25519::PublicKey>`. Build a `HashSet<ed25519::PublicKey>` from this and keep it as bridge loop state (updated on each heartbeat and broadcast ack). This is more efficient than converting back from the hex-string `connected_peers_tracker`.

**New bridge loop state variable:**

```rust
// Connected peer set (PublicKey form) for COMPAT-03 recipient resolution.
// Updated from heartbeat and broadcast ack results.
let mut connected_peer_set: HashSet<ed25519::PublicKey> = HashSet::new();
```

Updated in the heartbeat and broadcast ack paths:

```rust
// Where connected_peers_tracker is updated from recipients:
connected_peer_set = recipients.iter().cloned().collect();
```

All `get_recipients()` calls then pass `&connected_peer_set`:

```rust
let direct_recipients = peer_subscriptions.get_recipients(
    &service_id.inner(),
    &connected_peer_set,
);
```

### Pattern 3: Identical Changes in Both Bridge Loops

**What:** Both `run_lookup_network` (line 641) and `run_discovery_network` (line 1140) have character-for-character identical bridge loop logic. All changes in this phase must be applied identically to both.

**Locations (lookup loop):**
- New state variable: after line 817 (`known_peers`)
- Heartbeat prune: inside the `Ok(recipients) if !recipients.is_empty()` arm of the heartbeat tick (around line 1081)
- get_recipients calls: lines 844, 871, 1093 (Publish, Publish retry, heartbeat retry)

**Locations (discovery loop):**
- New state variable: after line 1302 (`known_peers`)
- Heartbeat prune: inside the `Ok(recipients) if !recipients.is_empty()` arm of the heartbeat tick (around line 1559)
- get_recipients calls: lines 1327, 1351, 1571 (Publish, Publish retry, heartbeat retry)

**Total changes:** 6 `get_recipients()` call site updates (3 per loop) + 2 heartbeat prune blocks (1 per loop) + 2 new state variables (1 per loop).

### Anti-Patterns to Avoid

- **Do NOT add a peer-disconnect event arm to `tokio::select!`:** commonware-p2p does not expose disconnect events. The heartbeat mechanism is the correct approach.
- **Do NOT prune on the empty-ack branch:** When `recipients.is_empty()`, no peers are connected. Pruning all subscriptions is unnecessary -- they will be repopulated on reconnect via heartbeat announcements (ANN-03). Only prune when we have a definitive connected set.
- **Do NOT convert hex strings back to PublicKey for comparison:** The heartbeat ack already provides `Vec<ed25519::PublicKey>`. Use these directly rather than round-tripping through `connected_peers_tracker`'s hex strings.
- **Do NOT change the `get_recipients()` call sites for Engine channel (channel 0):** Engine uses `mailbox.broadcast(Recipients::All, ...)` which does not call `get_recipients()`. Only the direct channel (channel 1) uses targeted delivery.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Peer disconnect detection | Custom TCP keepalive or ping/pong | Heartbeat ack recipient diffing | Heartbeat already runs every 2s; ack is authoritative connected set |
| Set difference computation | Manual loop with contains() | `HashSet::difference()` | Stdlib is correct, tested, and readable |
| Peer announcement tracking | Manual flag HashMap | `PeerSubscriptionMap::has_announced()` | Already implemented and tested |

## Common Pitfalls

### Pitfall 1: Pruning During Transient Network Partitions
**What goes wrong:** A brief network blip causes a peer to be absent from one heartbeat ack, triggering `remove_peer()`. When the peer recovers 100ms later, its subscriptions are gone until the next heartbeat announcement.
**Why it happens:** Heartbeat is every 2 seconds; transient disconnects within that window cause false positives.
**How to avoid:** This is acceptable behavior because ANN-03 (heartbeat subscription piggybacking) runs every 2 seconds. When the peer reconnects and sends its next heartbeat, `set_peer_subscriptions()` restores full state. Maximum subscription gap is ~4 seconds (1 missed heartbeat + 1 reconvergence). Additionally, channel 0 (Engine) continues using `Recipients::All` as catch-up safety net.
**Warning signs:** Frequent "Pruned departed peer" log messages followed immediately by subscription restore logs.

### Pitfall 2: Modifying get_recipients() Breaks Existing Unit Tests
**What goes wrong:** Existing unit tests for `get_recipients()` do not pass a `connected_peers` parameter.
**Why it happens:** The signature changes from `(&self, &[u8; 32])` to `(&self, &[u8; 32], &HashSet<ed25519::PublicKey>)`.
**How to avoid:** Update all 5 existing unit tests that call `get_recipients()` to pass an empty `HashSet` (which preserves the previous behavior -- no un-announced peers to include). Then add new tests for the COMPAT-03 behavior with non-empty connected peer sets.
**Warning signs:** Compiler errors on test code after signature change.

### Pitfall 3: Forgetting to Prune `known_peers` Alongside Subscriptions
**What goes wrong:** A departed peer is pruned from `peer_subscriptions` but not from `known_peers`. When it reconnects, the `!known_peers.contains(&peer_pubkey)` check on line 991/1469 returns false, so no hello announcement is sent. The peer misses the subscription state until the next heartbeat.
**Why it happens:** `known_peers` and `peer_subscriptions` are independent state variables that must stay consistent.
**How to avoid:** Always prune both in the same block: `peer_subscriptions.remove_peer()` + `known_peers.remove()`.
**Warning signs:** Reconnected peers not receiving hello announcements, delayed subscription convergence.

### Pitfall 4: Race Between Publish and Heartbeat Prune
**What goes wrong:** A Publish command fires between two heartbeat ticks. The `connected_peer_set` variable was last updated 1.5 seconds ago and may be slightly stale.
**Why it happens:** `connected_peer_set` is updated from heartbeat and broadcast acks, not continuously.
**How to avoid:** This is inherently acceptable -- the Publish handler also receives broadcast acks that update `connected_peer_set`. Additionally, the Engine channel (channel 0) provides `Recipients::All` delivery as a reliability safety net. The subscription state is eventually consistent, not strongly consistent.
**Warning signs:** None -- this is by design.

## Code Examples

### Example 1: New `tracked_peers()` Method

```rust
// Source: PeerSubscriptionMap implementation in p2p.rs
/// Returns the set of peers that have subscription entries in this map.
/// Used by heartbeat pruning to compare against connected peers (SUB-03).
pub fn tracked_peers(&self) -> HashSet<ed25519::PublicKey> {
    self.peer_to_services.keys().cloned().collect()
}
```

### Example 2: Heartbeat Prune Block (Both Loops)

```rust
// In heartbeat tick arm, after connected_peers_tracker update and retry drain:

// SUB-03: Prune subscription entries for departed peers.
// Heartbeat ack recipients are the authoritative connected peer set.
let connected_set: HashSet<ed25519::PublicKey> = recipients.iter().cloned().collect();
connected_peer_set = connected_set.clone();
let tracked = peer_subscriptions.tracked_peers();
for departed in tracked.difference(&connected_set) {
    peer_subscriptions.remove_peer(departed);
    known_peers.remove(departed);
    tracing::debug!(
        "Pruned departed peer: {}",
        const_hex::encode(departed.as_ref()),
    );
}
```

### Example 3: Modified `get_recipients()` (COMPAT-03)

```rust
// Source: PeerSubscriptionMap implementation in p2p.rs
/// Get the recipient set for targeted delivery.
/// Includes un-announced connected peers for backward compatibility (COMPAT-03).
/// Returns Recipients::Some(peers) if any peers are resolved, or Recipients::All as fallback.
pub fn get_recipients(
    &self,
    service_id: &[u8; 32],
    connected_peers: &HashSet<ed25519::PublicKey>,
) -> Recipients<ed25519::PublicKey> {
    let mut result: HashSet<ed25519::PublicKey> = HashSet::new();

    // Add peers subscribed to this specific service
    if let Some(peers) = self.service_to_peers.get(service_id) {
        result.extend(peers.iter().cloned());
    }

    // COMPAT-03: Include connected peers that have never announced.
    // Pre-v1.3 nodes never send subscription announcements, so they must be
    // included unconditionally to maintain backward-compatible delivery.
    for peer in connected_peers {
        if !self.has_announced(peer) {
            result.insert(peer.clone());
        }
    }

    if result.is_empty() {
        Recipients::All
    } else {
        Recipients::Some(result.into_iter().collect())
    }
}
```

### Example 4: Updated Call Site (Publish Handler)

```rust
// Before (current):
let direct_recipients = peer_subscriptions.get_recipients(&service_id.inner());

// After:
let direct_recipients = peer_subscriptions.get_recipients(
    &service_id.inner(),
    &connected_peer_set,
);
```

### Example 5: Updated Unit Test (Existing)

```rust
// Before (current):
assert!(matches!(map.get_recipients(&unknown_svc), Recipients::All));

// After (pass empty set to preserve behavior):
let empty_connected = HashSet::new();
assert!(matches!(
    map.get_recipients(&unknown_svc, &empty_connected),
    Recipients::All
));
```

### Example 6: New COMPAT-03 Unit Test

```rust
#[test]
fn test_get_recipients_includes_unannounced_connected_peers() {
    // COMPAT-03: Un-announced connected peers are included in recipient set
    let peer_v13 = test_pubkey(1);  // v1.3 peer (has announced)
    let peer_legacy = test_pubkey(2);  // pre-v1.3 peer (never announced)
    let svc_a = [0xAA; 32];

    let mut map = PeerSubscriptionMap::new();

    // peer_v13 subscribes to svc_a
    map.handle_announcement(
        &peer_v13,
        &SubscriptionAnnouncement {
            subscribe: vec![svc_a],
            unsubscribe: vec![],
            full_state: false,
        },
    );

    // Both peers are connected
    let connected: HashSet<ed25519::PublicKey> =
        [peer_v13.clone(), peer_legacy.clone()].into_iter().collect();

    match map.get_recipients(&svc_a, &connected) {
        Recipients::Some(peers) => {
            assert!(peers.contains(&peer_v13), "v1.3 peer must be included");
            assert!(peers.contains(&peer_legacy), "Legacy peer must be included (COMPAT-03)");
            assert_eq!(peers.len(), 2);
        }
        other => panic!("Expected Recipients::Some with 2 peers, got {:?}", other),
    }
}

#[test]
fn test_get_recipients_all_announced_no_legacy() {
    // When all connected peers have announced, only subscribed peers are included
    let peer_a = test_pubkey(1);
    let peer_b = test_pubkey(2);
    let svc_a = [0xAA; 32];
    let svc_b = [0xBB; 32];

    let mut map = PeerSubscriptionMap::new();

    // peer_a subscribes to svc_a, peer_b subscribes to svc_b
    map.handle_announcement(
        &peer_a,
        &SubscriptionAnnouncement {
            subscribe: vec![svc_a],
            unsubscribe: vec![],
            full_state: false,
        },
    );
    map.handle_announcement(
        &peer_b,
        &SubscriptionAnnouncement {
            subscribe: vec![svc_b],
            unsubscribe: vec![],
            full_state: false,
        },
    );

    // Both peers are connected and have announced
    let connected: HashSet<ed25519::PublicKey> =
        [peer_a.clone(), peer_b.clone()].into_iter().collect();

    match map.get_recipients(&svc_a, &connected) {
        Recipients::Some(peers) => {
            assert_eq!(peers.len(), 1, "Only peer_a subscribes to svc_a");
            assert!(peers.contains(&peer_a));
        }
        other => panic!("Expected Recipients::Some with 1 peer, got {:?}", other),
    }
}
```

### Example 7: New SUB-03 Unit Test

```rust
#[test]
fn test_heartbeat_prune_departed_peer() {
    // SUB-03: Simulates heartbeat-driven pruning of departed peers
    let peer_a = test_pubkey(1);
    let peer_b = test_pubkey(2);
    let svc_a = [0xAA; 32];

    let mut map = PeerSubscriptionMap::new();
    map.handle_announcement(
        &peer_a,
        &SubscriptionAnnouncement {
            subscribe: vec![svc_a],
            unsubscribe: vec![],
            full_state: false,
        },
    );
    map.handle_announcement(
        &peer_b,
        &SubscriptionAnnouncement {
            subscribe: vec![svc_a],
            unsubscribe: vec![],
            full_state: false,
        },
    );

    // Both peers subscribed
    let empty_connected = HashSet::new();
    match map.get_recipients(&svc_a, &empty_connected) {
        Recipients::Some(peers) => assert_eq!(peers.len(), 2),
        other => panic!("Expected 2 peers, got {:?}", other),
    }

    // Heartbeat ack only returns peer_a -- peer_b departed
    let connected: HashSet<ed25519::PublicKey> = [peer_a.clone()].into_iter().collect();
    let tracked = map.tracked_peers();
    for departed in tracked.difference(&connected) {
        map.remove_peer(departed);
    }

    // After prune: only peer_a remains for svc_a
    match map.get_recipients(&svc_a, &connected) {
        Recipients::Some(peers) => {
            assert_eq!(peers.len(), 1);
            assert!(peers.contains(&peer_a));
        }
        other => panic!("Expected 1 peer, got {:?}", other),
    }
}
```

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test (`#[cfg(test)]`) + cargo test |
| Config file | `packages/wavs/Cargo.toml` (package `wavs`) |
| Quick run command | `cargo test -p wavs -- p2p_broadcast_tests --lib` |
| Full suite command | `cargo test -p wavs` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SUB-03 | `tracked_peers()` returns correct set | unit | `cargo test -p wavs -- test_tracked_peers -x` | Wave 0 |
| SUB-03 | Heartbeat prune removes departed peer | unit | `cargo test -p wavs -- test_heartbeat_prune_departed_peer -x` | Wave 0 |
| SUB-03 | Prune no-ops when all peers still connected | unit | `cargo test -p wavs -- test_prune_noop_all_connected -x` | Wave 0 |
| COMPAT-03 | Un-announced connected peers included in recipients | unit | `cargo test -p wavs -- test_get_recipients_includes_unannounced -x` | Wave 0 |
| COMPAT-03 | All-announced peers: only subscribed included | unit | `cargo test -p wavs -- test_get_recipients_all_announced -x` | Wave 0 |
| COMPAT-03 | `has_announced()` called in production code (no dead_code) | compile | `cargo clippy -p wavs -- -D dead_code` | Implicit |
| COMPAT-01 | Existing secp256k1 e2e tests pass | e2e | `just test-wavs-e2e` | Existing |
| COMPAT-02 | Existing BLS e2e tests pass | e2e | `just test-wavs-e2e` | Existing |

### Sampling Rate

- **Per task commit:** `cargo test -p wavs -- p2p_broadcast_tests --lib && cargo clippy -p wavs -- -D warnings`
- **Per wave merge:** `cargo test -p wavs`
- **Phase gate:** Full suite green + `just lint` clean before verify

### Wave 0 Gaps

- [ ] `test_tracked_peers` -- new test for `tracked_peers()` method
- [ ] `test_heartbeat_prune_departed_peer` -- new test for SUB-03 pruning
- [ ] `test_prune_noop_all_connected` -- prune is no-op when all tracked peers are connected
- [ ] `test_get_recipients_includes_unannounced_connected_peers` -- COMPAT-03 behavior
- [ ] `test_get_recipients_all_announced_no_legacy` -- no-legacy path
- [ ] Update 5 existing `get_recipients()` tests to pass `&HashSet::new()` (signature change)

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No peer pruning; stale entries persist until restart | Heartbeat-driven pruning every 2s | Phase 18 | SUB-03 satisfied; subscription map reflects reality |
| `get_recipients()` ignores un-announced peers | `get_recipients()` includes un-announced connected peers | Phase 18 | COMPAT-03 satisfied; rolling upgrades safe |
| `has_announced()` dead_code | `has_announced()` called in production `get_recipients()` | Phase 18 | Clippy warning resolved |

## Open Questions

1. **Should `connected_peer_set` be updated from broadcast acks (Publish handler) in addition to heartbeat acks?**
   - What we know: Broadcast acks also return `Vec<ed25519::PublicKey>`. Currently both heartbeat and broadcast acks update `connected_peers_tracker` (hex strings).
   - What's unclear: Whether updating `connected_peer_set` (PublicKey form) from broadcast acks provides meaningful benefit over heartbeat-only.
   - Recommendation: YES, update from both ack sources for consistency. The broadcast ack path already updates `connected_peers_tracker`; add `connected_peer_set` update in the same block. This makes `get_recipients()` use the freshest possible connected set. Low-risk addition.

2. **Should prune logic also run when a broadcast ack (Publish handler) returns recipients?**
   - What we know: The requirement says "within one heartbeat cycle." Publish-triggered prunes would be more aggressive.
   - What's unclear: Whether more-frequent pruning adds value or just churn.
   - Recommendation: NO, only prune on heartbeat. The heartbeat is the dedicated "who is connected" probe. Publish-triggered acks may have different semantics (message-specific delivery vs. mesh probe). Keep the prune trigger clear and predictable.

## Sources

### Primary (HIGH confidence)

- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Direct code inspection of PeerSubscriptionMap (lines 357-466), bridge loops (lines 641-1600), test suite (lines 1764-2607)
- `.planning/v1.3-MILESTONE-AUDIT.md` - Gap identification for SUB-03 and COMPAT-03 with root cause analysis
- `.planning/REQUIREMENTS.md` - Requirement definitions and traceability table
- `.planning/phases/14-subscription-data-structures/14-RESEARCH.md` - Original PeerSubscriptionMap design decisions
- `.planning/phases/16-targeted-delivery/16-RESEARCH.md` - get_recipients() usage patterns and channel classification

### Secondary (MEDIUM confidence)

- `.planning/STATE.md` - Decision history confirming bridge loop duplication pattern and known_peers/has_announced relationship
- `.planning/PROJECT.md` - Architectural constraints on coexistence and backward compatibility

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - No new dependencies; all building blocks exist and are tested
- Architecture: HIGH - Direct code inspection of both bridge loops, exact line numbers, exact method signatures
- Pitfalls: HIGH - All pitfalls derived from concrete code analysis of the heartbeat/ack flow

**Research date:** 2026-04-04
**Valid until:** 2026-05-04 (stable -- changes are internal to p2p.rs, no external API dependencies)
