# Architecture Patterns: Per-Service P2P Targeting

**Domain:** P2P subscription tracking and targeted message delivery in WAVS operator nodes
**Researched:** 2026-04-03
**Confidence:** HIGH (all findings from direct source code analysis of p2p.rs, aggregator.rs, dispatcher.rs, and commonware-p2p/commonware-broadcast library source)

## Existing Architecture (Current State)

### Component Map

```
                    ┌──────────────────────────────────────────────────────┐
                    │                   Aggregator                          │
                    │                                                      │
  Dispatcher ──────>│ AggregatorCommand::Broadcast(submission)             │
  (crossbeam)       │ AggregatorCommand::SubscribeService { service_id }   │
                    │ AggregatorCommand::UnsubscribeService { service_id }  │
                    │                                                      │
                    │  p2p_handle: Arc<RwLock<Option<P2pHandle>>>           │
                    │      |                                               │
                    │      v                                               │
                    │  P2pHandle { command_tx: UnboundedSender<P2pCommand> }│
                    └──────────────────────┬───────────────────────────────┘
                                           |
                    ┌──────────────────────v───────────────────────────────┐
                    │         Commonware Runtime Thread                     │
                    │         (std::thread::spawn, own Tokio runtime)       │
                    │                                                      │
                    │  State owned inside bridge loop:                      │
                    │  ┌────────────────┐  ┌────────────┐                  │
                    │  │ ServiceRouter  │  │ RetryQueue  │                  │
                    │  │ (HashSet<[u8;  │  │ (VecDeque)  │                  │
                    │  │   32]>)        │  └────────────┘                  │
                    │  └────────────────┘                                  │
                    │  ┌────────────────┐  ┌──────────────────┐            │
                    │  │ seen_digests   │  │ connected_peers  │            │
                    │  │ (HashSet)      │  │ (Arc<RwLock<     │            │
                    │  └────────────────┘  │  Vec<String>>>)  │            │
                    │                      └──────────────────┘            │
                    │                                                      │
                    │  Network (Channel 0 + Channel 1):                    │
                    │  ┌──────────────────┐  ┌─────────────────────┐       │
                    │  │ Engine (Ch 0)    │  │ direct_sender (Ch 1)│       │
                    │  │ + Mailbox        │  │ + direct_receiver   │       │
                    │  │ (caching+catchup)│  │ (real-time fwd)     │       │
                    │  └──────────────────┘  └─────────────────────┘       │
                    └──────────────────────────────────────────────────────┘
```

### Current Data Flow (Outbound)

1. Aggregator receives `AggregatorCommand::Broadcast(submission)`
2. Sends to self via `aggregator_to_self_tx` for local processing
3. Calls `p2p_handle.publish(submission)`
4. P2pHandle sends `P2pCommand::Publish { service_id, submission }` via unbounded mpsc
5. Bridge loop receives command, creates `P2pMessage { service_id_bytes, payload }`
6. **Channel 0**: `mailbox.broadcast(Recipients::All, msg)` -- Engine caches for catch-up
7. **Channel 1**: `direct_sender.send(Recipients::All, encoded_bytes, false)` -- real-time delivery
8. Both channels send to ALL connected peers regardless of service

### Current Data Flow (Inbound)

1. Channel 1 `direct_receiver.recv()` produces `(peer_pubkey, raw_bytes)`
2. Bridge task forwards to Tokio mpsc `inbound_tx`
3. Bridge loop receives from `inbound_rx`
4. Tracks peer in `connected_peers_tracker`
5. Decodes `P2pMessage` from raw bytes
6. **Deduplication**: SHA-256 digest checked against `seen_digests` HashSet
7. **Service filtering**: `service_router.should_accept(&p2p_msg)` -- drops if not subscribed
8. Deserializes `Submission`, sends `AggregatorCommand::Receive { submission, peer }` to Aggregator

### Current Subscription Flow

1. Dispatcher calls `aggregator_tx.send(AggregatorCommand::SubscribeService { service_id })` when service added
2. Aggregator forwards `p2p_handle.subscribe(&service_id)`
3. P2pHandle sends `P2pCommand::Subscribe { service_id }`
4. Bridge loop calls `service_router.subscribe(&service_id)` -- adds to local `HashSet<[u8; 32]>`
5. **No announcement to peers** -- subscriptions are purely local state

