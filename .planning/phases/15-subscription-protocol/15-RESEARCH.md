# Phase 15: Subscription Protocol - Research

**Researched:** 2026-04-03
**Domain:** P2P subscription announcement protocol for WAVS operator nodes (bridge loop integration)
**Confidence:** HIGH

## Summary

Phase 15 wires the Phase 14 data structures into the live P2P bridge loops. The scope is: (1) broadcast a `SubscriptionAnnouncement` when the local node subscribes/unsubscribes from a service, (2) process inbound subscription announcements from peers and update the `PeerSubscriptionMap`, (3) piggyback full subscription state on the existing heartbeat for self-healing eventual consistency, (4) send a full subscription hello when a new peer first sends a message, and (5) treat peers that have never sent any subscription announcement as subscribed-to-all (COMPAT-03 backward compatibility). No new crate dependencies. No API changes to `P2pHandle` or `P2pCommand`. No changes outside `p2p.rs`.

The critical integration points are three locations in each bridge loop (`run_lookup_network` and `run_discovery_network`): the `P2pCommand::Subscribe`/`Unsubscribe` handlers (gain announcement broadcasting), the inbound message handler (gains subscription announcement interception before `ServiceRouter` filtering), and the heartbeat tick handler (gains subscription state piggybacking). Both bridge loops must receive identical changes -- the code is duplicated between lookup and discovery modes.

**Primary recommendation:** Add a `PeerSubscriptionMap` instance and a `known_peers: HashSet<ed25519::PublicKey>` tracker as bridge loop state variables. Intercept subscription announcements in the inbound path (before service filtering). Broadcast announcements on subscribe/unsubscribe. Piggyback full subscription state on every heartbeat. Add `set_peer_subscriptions()` and `has_announced()` methods to `PeerSubscriptionMap`. Use replace-not-merge semantics for heartbeat-carried subscription sync.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ANN-01 | When an operator adds a service, a subscription announcement is broadcast to all connected peers | Modify `P2pCommand::Subscribe` handler in both bridge loops to build a `SubscriptionAnnouncement { subscribe: [service_id], unsubscribe: [] }` and send via `direct_sender.send(Recipients::All, ...)` |
| ANN-02 | When an operator removes a service, an unsubscribe announcement is broadcast to all connected peers | Modify `P2pCommand::Unsubscribe` handler in both bridge loops to build a `SubscriptionAnnouncement { subscribe: [], unsubscribe: [service_id] }` and send via `direct_sender.send(Recipients::All, ...)` |
| ANN-03 | Subscription state is piggybacked on periodic heartbeats for self-healing eventual consistency | Extend heartbeat tick handler to build a `SubscriptionAnnouncement { subscribe: service_router.subscribed_services_raw(), unsubscribe: [] }` and broadcast after the existing heartbeat probe. Receiver uses `set_peer_subscriptions()` (replace-not-merge) for heartbeat-carried announcements. |
| ANN-04 | When a new peer connects, the node sends its full subscription set as a hello message | Track peers in `known_peers: HashSet<ed25519::PublicKey>`. When an inbound message arrives from a peer not in `known_peers`, send a targeted hello via `direct_sender.send(Recipients::One(peer), ...)` with full subscription state |
| COMPAT-03 | Nodes without v1.3 targeting are treated as subscribed-to-all (backward compatible during rolling updates) | `PeerSubscriptionMap::has_announced(peer)` returns false for peers that have never sent a subscription announcement. `get_recipients()` already returns `Recipients::All` when no subscription data exists. Phase 16 will use `has_announced()` to decide whether to target or broadcast to a given peer. |
</phase_requirements>

## Standard Stack

### Core

