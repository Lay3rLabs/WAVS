# Feature Landscape

**Domain:** Per-service P2P targeting for WAVS v1.3 -- replace broadcast-all-and-filter with targeted per-service message delivery
**Researched:** 2026-04-03
**Overall confidence:** HIGH (based on commonware-p2p source code analysis, existing codebase inspection, GossipSub specification study, and distributed pub/sub pattern research)

---

## Context: Current Architecture

Before cataloguing features, the existing architecture must be understood because every v1.3 feature builds on (or replaces) existing primitives.

**What exists today (v1.0-v1.2):**
- Single commonware broadcast Engine for all services
- `Recipients::All` on every `mailbox.broadcast()` and `direct_sender.send()` call
- `ServiceRouter` with `HashSet<[u8; 32]>` filtering inbound messages at application level
- Per-peer deque catch-up via broadcast Engine (replays ALL cached messages, not service-scoped)
- `P2pCommand::Subscribe`/`Unsubscribe` update local `ServiceRouter` only (no peer notification)
- Messages carry `service_id_bytes` in `P2pMessage` envelope (32 bytes prefix)

**What commonware-p2p provides natively:**
- `Recipients::All` -- send to all connected peers
- `Recipients::Some(Vec<PublicKey>)` -- send to specific peers by Ed25519 pubkey
- `Recipients::One(PublicKey)` -- send to a single peer
- Rate-limited and unlimited sender traits
- Broadcast Engine with per-peer bounded deques and digest-based lookup

**Key insight:** The `Recipients::Some` primitive already exists in commonware-p2p. WAVS currently hardcodes `Recipients::All` everywhere. The v1.3 work is about building the application-level subscription tracking to know WHICH peers to include in `Recipients::Some`.

---

## Table Stakes

Features that are fundamental requirements for per-service P2P targeting. Without these, the feature is incomplete or broken.

### Subscription Tracking (service_id -> Set<PeerPubkey>)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Local subscription registry | Every P2P pub/sub system (GossipSub, NATS, Kafka) maintains a mapping of topics/subjects to interested parties. Without `service_id -> Set<Ed25519::PublicKey>`, the node cannot know which peers to target. This is the central data structure. | Low | `HashMap<[u8; 32], HashSet<ed25519::PublicKey>>` (or equivalent). Lives alongside `ServiceRouter` in the bridge loop. Updated by subscription announcements from peers. |
| Own-subscription bootstrapping | When a node subscribes to a service (via `P2pCommand::Subscribe`), it must both update local `ServiceRouter` AND announce to peers. Without this, peers never learn about subscriptions. | Low | Extend `P2pCommand::Subscribe` handler to: (1) update ServiceRouter, (2) broadcast a subscription announcement message to all connected peers. |
| Peer subscription state initialization | When a new peer connects (or reconnects), the node needs to learn that peer's current service subscriptions. GossipSub does this with a "hello" packet containing all current subscriptions. Without this, new peers receive no targeted messages until they announce. | Medium | Requires detecting peer connection events from commonware-p2p. On connect, exchange subscription sets. The commonware `Provider::subscribe()` method provides peer set change notifications that can trigger this. |

### Subscription Announcement Protocol

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Subscription announcement message type | A new P2pMessage variant (or separate message type) that carries subscription changes: `{action: Subscribe|Unsubscribe, service_ids: Vec<[u8; 32]>}`. Every pub/sub protocol has this (GossipSub uses `RPC.subscriptions[]` with `subscribe: bool` and `topicid`). Without it, peers cannot communicate interest. | Low | Add a `P2pControlMessage` enum alongside `P2pMessage`. Options: (A) multiplex control and data on the same channel via a tagged envelope, or (B) use a separate P2P channel. Option A is simpler and recommended. |
| Broadcast subscription changes to all peers | When a service is added/removed, the subscription announcement must go to ALL connected peers (not just service subscribers). GossipSub sends SUBSCRIBE/UNSUBSCRIBE to all pubsub-capable peers. This is correct because the announcement itself is metadata, not service data. | Low | Use `Recipients::All` for subscription announcements. These are small control messages, so bandwidth is negligible. |
| Idempotent subscription handling | Peers may reconnect, replay announcements, or send duplicate subscriptions. The registry must handle duplicates gracefully (insert into set is naturally idempotent). | Low | HashSet insert is O(1) and idempotent. Log duplicates at trace level. |