### Key Constraints From Existing Code

| Constraint | Source | Impact |
|-----------|--------|--------|
| Channels registered before `network.start()` | commonware-p2p API | Cannot add channels dynamically at runtime |
| Two channels (0 + 1) already registered | p2p.rs lines 569-580 | Must reuse existing channels or restructure |
| Bridge loop owns all mutable state | p2p.rs `run_lookup_network` / `run_discovery_network` | ServiceRouter, RetryQueue, seen_digests only accessible inside loop |
| `Recipients::All` hardcoded in 17 call sites | p2p.rs (grep results) | Every broadcast/heartbeat/retry sends to all peers |
| `Recipients::Some(Vec<P>)` requires `Vec<ed25519::PublicKey>` | commonware-p2p `Recipients<P: PublicKey>` | Must resolve service subscriptions to Ed25519 pubkeys |
| Mailbox::broadcast takes `Recipients<P>` | commonware-broadcast Broadcaster trait | Engine supports targeted broadcast natively |
| Sender::send takes `Recipients<P>` | commonware-p2p Sender trait | Direct channel supports targeted send natively |
| Commonware runtime on separate OS thread | p2p.rs `spawn_commonware_runtime` | All P2P state lives on that thread; cross-thread via mpsc only |
| `P2pHandle` is the only cross-thread API | p2p.rs struct P2pHandle | All commands go through `command_tx: UnboundedSender<P2pCommand>` |

## Recommended Architecture: Per-Service P2P Targeting

### Design Principle

**Extend, don't restructure.** The existing two-channel architecture, command protocol, and bridge loop pattern are sound. Per-service targeting adds a new layer of state (peer subscriptions) and changes where `Recipients::All` is used to `Recipients::Some(service_peers)`.

### New Component: PeerSubscriptionMap

```rust
/// Tracks which peers are subscribed to which services.
/// Lives inside the bridge loop (same thread as ServiceRouter).
pub(crate) struct PeerSubscriptionMap {
    /// service_id -> Set<ed25519::PublicKey>
    service_to_peers: HashMap<[u8; 32], HashSet<ed25519::PublicKey>>,
    /// peer -> Set<service_id> (reverse index for efficient removal on disconnect)
    peer_to_services: HashMap<ed25519::PublicKey, HashSet<[u8; 32]>>,
}
```

**Why a new struct rather than extending ServiceRouter:** ServiceRouter tracks what THIS node subscribes to (local filtering). PeerSubscriptionMap tracks what REMOTE peers subscribe to (outbound targeting). Different concerns, different data shapes.

### New Wire Message: SubscriptionAnnouncement

```rust
/// Control message for subscription announcements.
/// Encoded as a P2pMessage with a reserved sentinel service_id.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubscriptionAnnouncement {
    /// Services this peer is subscribing to (add)
    pub subscribe: Vec<[u8; 32]>,
    /// Services this peer is unsubscribing from (remove)
    pub unsubscribe: Vec<[u8; 32]>,
}
```

**Wire format:** Reuses `P2pMessage` with a reserved sentinel `service_id_bytes` (proposed: `[0xFF; 32]`). The existing `HEARTBEAT_SERVICE_ID` (`[0x00; 32]`) shows this sentinel pattern works. ServiceRouter will never accept this sentinel, so subscription announcements are never forwarded to the Aggregator.

### Modified Components

#### 1. P2pCommand (no new variants needed)

```rust
enum P2pCommand {
    Publish { service_id: ServiceId, submission: Box<Submission> },
    Subscribe { service_id: ServiceId },      // existing -- gains announcement side effect
    Unsubscribe { service_id: ServiceId },    // existing -- gains announcement side effect
    GetStatus { response_tx: ... },            // existing
    BlockPeer { pubkey_hex: String },          // existing
}
```

The existing `Subscribe` and `Unsubscribe` commands gain additional behavior (announcing to peers) without any API change.

#### 2. Bridge Loop (modify Publish handler)

**Before:**
```rust
let ack_rx = mailbox.broadcast(Recipients::All, msg.clone()).await;
let encoded_bytes = Encode::encode(&msg);
direct_sender.send(Recipients::All, encoded_bytes, false).await;
```