No new dependencies. All code uses existing workspace crates already imported in p2p.rs.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::collections::HashMap` | stdlib | PeerSubscriptionMap indexes (already in use) | Bridge loop is single-threaded |
| `std::collections::HashSet` | stdlib | Peer sets, service sets, `known_peers` tracker | Same thread safety reasoning |
| `commonware-cryptography` | 2026.3.0 | `ed25519::PublicKey` for peer identity in maps and `Recipients::One` | Already imported and used throughout p2p.rs |
| `commonware-p2p` | 2026.3.0 | `Recipients::All` / `Recipients::One(peer)` for announcement sends | Already imported; `Recipients::One` verified at lib.rs line 45 |
| `commonware-codec` | 2026.3.0 | `Encode::encode()` for `P2pMessage` -> bytes before `direct_sender.send()` | Already used in existing publish and heartbeat paths |
| `serde_json` | workspace | Serialize `SubscriptionAnnouncement` (Phase 14 established pattern) | Already used for announcement to/from P2pMessage encoding |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `Recipients::All` for announcements | `Recipients::Some(connected)` with known peer list | Announcements must reach ALL peers including ones we do not yet know about; `Recipients::All` is correct here |
| Per-heartbeat full subscription broadcast | Delta encoding (only changes) | Full broadcast is simpler, idempotent, and self-healing; delta requires tracking what each peer has seen. Defer to SCALE-03 (v1.4+) |
| `known_peers: HashSet` for hello tracking | Derive "new peer" from `PeerSubscriptionMap::has_announced` | `has_announced` tracks who sent US an announcement; `known_peers` tracks who WE have seen any message from. Different semantics. A peer can be "known" (we have seen their heartbeat) but not "announced" (they have not sent subscription data). |

## Architecture Patterns

### Recommended File Structure

All changes in one existing file:
```
packages/wavs/src/subsystems/aggregator/p2p.rs
```

No new files. No changes outside this file.

### Pattern 1: Three Integration Points per Bridge Loop

**What:** Each bridge loop (`run_lookup_network`, `run_discovery_network`) requires changes at exactly three integration points:

1. **Subscribe/Unsubscribe command handlers** (lines ~839-846 lookup, ~1215-1222 discovery) -- Add announcement broadcast after local ServiceRouter update
2. **Inbound message handler** (lines ~878-936 lookup, ~1254-1310 discovery) -- Intercept subscription announcements before ServiceRouter filtering; send hello on first contact
3. **Heartbeat tick handler** (lines ~943-975 lookup, ~1317-1349 discovery) -- Piggyback full subscription state after existing heartbeat probe

**When to use:** Every bridge loop modification in this phase.

**Why:** The bridge loops are nearly identical between lookup and discovery modes. The same change must be applied to both. Keeping integration points consistent prevents divergence.

### Pattern 2: Announcement Broadcast Helper Function

**What:** Extract a `broadcast_subscription_announcement()` helper to avoid duplicating the encode-and-send logic across multiple call sites (subscribe, unsubscribe, heartbeat, hello).

**When to use:** Whenever sending a SubscriptionAnnouncement.

**Example:**
```rust
/// Broadcast a subscription announcement to the given recipients on the direct channel.
/// Returns Ok(()) on success, logs and ignores send failures (announcements are best-effort).
async fn broadcast_subscription_announcement(
    direct_sender: &impl commonware_p2p::Sender<PublicKey = ed25519::PublicKey>,
    announcement: &SubscriptionAnnouncement,
    recipients: Recipients<ed25519::PublicKey>,
) {
    match announcement.to_p2p_message() {
        Ok(msg) => {
            let encoded = Encode::encode(&msg);
            if let Err(e) = direct_sender.send(recipients, encoded, false).await {
                tracing::debug!("Subscription announcement send failed: {:?}", e);
            }
        }
        Err(e) => {
            tracing::error!("Failed to encode subscription announcement: {:?}", e);
        }
    }
}
```

**Note on generic Sender trait:** The `direct_sender` in the bridge loops has a concrete type derived from `network.register()`. The helper function should either take a concrete reference or use `impl Sender`. Given that the bridge loop code is not generic today (each loop has its own concrete types), the simplest approach is to inline the helper as a closure or extract it as a standalone async function that takes `&impl Sender<PublicKey = ed25519::PublicKey>`. Alternatively, since both bridge loops have the exact same code pattern, the helper can be a plain async function.

### Pattern 3: Inbound Announcement Interception (Before ServiceRouter)

**What:** Check `is_subscription_announcement(&p2p_msg)` AFTER deduplication but BEFORE `service_router.should_accept()`. If it is a subscription announcement, process it and `continue` -- never forward to the Aggregator.

**When to use:** In the inbound message handler of both bridge loops.

**Why the ordering matters:**
- **After dedup:** Subscription announcements on heartbeat are sent every 2 seconds. The seen_digests set deduplicates them within a heartbeat cycle. But heartbeat announcements across different cycles have different digests (different payloads if services change, and different P2pMessage encodings). This is fine -- idempotent processing.
- **Before ServiceRouter:** ServiceRouter.should_accept() would reject the announcement because SUBSCRIPTION_SENTINEL is not in subscribed_services. If we check after should_accept, the announcement is silently dropped.

**Wait -- important subtlety about deduplication and heartbeat announcements:** The existing `seen_digests` set will NOT deduplicate identical heartbeat-carried announcements because each heartbeat creates a new `P2pMessage` object. However, the `Digestible` implementation for `P2pMessage` computes SHA-256 of `(service_id_bytes || payload)`. If the subscription set does not change between heartbeats, the announcement payload is identical, so the digest IS the same. This means `seen_digests` WILL deduplicate repeated heartbeat announcements with unchanged subscription sets. This is actually the desired behavior for regular heartbeats. For the replace-not-merge logic, we need the announcement to be processed even if deduplicated... but since replace-not-merge with the same data is a no-op, deduplication is fine.

**Example integration point:**
```rust
msg = inbound_rx.recv() => {
    match msg {
        Some((peer_pubkey, raw_bytes)) => {
            // Track inbound peer as connected (existing OBS-01 logic)
            // ...

            // Decode P2pMessage (existing logic)
            let p2p_msg = match P2pMessage::decode_cfg(...) { ... };

            // BCAST-02: Deduplication (existing logic)
            let digest = p2p_msg.digest();
            if seen_digests.contains(&digest) { continue; }
            // ...
            seen_digests.insert(digest);

            // NEW: Hello on first contact (ANN-04)
            if !known_peers.contains(&peer_pubkey) {
                known_peers.insert(peer_pubkey.clone());
                // Send our full subscription set to this new peer
                let my_services = service_router.subscribed_services_raw();
                if !my_services.is_empty() {
                    let hello = SubscriptionAnnouncement {
                        subscribe: my_services,
                        unsubscribe: vec![],
                    };
                    broadcast_subscription_announcement(
                        &direct_sender, &hello,
                        Recipients::One(peer_pubkey.clone()),
                    ).await;
                }
            }

            // NEW: Intercept subscription announcements (ANN-01..04 processing)
            if is_subscription_announcement(&p2p_msg) {
                match SubscriptionAnnouncement::from_payload(&p2p_msg.payload) {
                    Ok(announcement) => {
                        peer_subscriptions.handle_announcement(&peer_pubkey, &announcement);
                        tracing::debug!(
                            "Processed subscription announcement from {}: +{} -{}",
                            const_hex::encode(peer_pubkey.as_ref()),
                            announcement.subscribe.len(),
                            announcement.unsubscribe.len(),
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Invalid subscription announcement: {:?}", e);
                    }
                }
                continue; // Do not forward to Aggregator
            }

            // Existing: Service filtering (BCAST-05)
            if !service_router.should_accept(&p2p_msg) { continue; }

            // Existing: Deserialize and forward to Aggregator
            // ...
        }
    }
}
```

### Pattern 4: Heartbeat Subscription Piggybacking

**What:** After the existing heartbeat probe, broadcast a SubscriptionAnnouncement with the full subscription set. The heartbeat probe (HEARTBEAT_SERVICE_ID) continues unchanged for peer discovery and retry queue flushing. The subscription announcement (SUBSCRIPTION_SENTINEL) is a separate message sent immediately after.

**Why separate messages (not merged):** The heartbeat probe serves a specific purpose -- it populates `connected_peers_tracker` from the acknowledgment result. Merging subscription data into the heartbeat payload would require changing the HEARTBEAT_SERVICE_ID handling. Keeping them separate respects the existing heartbeat contract and is simpler to implement.

**Replace-not-merge on receiver side:** When processing a heartbeat-carried announcement, the receiver should call `set_peer_subscriptions(peer, services)` which REPLACES the entire set rather than merging. This handles the case where a peer removed a service between heartbeats -- the replacement removes stale entries. For event-driven announcements (subscribe/unsubscribe commands), the incremental `handle_announcement()` method is appropriate.

**How to distinguish heartbeat-carried vs event-driven announcements:** The simplest approach is to NOT distinguish them on the wire. Instead, ALWAYS use replace-not-merge semantics for subscription announcements. Since `handle_announcement` currently does incremental add/remove, we need a new method `set_peer_subscriptions(peer, full_set)` that replaces. For heartbeat announcements, the `subscribe` list is the FULL set and `unsubscribe` is empty -- so we can detect "this is a full state dump" by checking that `unsubscribe.is_empty() && !subscribe.is_empty()`. However, this heuristic is fragile. A better approach: always call `set_peer_subscriptions` for ALL announcements. This is safe because event-driven announcements (subscribe one service) will set the peer's full set to just that one service if the peer hasn't been seen before, and the next heartbeat will correct it to the full set within 2 seconds.

**ACTUALLY -- the correct approach from the STATE.md decision "Replace-not-merge on heartbeat subscription sync":** Use `handle_announcement()` for event-driven subscribe/unsubscribe (incremental). Use `set_peer_subscriptions()` for heartbeat-carried full state (replace). The distinction is: heartbeat announcements always have `unsubscribe: []` and `subscribe: [full_set]`. Event-driven announcements have either `subscribe: [one_service]` or `unsubscribe: [one_service]`. The receiver can detect "full state" by an additional field or by convention. The simplest convention: add a `full_state: bool` field to `SubscriptionAnnouncement`. When true, the receiver calls `set_peer_subscriptions`. When false, the receiver calls `handle_announcement`.

**Recommended:** Add a `full_state: bool` field to `SubscriptionAnnouncement`:
```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct SubscriptionAnnouncement {
    pub subscribe: Vec<[u8; 32]>,
    pub unsubscribe: Vec<[u8; 32]>,
    #[serde(default)]
    pub full_state: bool,
}
```

The `#[serde(default)]` on `full_state` ensures backward compatibility -- old announcements without this field deserialize with `full_state: false`, which means incremental processing. This is correct for old-format announcements.

### Pattern 5: Backward Compatibility via has_announced (COMPAT-03)

**What:** Track which peers have sent at least one subscription announcement. Peers that have never announced are assumed to subscribe to ALL services (pre-v1.3 nodes or nodes that haven't been up long enough to send a heartbeat).

**When to use:** Phase 16 will use `has_announced()` when building recipient lists. If a peer has announced, use their subscription set. If not, include them in all sends.

**Example:**
```rust
impl PeerSubscriptionMap {
    /// Returns true if this peer has ever sent a subscription announcement.
    /// Peers that have not announced are treated as subscribed-to-all (COMPAT-03).
    pub fn has_announced(&self, peer: &ed25519::PublicKey) -> bool {
        self.peer_to_services.contains_key(peer)
    }

    /// Replace all subscriptions for a peer with the given set (heartbeat full sync).
    /// Uses replace-not-merge semantics per v1.3 design decision.
    pub fn set_peer_subscriptions(
        &mut self,
        peer: &ed25519::PublicKey,
        services: Vec<[u8; 32]>,
    ) {
        // First remove all existing subscriptions for this peer
        self.remove_peer(peer);
        // Then add the new full set
        if !services.is_empty() {
            let service_set: HashSet<[u8; 32]> = services.iter().copied().collect();
            for service_id in &service_set {
                self.service_to_peers
                    .entry(*service_id)
                    .or_default()
                    .insert(peer.clone());
            }
            self.peer_to_services.insert(peer.clone(), service_set);
        }
    }
}
```

### Anti-Patterns to Avoid

- **Sending announcements on Channel 0 (Engine):** Subscription announcements are control messages, not data. The Engine caches messages for catch-up replay. Replaying stale subscription announcements on reconnect would restore outdated subscription state. Only send on Channel 1 (direct) via `direct_sender.send()`.
- **Merging heartbeat subscription data instead of replacing:** A peer that removes a service between heartbeats would never have that service removed from remote maps. Replace-not-merge ensures convergence.
- **Adding new P2pCommand variants:** The existing Subscribe/Unsubscribe commands are sufficient. Announcement broadcasting is an internal bridge loop concern, not an API concern. Adding `P2pCommand::AnnounceSubscriptions` would leak implementation details to the Aggregator.
- **Processing subscription announcements after ServiceRouter filtering:** ServiceRouter would reject SUBSCRIPTION_SENTINEL (it is not a subscribed service). Interception must happen BEFORE should_accept().
- **Using `mailbox.broadcast()` for announcements:** The mailbox sends on Channel 0 (Engine). Announcements must NOT go through the Engine. Use `direct_sender.send()` only.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| P2pMessage encoding for direct send | Custom byte serialization | `Encode::encode(&msg)` (existing codec) | Already used in 6+ call sites in the bridge loops |
| Announcement wire format | New message type or envelope | `SubscriptionAnnouncement::to_p2p_message()` (Phase 14) | Already built and tested |
| Peer identity | String-based peer IDs | `ed25519::PublicKey` directly | Already the canonical identity type throughout p2p.rs |
| Recipients construction | Manual Vec building | `Recipients::All` / `Recipients::One(peer)` | Enum from commonware-p2p |

**Key insight:** Phase 15 is pure bridge loop integration. Every building block (PeerSubscriptionMap, SubscriptionAnnouncement, SUBSCRIPTION_SENTINEL, is_subscription_announcement, ServiceRouter::subscribed_services_raw) was built in Phase 14. This phase wires them together with ~60 lines of new logic per bridge loop.

## Common Pitfalls

### Pitfall 1: Announcement Sent ONLY on Direct Channel, Not Engine

**What goes wrong:** Using `mailbox.broadcast()` for subscription announcements causes the Engine to cache them. On peer reconnect, the Engine replays stale subscription state, potentially restoring subscriptions that were already removed.
**Why it happens:** The existing publish path sends on both channels. It is natural to follow the same pattern for announcements.
**How to avoid:** Subscription announcements use `direct_sender.send()` ONLY. Never `mailbox.broadcast()`. The heartbeat re-broadcasts subscription state every 2 seconds, which is the consistency repair mechanism -- not Engine catch-up.
**Warning signs:** If announcements appear in `seen_digests` from Engine catch-up (channel 0) rather than direct delivery (channel 1).

### Pitfall 2: Heartbeat Subscription Digest Deduplication Blocks Updates

**What goes wrong:** If the subscription set does not change between heartbeats, the `SubscriptionAnnouncement` encoded as P2pMessage has the same digest. The `seen_digests` set deduplicates the second heartbeat announcement. A peer that missed the first announcement never receives the second.
**Why it happens:** Subscription announcements are P2pMessages. P2pMessage's `Digestible` implementation hashes `(service_id_bytes || payload)`. Identical subscription sets produce identical digests.
**How to avoid:** This is actually acceptable behavior. If a peer missed the announcement, it was offline. When it reconnects, it will be a "new peer" (not in `known_peers`) and receive a hello message. The heartbeat deduplication only affects peers that already received the previous identical announcement -- which is a no-op anyway. Additionally, `seen_digests` is cleared when it reaches 1024 entries, so even if a peer was connected but somehow missed the first send, the digest will eventually be cleared and the next heartbeat will get through.
**Warning signs:** None expected. This is a non-issue in practice.

### Pitfall 3: Bridge Loop Duplication Divergence

**What goes wrong:** The same changes must be applied to both `run_lookup_network` and `run_discovery_network`. If one is updated but not the other, or if they diverge slightly, behavior differs between local and remote P2P modes.
**Why it happens:** These functions are ~400 lines each with near-identical bridge loop structures but different network setup code. They were not refactored into a shared bridge loop in v1.0-v1.2.
**How to avoid:** Implement changes in `run_lookup_network` first, then copy the bridge loop changes exactly to `run_discovery_network`. Verify with `cargo test -p wavs` which exercises both modes. The helper function `broadcast_subscription_announcement()` is shared between both loops, reducing divergence risk.
**Warning signs:** Tests pass for one P2P mode but fail for the other.

### Pitfall 4: Hello Message Creates Infinite Ping-Pong

**What goes wrong:** Peer A sends a hello to Peer B (first contact). Peer B receives the hello, which is a P2pMessage that triggers the inbound handler. If B's `known_peers` does not already contain A (because the hello was the first message from A), B sends a hello back to A. A's inbound handler receives B's hello, but A already has B in known_peers, so no reply. No infinite loop -- this self-terminates after one round trip.
**Why it happens:** N/A -- this is not actually a problem. The hello mechanism is a one-shot per peer pair.
**How to avoid:** Verify that the `known_peers.insert()` happens BEFORE checking `is_subscription_announcement()`. This ensures that a hello from a new peer marks them as known before processing their announcement, so the response hello does not trigger another response.
**Warning signs:** Excessive "Processed subscription announcement" log entries from the same peer pair.

### Pitfall 5: Empty Subscription Set on Heartbeat

**What goes wrong:** If the node has no services subscribed (e.g., during startup before any service is added), the heartbeat broadcasts an empty subscription announcement. Peers receive `SubscriptionAnnouncement { subscribe: [], unsubscribe: [], full_state: true }`. If processed with `set_peer_subscriptions()`, this REMOVES any previously known subscriptions for this peer -- effectively unsubscribing them from everything.
**Why it happens:** Between node startup and service registration, `service_router.subscribed_services_raw()` returns an empty vec.
**How to avoid:** Only broadcast subscription announcements on heartbeat when the local subscription set is non-empty. Add `if !my_services.is_empty()` guard. Alternatively, interpret an empty `full_state: true` announcement as "peer has no services" which is correct -- do not skip the broadcast, as this is meaningful information.
**Warning signs:** Peers temporarily losing subscription knowledge about a restarting node.

## Code Examples

### New Bridge Loop State Variables

```rust
// Source: Derived from existing bridge loop pattern at p2p.rs lines 762-769
// NEW: Peer subscription tracking (Phase 15)
let mut peer_subscriptions = PeerSubscriptionMap::new();
// NEW: Track peers we have seen (for hello on first contact, ANN-04)
let mut known_peers: HashSet<ed25519::PublicKey> = HashSet::new();
```

### Modified Subscribe Handler (ANN-01)

```rust
// Source: Modification of p2p.rs lines 839-842 (lookup) / 1215-1218 (discovery)
Some(P2pCommand::Subscribe { service_id }) => {
    service_router.subscribe(&service_id);
    tracing::info!("Subscribed to service: {}", service_id);
    // ANN-01: Announce subscription to all connected peers
    let announcement = SubscriptionAnnouncement {
        subscribe: vec![service_id.inner()],
        unsubscribe: vec![],
        full_state: false,
    };
    if let Ok(msg) = announcement.to_p2p_message() {
        let encoded = Encode::encode(&msg);
        if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
            tracing::debug!("Subscription announcement send failed: {:?}", e);
        }
    }
}
```

### Modified Unsubscribe Handler (ANN-02)

```rust
// Source: Modification of p2p.rs lines 843-846 (lookup) / 1219-1222 (discovery)
Some(P2pCommand::Unsubscribe { service_id }) => {
    service_router.unsubscribe(&service_id);
    tracing::info!("Unsubscribed from service: {}", service_id);
    // ANN-02: Announce unsubscription to all connected peers
    let announcement = SubscriptionAnnouncement {
        subscribe: vec![],
        unsubscribe: vec![service_id.inner()],
        full_state: false,
    };
    if let Ok(msg) = announcement.to_p2p_message() {
        let encoded = Encode::encode(&msg);
        if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
            tracing::debug!("Unsubscription announcement send failed: {:?}", e);
        }
    }
}
```

### Heartbeat Subscription Piggybacking (ANN-03)

```rust
// Source: Extension of p2p.rs lines 943-975 (lookup) / 1317-1349 (discovery)
_ = heartbeat.tick() => {
    // Existing heartbeat probe (unchanged)
    let probe = P2pMessage {
        service_id_bytes: HEARTBEAT_SERVICE_ID,
        payload: vec![],
    };
    let ack_rx = mailbox.broadcast(Recipients::All, probe.clone()).await;
    let encoded = Encode::encode(&probe);
    if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
        tracing::trace!("Heartbeat direct send failed: {:?}", e);
    }
    // ... existing ack_rx handling (connected_peers_tracker, retry queue) ...

    // ANN-03: Piggyback full subscription state for self-healing consistency
    let my_services = service_router.subscribed_services_raw();
    if !my_services.is_empty() {
        let announcement = SubscriptionAnnouncement {
            subscribe: my_services,
            unsubscribe: vec![],
            full_state: true,
        };
        if let Ok(msg) = announcement.to_p2p_message() {
            let encoded = Encode::encode(&msg);
            if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
                tracing::trace!("Heartbeat subscription announcement failed: {:?}", e);
            }
        }
    }
}
```

### Inbound Handler with Hello and Announcement Interception (ANN-04, processing)

```rust
// Source: Modification of p2p.rs lines 878-936 (lookup) / 1254-1310 (discovery)
Some((peer_pubkey, raw_bytes)) => {
    // Track inbound peer as connected (existing OBS-01 logic, unchanged)
    {
        let sender_hex = const_hex::encode(peer_pubkey.as_ref());
        let mut peers = connected_peers_tracker.write().unwrap();
        if !peers.contains(&sender_hex) {
            peers.push(sender_hex);
        }
    }

    // Decode P2pMessage (existing logic, unchanged)
    let p2p_msg: P2pMessage = match P2pMessage::decode_cfg(
        raw_bytes, &(RangeCfg::new(0..=(max_message_size as usize)), ())
    ) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("Failed to decode P2P message: {:?}", e);
            continue;
        }
    };

    // BCAST-02: Deduplication (existing logic, unchanged)
    let digest = p2p_msg.digest();
    if seen_digests.contains(&digest) {
        tracing::trace!("Duplicate message filtered by digest");
        continue;
    }
    if seen_digests.len() >= MAX_SEEN_DIGESTS {
        seen_digests.clear();
    }
    seen_digests.insert(digest);

    // ANN-04: Send hello on first contact with a new peer
    if !known_peers.contains(&peer_pubkey) {
        known_peers.insert(peer_pubkey.clone());
        let my_services = service_router.subscribed_services_raw();
        if !my_services.is_empty() {
            let hello = SubscriptionAnnouncement {
                subscribe: my_services,
                unsubscribe: vec![],
                full_state: true,
            };
            if let Ok(msg) = hello.to_p2p_message() {
                let encoded = Encode::encode(&msg);
                if let Err(e) = direct_sender.send(
                    Recipients::One(peer_pubkey.clone()),
                    encoded,
                    false,
                ).await {
                    tracing::debug!("Hello announcement to new peer failed: {:?}", e);
                }
            }
        }
    }

    // NEW: Intercept subscription announcements
    if is_subscription_announcement(&p2p_msg) {
        match SubscriptionAnnouncement::from_payload(&p2p_msg.payload) {
            Ok(announcement) => {
                if announcement.full_state {
                    // Replace-not-merge for full state updates (heartbeat/hello)
                    peer_subscriptions.set_peer_subscriptions(
                        &peer_pubkey,
                        announcement.subscribe.clone(),
                    );
                } else {
                    // Incremental for event-driven announcements
                    peer_subscriptions.handle_announcement(&peer_pubkey, &announcement);
                }
                tracing::debug!(
                    "Subscription update from {}: +{} -{}{}",
                    const_hex::encode(peer_pubkey.as_ref()),
                    announcement.subscribe.len(),
                    announcement.unsubscribe.len(),
                    if announcement.full_state { " (full)" } else { "" },
                );
            }
            Err(e) => {
                tracing::warn!("Invalid subscription announcement: {:?}", e);
            }
        }
        continue; // Do not forward to Aggregator
    }

    // Existing: Service filtering (BCAST-05, unchanged)
    if !service_router.should_accept(&p2p_msg) {
        tracing::trace!("Filtered message for unsubscribed service");
        continue;
    }

    // Existing: Deserialize and forward to Aggregator (unchanged)
    // ...
}
```

### New PeerSubscriptionMap Methods

```rust
// Source: Extension of Phase 14 PeerSubscriptionMap at p2p.rs lines 359-422
impl PeerSubscriptionMap {
    /// Returns true if this peer has ever sent a subscription announcement (COMPAT-03).
    /// Peers that have never announced are treated as subscribed-to-all by callers.
    pub fn has_announced(&self, peer: &ed25519::PublicKey) -> bool {
        self.peer_to_services.contains_key(peer)
    }

    /// Replace all subscriptions for a peer with the given set.
    /// Uses replace-not-merge semantics for heartbeat/hello full state sync.
    pub fn set_peer_subscriptions(
        &mut self,
        peer: &ed25519::PublicKey,
        services: Vec<[u8; 32]>,
    ) {
        // Remove existing subscriptions first
        self.remove_peer(peer);
        // Then set the new full set (if non-empty)
        if !services.is_empty() {
            let service_set: HashSet<[u8; 32]> = services.iter().copied().collect();
            for service_id in &service_set {
                self.service_to_peers
                    .entry(*service_id)
                    .or_default()
                    .insert(peer.clone());
            }
            self.peer_to_services.insert(peer.clone(), service_set);
        }
    }
}
```

### SubscriptionAnnouncement with full_state Field

```rust
// Source: Extension of Phase 14 SubscriptionAnnouncement at p2p.rs lines 324-346
/// Subscription announcement carried as P2pMessage payload (ANN-05).
/// The service_id_bytes field of the wrapping P2pMessage is set to SUBSCRIPTION_SENTINEL.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct SubscriptionAnnouncement {
    /// Services this peer is subscribing to
    pub subscribe: Vec<[u8; 32]>,
    /// Services this peer is unsubscribing from
    pub unsubscribe: Vec<[u8; 32]>,
    /// If true, `subscribe` is the FULL set of services (replace-not-merge).
    /// If false, `subscribe`/`unsubscribe` are incremental changes.
    /// Defaults to false for backward compatibility with Phase 14 announcements.
    #[serde(default)]
    pub full_state: bool,
}
```

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (cargo test) |
| Config file | `Cargo.toml` workspace |
| Quick run command | `cargo test -p wavs --lib -- p2p_broadcast_tests` |
| Full suite command | `cargo test -p wavs` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ANN-01 | Subscribe command triggers announcement broadcast (verify announcement P2pMessage is constructed correctly) | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_subscribe_builds_announcement` | Wave 0 |
| ANN-02 | Unsubscribe command triggers unsubscribe announcement | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_unsubscribe_builds_announcement` | Wave 0 |
| ANN-03 | Heartbeat carries full subscription state (full_state=true, subscribe=all services) | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_heartbeat_subscription_announcement` | Wave 0 |
| ANN-04 | First inbound message from new peer triggers hello with full subscription set | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_hello_on_first_contact` | Wave 0 |
| ANN-03 | set_peer_subscriptions replaces (not merges) existing subscriptions | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_set_peer_subscriptions_replaces` | Wave 0 |
| COMPAT-03 | has_announced returns false for unknown peers, true after announcement | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_has_announced_compat03` | Wave 0 |
| ANN-03 | full_state field defaults to false for backward compat (serde default) | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_full_state_serde_default` | Wave 0 |
| ANN-01 | Announcement with full_state=false uses incremental handle_announcement | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_incremental_vs_full_state_processing` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p wavs --lib -- p2p_broadcast_tests`
- **Per wave merge:** `cargo test -p wavs`
- **Phase gate:** Full `cargo test -p wavs` green before verification

### Wave 0 Gaps
- All new tests extend the existing `#[cfg(test)] mod p2p_broadcast_tests` module in p2p.rs
- No new test files or framework installs needed
- Existing 22 tests in `p2p_broadcast_tests` must continue passing (regression check)
- Note: Bridge loop integration (the actual send/receive flow) cannot be unit tested in isolation without mocking the commonware P2P network. The unit tests verify data structure operations and announcement construction. Full protocol integration testing is covered by the existing E2E tests (`cargo test -p layer-tests`).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Subscriptions are local-only (ServiceRouter) | Remote peer subscriptions tracked via PeerSubscriptionMap | Phase 14 (data structures) + Phase 15 (protocol) | Enables targeted delivery in Phase 16 |
| No subscription announcements | Sentinel-based announcements on direct channel | Phase 14 (wire format) + Phase 15 (broadcast/receive) | Peers learn each other's subscriptions |
| Heartbeat is connectivity probe only | Heartbeat carries subscription state for consistency | Phase 15 | Self-healing subscription convergence |
| All peers assumed to handle all services | Peers explicitly declare their service set | Phase 15 + COMPAT-03 fallback | Unknown peers still treated as "all" |

## Open Questions

1. **Should `full_state: true` with empty `subscribe` list remove all peer subscriptions?**
   - What we know: A node with zero services would broadcast `{ subscribe: [], unsubscribe: [], full_state: true }`. Processing this with `set_peer_subscriptions(peer, [])` calls `remove_peer(peer)` and then adds nothing. The peer effectively becomes "has not announced" which reverts to `Recipients::All` fallback.
   - What's unclear: Whether this is the desired behavior. A node that explicitly says "I have no services" is different from a node that has never announced.
   - Recommendation: Guard heartbeat subscription broadcasts with `if !my_services.is_empty()`. A node with no services does not need to announce. If it later adds a service, the event-driven ANN-01 announcement covers it.

2. **Should the hello message (ANN-04) be sent on first inbound message or on peer connection?**
   - What we know: The bridge loop does not receive a "peer connected" event from commonware-p2p. The only signal of a new peer is receiving a message from them (either heartbeat, announcement, or submission). Using first inbound message as proxy for "connected" is the only option without modifying commonware internals.
   - What's unclear: Whether a peer's first message could be a submission that arrives before the hello response reaches them, causing a brief window where the new peer is not in our subscription map.
   - Recommendation: Use first inbound message (the only available signal). The 2-second heartbeat ensures convergence within one cycle even if the hello is missed. The hello is an optimization, not a correctness requirement.

## Project Constraints (from CLAUDE.md)

- **Build system:** `justfile`-based -- all test commands via `cargo test -p wavs`
- **Lint:** `cargo clippy --all-targets --all-features` with `-D warnings` (deny all warnings)
- **Format:** `cargo fmt` enforced
- **Test placement:** Inline `#[cfg(test)] mod tests { ... }` modules within Rust source files
- **Naming:** `snake_case` for functions, variables, module names; `PascalCase` for types and structs; `SCREAMING_SNAKE_CASE` for constants
- **Error handling:** Use `Result<T, E>` for fallible operations
- **Comments:** Three-slash doc comments (`///`) for public items; explain the "why" not the "what"
- **Logging:** `tracing` macros (`info!`, `warn!`, `debug!`, `trace!`); structured fields
- **Module visibility:** `pub(crate)` for items used within the crate but not exported; `pub` only for true public API
- **GSD Workflow:** Follow GSD workflow for all changes

## Sources

### Primary (HIGH confidence)
- `packages/wavs/src/subsystems/aggregator/p2p.rs` (2115 lines) -- complete source analysis: both bridge loops (`run_lookup_network` lines 598-978, `run_discovery_network` lines 996-1352), Phase 14 data structures (PeerSubscriptionMap lines 348-422, SubscriptionAnnouncement lines 322-346), heartbeat mechanism (lines 776-781, 943-975), inbound handler (lines 878-936), P2pCommand enum (lines 1359-1376)
- `.planning/research/ARCHITECTURE.md` -- component diagrams, data flow sequences, subscription protocol design with code examples
- `.planning/research/PITFALLS.md` -- 11 pitfalls; Pitfalls 1, 3, 5, 6, 8, 9 directly relevant to Phase 15
- `.planning/phases/14-subscription-data-structures/14-01-SUMMARY.md` -- Phase 14 deliverables confirmed: PeerSubscriptionMap, SubscriptionAnnouncement, SUBSCRIPTION_SENTINEL, is_subscription_announcement, subscribed_services_raw, 11 tests
- `.planning/STATE.md` -- Accumulated decisions: HashMap/HashSet for PeerSubscriptionMap, serde_json for encoding, replace-not-merge on heartbeat sync

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` -- ANN-01 through ANN-04, COMPAT-03 requirement definitions
- `.planning/research/SUMMARY.md` -- phase ordering rationale and scope boundaries

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- zero new dependencies; all types and functions already exist in p2p.rs
- Architecture: HIGH -- integration points identified by exact line numbers; both bridge loops analyzed; patterns derived from existing heartbeat and publish code paths
- Pitfalls: HIGH -- 5 phase-specific pitfalls identified from code analysis + project-level research (PITFALLS.md cross-referenced)

**Research date:** 2026-04-03
**Valid until:** 2026-05-03 (stable -- pure bridge loop integration, no external API dependencies)