### Targeted Send via Recipients::Some

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Replace `Recipients::All` with `Recipients::Some` for service submissions | The core value proposition of v1.3. On `P2pCommand::Publish`, look up `service_id` in the subscription registry, collect peer pubkeys, and use `Recipients::Some(peers)`. If no peers are subscribed, fall back to `Recipients::All` (graceful degradation). | Medium | Modify both `mailbox.broadcast()` and `direct_sender.send()` calls in the bridge loop. Must handle both lookup and discovery mode bridge loops (code is duplicated). Also include self (own pubkey) is NOT in the recipients list per commonware semantics (broadcast does not echo to self). |
| Fallback to broadcast-all for unknown services | If a service has no known subscribers (e.g., just deployed, peers haven't announced yet), the node should broadcast to all peers rather than silently dropping. This prevents message loss during bootstrap. | Low | `if subscribers.is_empty() { Recipients::All } else { Recipients::Some(subscribers) }`. Log when falling back. |
| Include all peers for subscription announcements | Subscription control messages themselves must always use `Recipients::All` because they are network metadata, not service-specific data. Targeting subscription messages only to service subscribers creates a chicken-and-egg problem. | Low | Already natural -- announcements go to all peers. |

### Subscription Lifecycle

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Dynamic service add: announce subscription | When a service is registered at runtime (via `POST /service`), the node subscribes locally and announces to peers. Existing `AggregatorCommand::SubscribeService` already triggers `P2pCommand::Subscribe`. The P2P command handler must now also send an announcement. | Low | Extend the `Subscribe` arm of the bridge loop to broadcast a control message. |
| Dynamic service remove: announce unsubscription | When a service is removed, the node unsubscribes locally and announces to peers. Existing `AggregatorCommand::UnsubscribeService` already triggers `P2pCommand::Unsubscribe`. | Low | Extend the `Unsubscribe` arm similarly. |
| Peer disconnect: cleanup subscription state | When a peer disconnects, its entries in the subscription registry must be removed. Otherwise, `Recipients::Some` will target disconnected peers (commonware drops the message silently for offline recipients, but the intent is wrong). | Medium | Detect peer disconnects via commonware `Provider::subscribe()` peer set change notifications. When a peer leaves tracked sets, remove all its subscription entries. Alternatively, rely on commonware's behavior that `Recipients::Some` with an offline peer simply drops the message -- no error, just wasted effort. |
| Node restart: re-announce all subscriptions | On startup, after P2P connects, the node must announce its full subscription set to all peers (like GossipSub's "hello" packet). Without this, returning peers are invisible to the network until they add a new service. | Medium | After network startup and initial peer connections, iterate all registered services and send a bulk subscription announcement. Timing is tricky -- must wait for peers to connect first. Use the heartbeat timer or a dedicated startup delay. |

---

## Differentiators

Features that go beyond basic targeted send and provide additional value. Not strictly required for v1.3 to work, but make it significantly better.

### Per-Service Catch-Up Scoping

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Filter catch-up replay by service subscription | Currently, the broadcast Engine replays ALL cached messages on reconnect. With per-service targeting, a reconnecting peer should only receive catch-up messages for services it subscribes to. Reduces bandwidth and processing for operators running few services in a many-service network. | High | The commonware broadcast Engine does NOT support per-message filtering on replay. It replays the entire per-peer deque. Options: (A) Accept unfiltered catch-up and rely on ServiceRouter to discard (current behavior, simplest), (B) Build application-level catch-up on top of the Engine's `mailbox.get(digest)` API, (C) Maintain separate per-service message caches at application level. **Recommendation: defer to v1.4 or accept option A for now.** The Engine's deque is bounded (default 128 messages) so the overhead is manageable. |
| Service-scoped message cache | Instead of one global deque per peer in the Engine, maintain per-service message caches at the application level. On reconnect, only replay messages for the reconnecting peer's subscribed services. | High | Requires a parallel caching layer outside the Engine. The Engine caches all messages indiscriminately. Application would need to intercept outbound messages and store them by service_id, then replay on demand. Significant complexity for modest gain. **Recommendation: not for v1.3.** |

### Observability Enhancements

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Per-service peer count in /p2p/status | Expose `service_subscriptions: Map<ServiceId, Vec<PeerHex>>` in the P2pStatus response. Operators can see which peers handle which services. Useful for debugging quorum failures ("why did my 3-operator BLS service only get 2 signatures? peer C isn't subscribed"). | Low | Add field to `P2pStatus` struct. Populate from the subscription registry. This is low-cost high-value observability. |
| Targeted vs broadcast metric | Track how many messages are sent via `Recipients::Some` vs `Recipients::All` (fallback). High fallback rate indicates subscription protocol issues. | Low | Counter metrics: `p2p_targeted_sends`, `p2p_broadcast_fallback_sends`. Increment in the publish handler. |
| Subscription event logging | Structured log entries when peers subscribe/unsubscribe, with peer ID and service ID. Essential for debugging "peer X says it handles service Y but never sends submissions." | Low | Already partially done (tracing::info on subscribe/unsubscribe). Extend to log peer subscription announcements received. |

### Protocol Robustness

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Subscription heartbeat/refresh | Periodic re-announcement of full subscription set (e.g., every 60s). Protects against subscription state drift if announcements are lost. GossipSub relies on connection-time hello packets, not heartbeats. A heartbeat is more robust for long-running connections. | Low | Piggyback on the existing 2-second heartbeat interval (currently used for peer discovery probes). Every N heartbeats (e.g., every 30th = 60s), re-broadcast full subscription set. |
| Subscription validation | Verify that a peer announcing subscription to service X actually has the service registered (i.e., is in the on-chain operator set). Prevents peers from subscribing to services they don't operate, reducing unnecessary message delivery. | High | Requires on-chain lookups (POA/EigenLayer operator registries). Not practical for v1.3 -- would need per-service contract queries. **Recommendation: trust peer announcements for now. Misbehaving peers are handled by Oracle-level blocking.** |

---

## Anti-Features

Features to explicitly NOT build in v1.3. Important to document to prevent scope creep.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Per-service P2P channels | Registering a separate commonware P2P channel per service would provide true network-level isolation. However, commonware channels are registered at startup time via `network.register()` before `network.start()`. Dynamic channel creation is not supported. Multiple channels also multiply rate-limit tracking and connection overhead. | Keep single broadcast channel. Use `Recipients::Some` for targeting within the channel. ServiceRouter remains as a safety net for any messages that leak through. |
| Content-based routing | Routing based on message content attributes (e.g., chain, signature algorithm, workflow ID). More flexible than topic-based but adds matching complexity and is overkill for service-level isolation. | Use simple service_id-based topic routing. The service_id is already the first 32 bytes of every P2pMessage, making it trivially extractable. |
| Separate subscription protocol channel | Using channel 2 exclusively for subscription control messages. Adds another channel to manage, rate-limit, and bridge. Control messages are small and infrequent. | Multiplex control and data messages on the existing channels via a tagged envelope (discriminator byte or enum). |
| Recursive subscription propagation | In GossipSub, subscription announcements propagate only to direct peers (one hop). Some systems propagate subscriptions across multiple hops. For WAVS with small operator sets (typically 3-20), all peers are directly connected. Multi-hop propagation is unnecessary overhead. | Direct-peer-only announcements. All WAVS peers are typically within one hop of each other in the mesh. |
| Threshold-based subscription gating | Requiring a minimum number of subscribers before enabling targeted send. Over-engineering for a feature that works fine with graceful fallback to broadcast-all. | Fall back to `Recipients::All` when subscriber set is empty. No artificial minimums. |
| On-chain subscription verification | Verifying subscription claims against on-chain operator registries before accepting. Adds latency, requires chain queries, and is fragile if chains are slow or unavailable. | Trust peer announcements. Malicious peers are handled at the Oracle/block level. Unregistered operators fail at submission time anyway (contract rejects their signatures). |
| Per-service catch-up protocol | Building a custom catch-up mechanism that replays only service-specific messages on reconnect. The commonware Engine's existing deque replay is unfiltered. Building filtered replay requires a parallel caching layer. | Accept the current catch-up behavior for v1.3. The Engine replays all cached messages (bounded by deque_size, default 128), and ServiceRouter filters at application level. The bandwidth overhead is acceptable for the current scale. Revisit in v1.4 if operator counts exceed ~50. |

---

## Feature Dependencies

```
Subscription Registry ──────┐
                             │
Subscription Announcement ──┤── Targeted Send (Recipients::Some)
Protocol                     │
                             │
Peer Connection Detection ──┘
        │
        └── Peer Disconnect Cleanup
        └── Node Restart Re-announce

Targeted Send ──────────────── Fallback to Recipients::All
                                (when subscriber set empty)

Subscription Registry ──────── /p2p/status enhancements
                                (per-service peer counts)

[Independent]
Per-Service Catch-Up Scoping ── NOT a dependency of targeting
                                 (deferred, Engine handles catch-up)
```

**Critical path:** Subscription Registry -> Announcement Protocol -> Targeted Send. These three are tightly coupled and must be built together.

**Independent:** Observability enhancements, heartbeat refresh, peer disconnect cleanup. These can be added incrementally.

---

## Comparison: GossipSub Topics vs WAVS Per-Service Targeting

This comparison is explicitly requested by the quality gate and is important for understanding the design space.

| Aspect | GossipSub (libp2p) | WAVS v1.3 Per-Service Targeting |
|--------|-------------------|-------------------------------|
| **Topic isolation** | Full network-level isolation. Each topic has its own mesh (D peers), gossip set, and message flow. Topics never mix at the transport level. | Application-level targeting via `Recipients::Some`. All messages share the same commonware channel. ServiceRouter provides a safety net. |
| **Subscription protocol** | Built into pubsub spec. `RPC.subscriptions[]` with `subscribe: bool, topicid: string`. "Hello" packet on connect with all current subscriptions. Subscriptions are NOT propagated beyond direct peers. | Must be built as application-level control messages. Equivalent to GossipSub's hello + subscription change announcements. |
| **Mesh management** | D_lo/D_hi mesh bounds per topic. GRAFT/PRUNE to maintain mesh density. Heartbeat (1s default) checks mesh health. Fan-out for unsubscribed topics. | No mesh management needed. commonware-p2p handles connection management. WAVS just selects recipients from the subscription registry. Simpler because there is no per-topic mesh to maintain. |
| **Catch-up** | IHAVE/IWANT gossip. Peers exchange message IDs, request missing messages. Per-topic. | Broadcast Engine deque replay (all messages, all topics). Not per-service. ServiceRouter filters at application level. |
| **Complexity** | High. GossipSub v1.1 is ~50 pages of spec. Scoring, flood publishing, peer exchange, message validation. | Low-Medium. Subscription tracking + `Recipients::Some`. No mesh management, no scoring, no GRAFT/PRUNE. Commonware handles the hard networking parts. |
| **When better** | Large networks (>100 peers), many topics (>50), topics with very different subscriber sets, need for gossip-based catch-up. | Small-medium networks (<50 peers), moderate topics (<50 services), all peers authenticated, trusted operator environment. |

**Verdict:** WAVS does not need GossipSub-level complexity. The commonware-p2p `Recipients::Some` primitive combined with application-level subscription tracking achieves the goal with dramatically less complexity. GossipSub's mesh management (GRAFT/PRUNE/heartbeat/scoring) is designed for untrusted, large-scale networks. WAVS has authenticated, Oracle-managed peer sets -- a much simpler environment.

---

## MVP Recommendation

**Phase 1 (v1.3 core) -- build together, they are interdependent:**

1. **Subscription Registry** (`HashMap<[u8; 32], HashSet<ed25519::PublicKey>>`) -- the central data structure
2. **Control Message Protocol** -- tagged P2pMessage variant for subscribe/unsubscribe announcements
3. **Targeted Send** -- replace `Recipients::All` with `Recipients::Some(service_peers)` in publish handler, with fallback
4. **Subscription Lifecycle** -- announce on subscribe/unsubscribe, re-announce on startup
5. **Per-service peer count in /p2p/status** -- low-effort, high-value observability

**Defer to v1.4 or later:**

- **Per-service catch-up scoping**: High complexity, low urgency. The Engine's bounded deque (128 messages) and ServiceRouter filtering are sufficient at current scale. Only becomes important with >50 services or >50 operators.
- **Subscription heartbeat/refresh**: Nice-to-have robustness. Can be added later as a small enhancement. Subscription state drift is unlikely in small operator sets.
- **Peer disconnect cleanup**: Low priority because commonware silently drops messages to offline peers. The subscription registry will have stale entries, but `Recipients::Some` handles offline recipients gracefully. Can add via `Provider::subscribe()` notifications later.
- **Subscription validation**: Requires on-chain lookups. Not practical for v1.3.

---

## Sources

- [GossipSub v1.0 Specification](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md) -- subscription protocol, mesh management, hello packets
- [GossipSub v1.1 Specification](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md) -- peer scoring, extended validation
- [Publish-subscribe pattern (Wikipedia)](https://en.wikipedia.org/wiki/Publish%E2%80%93subscribe_pattern) -- topic-based vs content-based routing
- [commonware-p2p source](https://github.com/commonwarexyz/monorepo) -- `Recipients` enum, `Sender` trait, rate limiting (v2026.3.0)
- [commonware-broadcast source](https://github.com/commonwarexyz/monorepo) -- `Engine` cache/deque, `Mailbox` API (v2026.3.0)
- Codebase analysis: `packages/wavs/src/subsystems/aggregator/p2p.rs` (ServiceRouter, P2pHandle, bridge loops)
- Codebase analysis: `packages/wavs/src/subsystems/aggregator.rs` (AggregatorCommand variants, subscription lifecycle)
- [Selective Delivery in P2P Topic-Based Pub/Sub Systems](https://www.researchgate.net/publication/308814521_Selective_Delivery_of_Event_Messages_in_Peer-to-Peer_Topic-Based_Publish_Subscribe_Systems)