**After:**
```rust
let recipients = peer_subscriptions.get_recipients(&msg.service_id_bytes);
let ack_rx = mailbox.broadcast(recipients.clone(), msg.clone()).await;
let encoded_bytes = Encode::encode(&msg);
direct_sender.send(recipients, encoded_bytes, false).await;
```

Where `get_recipients` returns `Recipients::Some(peers)` if any peers are known, or `Recipients::All` as fallback when no subscription data exists (backward compatibility during rolling upgrades).

#### 3. Bridge Loop (modify Subscribe/Unsubscribe handlers)

**Before:**
```rust
Some(P2pCommand::Subscribe { service_id }) => {
    service_router.subscribe(&service_id);
}
```

**After:**
```rust
Some(P2pCommand::Subscribe { service_id }) => {
    service_router.subscribe(&service_id);
    // Announce subscription to all connected peers
    let announcement = SubscriptionAnnouncement {
        subscribe: vec![service_id.inner()],
        unsubscribe: vec![],
    };
    broadcast_announcement(&mut direct_sender, &announcement).await;
}
```

#### 4. Bridge Loop (modify inbound message handler)

Add a check before ServiceRouter filtering:

```rust
// Check if this is a subscription announcement
if p2p_msg.service_id_bytes == SUBSCRIPTION_SENTINEL {
    match serde_json::from_slice::<SubscriptionAnnouncement>(&p2p_msg.payload) {
        Ok(announcement) => {
            peer_subscriptions.handle_announcement(&peer_pubkey, &announcement);
        }
        Err(e) => {
            tracing::warn!("Invalid subscription announcement from peer: {:?}", e);
        }
    }
    continue; // Don't forward to Aggregator
}
```

#### 5. Heartbeat (extend for subscription exchange)

On heartbeat, include the node's current service subscriptions so newly connected peers learn about existing subscriptions:

```rust
_ = heartbeat.tick() => {
    // Existing heartbeat probe
    let probe = P2pMessage { service_id_bytes: HEARTBEAT_SERVICE_ID, payload: vec![] };
    // ... existing heartbeat logic ...

    // Also broadcast full subscription state for new peers
    let my_services = service_router.subscribed_services_raw(); // Vec<[u8; 32]>
    if !my_services.is_empty() {
        let announcement = SubscriptionAnnouncement {
            subscribe: my_services,
            unsubscribe: vec![],
        };
        broadcast_announcement(&mut direct_sender, &announcement).await;
    }
}
```

This is idempotent -- receiving a subscription for an already-known service is a no-op in the PeerSubscriptionMap.

### Complete Data Flow: Targeted Send

```
1. Aggregator::handle_broadcast(submission)
   |
   v
2. P2pHandle::publish(submission)
   -> P2pCommand::Publish { service_id: "svc-123", submission }
   |
   v
3. Bridge loop receives Publish command
   -> P2pMessage::from_submission(service_id, submission)
   -> peer_subscriptions.get_recipients(service_id_bytes)
      |
      +-- Known peers for svc-123: [PeerA, PeerC]
      |   -> Recipients::Some(vec![PeerA, PeerC])
      |
      +-- No subscription data for svc-123 (fallback)
          -> Recipients::All
   |
   v
4. mailbox.broadcast(Recipients::Some([PeerA, PeerC]), msg)  -- Ch 0 (Engine)
   direct_sender.send(Recipients::Some([PeerA, PeerC]), bytes) -- Ch 1 (direct)
   |
   v
5. commonware-p2p delivers only to PeerA and PeerC
   (PeerB, PeerD never receive the message)
```

### Complete Data Flow: Subscription Protocol

```
1. Dispatcher adds service "svc-456" to this node
   -> AggregatorCommand::SubscribeService { service_id: "svc-456" }
   |
   v
2. Aggregator forwards to P2pHandle
   -> P2pCommand::Subscribe { service_id: "svc-456" }
   |
   v
3. Bridge loop handles Subscribe
   a) service_router.subscribe("svc-456")     -- local filtering
   b) SubscriptionAnnouncement { subscribe: ["svc-456"], unsubscribe: [] }
   c) broadcast_announcement(Recipients::All)  -- tell all peers
   |
   v
4. All connected peers receive announcement
   -> peer_subscriptions.handle_announcement(our_pubkey, announcement)
   -> Adds our_pubkey to service_to_peers["svc-456"]
   -> Future publishes for svc-456 from those peers now target us
```

### Complete Data Flow: Peer Reconnection Catch-Up

```
1. PeerB reconnects to network
   |
   v
2. PeerB sends heartbeat with full subscription state
   -> SubscriptionAnnouncement { subscribe: ["svc-123", "svc-789"], unsubscribe: [] }
   |
   v
3. Our bridge loop receives announcement
   -> peer_subscriptions.handle_announcement(PeerB_pubkey, announcement)
   -> PeerB now listed as subscriber for svc-123 and svc-789
   |
   v
4. Engine catch-up (Channel 0) replays cached messages to PeerB
   -> Engine internally replays from its per-peer deques
   -> ALL cached messages replayed (Engine has no service awareness)
   -> PeerB's ServiceRouter filters on their side
```

**Important:** The broadcast Engine (Channel 0) does NOT support per-service filtering on catch-up. It replays all cached messages to reconnecting peers. This is acceptable because:
- The Engine's deque is bounded (default 128 messages per peer)
- PeerB's own ServiceRouter filters inbound on their side
- Adding service-aware caching to the Engine would require forking commonware-broadcast

### Impact Assessment

| Component | Change Type | Scope |
|-----------|------------|-------|
| `p2p.rs` -- `PeerSubscriptionMap` | **NEW** struct | ~80 lines |
| `p2p.rs` -- `SubscriptionAnnouncement` | **NEW** type | ~15 lines |
| `p2p.rs` -- `SUBSCRIPTION_SENTINEL` | **NEW** constant | 1 line |
| `p2p.rs` -- `ServiceRouter` | **EXTEND** (add `subscribed_services_raw()`) | ~5 lines |
| `p2p.rs` -- `P2pCommand` enum | **NO CHANGE** | 0 lines |
| `p2p.rs` -- `P2pHandle` | **NO CHANGE** | 0 lines |
| `p2p.rs` -- `run_lookup_network` bridge loop | **MODIFY** (Publish, Subscribe, Unsubscribe, inbound, heartbeat) | ~60 lines changed |
| `p2p.rs` -- `run_discovery_network` bridge loop | **MODIFY** (same changes as lookup) | ~60 lines changed |
| `aggregator.rs` -- `AggregatorCommand` | **NO CHANGE** | 0 lines |
| `aggregator.rs` -- `Aggregator` | **NO CHANGE** | 0 lines |
| `dispatcher.rs` | **NO CHANGE** | 0 lines |
| `packages/types/src/http.rs` -- `P2pStatus` | **EXTEND** (add `peer_subscriptions` field) | ~5 lines |

**Total estimated delta:** ~220 lines added/changed in p2p.rs, ~5 lines in types.

### No Changes Required Outside P2P Module

The Aggregator, Dispatcher, and all other subsystems are unaffected because:
- `P2pHandle` API (`publish`, `subscribe`, `unsubscribe`) does not change
- `AggregatorCommand` variants do not change
- The targeting logic is entirely internal to the bridge loop
- Subscription announcements are a P2P-layer concern, invisible to the application

## Patterns to Follow

### Pattern 1: Sentinel-Based Message Discrimination

**What:** Use reserved service_id_bytes values to differentiate control messages from data messages on the same broadcast channel.

**When:** Need to add new message types without adding new P2P channels (which must be registered before `network.start()`).

**Why this pattern:** Commonware channels cannot be added dynamically. Sentinel values let us multiplex control and data messages on the same channel. Already proven with `HEARTBEAT_SERVICE_ID = [0u8; 32]`.

```rust
const HEARTBEAT_SERVICE_ID: [u8; 32] = [0u8; 32];       // existing
const SUBSCRIPTION_SENTINEL: [u8; 32] = [0xFF; 32];     // new

// In inbound message handler:
match p2p_msg.service_id_bytes {
    HEARTBEAT_SERVICE_ID => { /* ignore heartbeat */ }
    SUBSCRIPTION_SENTINEL => { /* handle subscription announcement */ }
    _ => { /* normal submission message -- ServiceRouter filter */ }
}
```

### Pattern 2: Fallback-to-Broadcast on Missing Data

**What:** When the PeerSubscriptionMap has no data for a service, fall back to `Recipients::All` rather than `Recipients::Some(empty_vec)`.

**When:** Rolling upgrades where old nodes don't send subscription announcements, or when a service is first deployed and no announcements have been received yet.

**Why this pattern:** Correctness over efficiency. Broadcasting to all peers and letting their ServiceRouter filter is the existing behavior and always correct. Targeted send is an optimization, not a correctness requirement.

```rust
impl PeerSubscriptionMap {
    fn get_recipients(&self, service_id: &[u8; 32]) -> Recipients<ed25519::PublicKey> {
        match self.service_to_peers.get(service_id) {
            Some(peers) if !peers.is_empty() => {
                Recipients::Some(peers.iter().cloned().collect())
            }
            _ => Recipients::All, // fallback: broadcast to everyone
        }
    }
}
```

### Pattern 3: Idempotent Subscription State

**What:** Subscription announcements are idempotent -- receiving the same subscription twice is a no-op.

**When:** Heartbeats periodically re-broadcast full subscription state to handle missed announcements.

**Why this pattern:** In a P2P network, messages can be lost, duplicated, or arrive out of order. Making announcements idempotent means we don't need exactly-once delivery guarantees.

```rust
impl PeerSubscriptionMap {
    fn handle_announcement(
        &mut self,
        peer: &ed25519::PublicKey,
        announcement: &SubscriptionAnnouncement,
    ) {
        for service_id in &announcement.subscribe {
            self.service_to_peers
                .entry(*service_id)
                .or_default()
                .insert(peer.clone());
            self.peer_to_services
                .entry(peer.clone())
                .or_default()
                .insert(*service_id);
        }
        for service_id in &announcement.unsubscribe {
            if let Some(peers) = self.service_to_peers.get_mut(service_id) {
                peers.remove(peer);
                if peers.is_empty() {
                    self.service_to_peers.remove(service_id);
                }
            }
            if let Some(services) = self.peer_to_services.get_mut(peer) {
                services.remove(service_id);
                if services.is_empty() {
                    self.peer_to_services.remove(peer);
                }
            }
        }
    }
}
```

### Pattern 4: Peer Disconnect Cleanup

**What:** When a peer disconnects (detected via heartbeat failure or explicit disconnect), remove all their subscriptions from PeerSubscriptionMap.

**When:** Connected peers tracker detects a peer is no longer in the set.

**Why this pattern:** Stale subscription entries would cause targeted sends to disconnected peers, wasting the `Recipients::Some` slot and potentially missing peers that have moved.

```rust
// In heartbeat handler, after updating connected_peers_tracker:
let current_peers: HashSet<_> = recipients.iter().cloned().collect();
peer_subscriptions.remove_disconnected_peers(&current_peers);
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Per-Service Channels

**What:** Registering a separate commonware-p2p channel for each service.

**Why bad:** Channels must be registered before `network.start()` and the set of services is dynamic (added/removed at runtime via HTTP API). This would require restarting the entire P2P network every time a service is added.

**Instead:** Use a single pair of channels with application-level routing (the existing pattern). Targeted `Recipients::Some` achieves the same network-level efficiency without channel-per-service overhead.

### Anti-Pattern 2: Forking commonware-broadcast for Service-Aware Caching

**What:** Modifying the broadcast Engine to understand service IDs and only cache/relay messages for subscribed services.

**Why bad:** Forks create maintenance burden. The Engine's per-peer deque is bounded (default 128), so the overhead of caching messages for unsubscribed services is bounded and small. The receiving node's ServiceRouter already filters these.

**Instead:** Accept that Engine catch-up replays all cached messages. The receiving node filters. The bounded deque prevents unbounded growth.

### Anti-Pattern 3: Subscription Announcements on Engine Channel (Channel 0)

**What:** Sending SubscriptionAnnouncements via the broadcast Engine's mailbox (Channel 0).

**Why bad:** The Engine caches ALL messages in its per-peer deque. Subscription announcements are ephemeral state -- caching them wastes deque slots and could replay stale subscription states to reconnecting peers.

**Instead:** Send subscription announcements on Channel 1 (direct) only. The heartbeat mechanism provides the catch-up path for subscription state -- no Engine caching needed.

### Anti-Pattern 4: Blocking on Subscription Convergence

**What:** Waiting until all peers have acknowledged subscription announcements before allowing publishes.

**Why bad:** P2P networks are asynchronous. Peers may be temporarily disconnected, slow, or running old software. Blocking would stall the entire submission pipeline.

**Instead:** Use the fallback-to-broadcast pattern. Publish immediately with best-available subscription data. If a peer is missed by `Recipients::Some`, they can still receive via Engine catch-up or the next publish when subscription data converges.

## Scalability Considerations

| Concern | At 5 services | At 50 services | At 500 services |
|---------|---------------|----------------|-----------------|
| PeerSubscriptionMap memory | Negligible | ~50KB per peer | ~500KB per peer (may need LRU) |
| Subscription announcement size | ~160 bytes | ~1.6KB | ~16KB (near single message limit) |
| Heartbeat bandwidth overhead | Negligible | ~1.6KB/2s | 16KB/2s (consider differential updates) |
| Recipients::Some Vec allocation | Trivial | Per-publish allocation | Consider caching Recipients per service |
| Duplicate code in lookup vs discovery | Manageable | Maintenance burden grows | Extract shared bridge loop logic |

### Key Scaling Decision: Full vs Differential Heartbeat

For small deployments (< 50 services per node), heartbeating the full subscription list is fine. For larger deployments, consider switching to:

1. **Full subscription on first heartbeat after connect** (or periodically, e.g., every 10th heartbeat)
2. **Differential updates only** on subsequent heartbeats (subscribe/unsubscribe events since last heartbeat)

This optimization is NOT needed for v1.3 but should be flagged for future phases.

### Key Refactoring Opportunity: Shared Bridge Loop

The `run_lookup_network` and `run_discovery_network` functions share ~90% identical bridge loop code. The per-service targeting changes must be applied to BOTH. Consider extracting the shared bridge loop into a generic function parameterized by the network type. This reduces the diff size and prevents the two implementations from diverging.

## Build Order (Dependency-Aware)

### Phase 1: Core Data Structures (no behavioral changes)
1. `PeerSubscriptionMap` struct with unit tests
2. `SubscriptionAnnouncement` type with serde roundtrip tests
3. `SUBSCRIPTION_SENTINEL` constant
4. `ServiceRouter::subscribed_services_raw()` method

**Rationale:** All new code, no existing behavior modified, fully testable in isolation.

### Phase 2: Subscription Protocol (announce/receive)
1. Modify inbound message handler to detect and dispatch `SUBSCRIPTION_SENTINEL` messages
2. Modify `Subscribe`/`Unsubscribe` command handlers to broadcast announcements
3. Modify heartbeat to include full subscription state
4. Add `PeerSubscriptionMap` as bridge loop state

**Rationale:** Adds the subscription protocol but does NOT yet change outbound targeting. Messages still go to `Recipients::All`. This is safe to deploy -- it is additive-only.

### Phase 3: Targeted Outbound (the actual optimization)
1. Modify Publish handler to use `peer_subscriptions.get_recipients()` instead of `Recipients::All`
2. Modify retry queue drain to use targeted recipients
3. Verify fallback-to-broadcast works when no subscription data exists

**Rationale:** This is the behavioral change. By doing it last, Phases 1-2 can be deployed and validated independently. If targeting causes issues, reverting to `Recipients::All` is a one-line change.

### Phase 4: Observability and Status
1. Extend `P2pStatus` with `peer_subscriptions: HashMap<String, Vec<String>>` (peer -> services)
2. Extend `GetStatus` handler to include subscription data
3. Update P2P docs

**Rationale:** Non-functional, but important for debugging production deployments.

## Sources

- `packages/wavs/src/subsystems/aggregator/p2p.rs` -- full source analysis (~1400 lines)
- `packages/wavs/src/subsystems/aggregator.rs` -- AggregatorCommand enum and dispatch logic
- `packages/wavs/src/dispatcher.rs` -- SubscribeService/UnsubscribeService integration points
- `commonware-p2p-2026.3.0/src/lib.rs` -- `Recipients<P>` enum (`All`, `Some(Vec<P>)`, `One(P)`)
- `commonware-broadcast-2026.3.0/src/buffered/engine.rs` -- Engine cache architecture (per-peer deques, no service awareness)
- `commonware-broadcast-2026.3.0/src/buffered/ingress.rs` -- Mailbox::broadcast takes `Recipients<P>`
- `commonware-p2p-2026.3.0/src/lib.rs` -- Sender::send takes `Recipients<P>` with rate limiting
- `packages/types/src/http.rs` -- P2pStatus struct
- `docs/P2P.md` -- Existing P2P documentation
