# Technology Stack

**Project:** WAVS v1.3 Per-Service P2P Targeting
**Researched:** 2026-04-03
**Overall confidence:** HIGH

This document covers ONLY the stack additions/changes needed for v1.3: replacing `Recipients::All` broadcast with `Recipients::Some(service_peers)` targeted delivery, adding a service subscription protocol between peers, scoping catch-up per service, and managing subscription lifecycle. The existing commonware stack (p2p 2026.3.0, broadcast 2026.3.0, cryptography 2026.3.0, codec 2026.3.0) is validated and NOT re-researched.

## Executive Summary

**No new crate dependencies are needed.** The entire v1.3 feature set is achievable with the existing commonware-p2p 2026.3.0 stack. The `Recipients::Some(Vec<PublicKey>)` variant already exists in commonware-p2p and is already imported but unused -- every call site currently uses `Recipients::All`. The work is:

1. **New subscription protocol** -- peers exchange service subscription announcements over the existing P2P channels to build a `service_id -> Set<PublicKey>` map
2. **Replace `Recipients::All` with `Recipients::Some`** -- look up service peers from the subscription map when publishing
3. **Per-service catch-up scoping** -- filter the broadcast Engine's cached messages by service_id on replay, or use subscription announcements to avoid replaying irrelevant messages
4. **Subscription lifecycle** -- extend existing `P2pCommand::Subscribe`/`Unsubscribe` to trigger announcement broadcasts to peers

No new runtime dependencies. No commonware version bump required. No architecture changes to the two-channel broadcast pattern. The subscription protocol is a pure application-layer addition.

## Recommended Stack -- NO Additions, Internal Changes Only

### Existing Dependencies (unchanged)

| Crate | Version | Role in v1.3 | Confidence |
|-------|---------|-------------|------------|
| `commonware-p2p` | 2026.3.0 | `Recipients::Some(Vec<PublicKey>)` for targeted send. `Sender::send()` already accepts `Recipients` enum. | HIGH -- verified from source in cargo registry |
| `commonware-broadcast` | 2026.3.0 | `Mailbox::broadcast()` already accepts `Recipients` enum. Engine caches per-peer, no changes needed. | HIGH -- verified from source |
| `commonware-codec` | 2026.3.0 | `Encode`/`Decode` for new `SubscriptionMessage` type. Existing derive pattern via `Write`/`Read`/`EncodeSize`. | HIGH -- same pattern as `P2pMessage` |
| `commonware-cryptography` | 2026.3.0 | `ed25519::PublicKey` for peer identification in subscription map. `Sha256` for message dedup. | HIGH -- unchanged |
| `commonware-runtime` | 2026.3.0 | `Buf`/`BufMut` for codec. No changes. | HIGH |
| `commonware-utils` | 2026.3.0 | `ordered::Map`/`ordered::Set` for peer tracking. Already used for Oracle. | HIGH |
| `commonware-math` | 2026.3.0 | `Random` trait for Ed25519 key derivation. Unchanged. | HIGH |

### Standard Library / Existing Workspace Dependencies Used

| Dependency | Already In | Use in v1.3 |
|-----------|-----------|-------------|
| `std::collections::HashMap` | stdlib | `service_id -> HashSet<PublicKey>` subscription map |
| `std::collections::HashSet` | stdlib | Per-service peer set |
| `dashmap` | workspace | Alternative for thread-safe subscription map if shared across tasks (already used elsewhere in WAVS) |
| `serde` + `serde_json` | workspace | Serialization of subscription announcements |
| `tokio::sync::mpsc` | workspace | Already used for P2P command channel |
| `tracing` | workspace | Logging subscription events |

## Key Integration Points

### 1. `Recipients::Some` -- Already Available, Zero Changes to commonware

The `Recipients` enum in commonware-p2p 2026.3.0 (verified from `/Users/jacobhartnell/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/commonware-p2p-2026.3.0/src/lib.rs` line 42-46):

```rust
pub enum Recipients<P: PublicKey> {
    All,
    Some(Vec<P>),
    One(P),
}
```

Both `Sender::send()` (direct channel) and `Broadcaster::broadcast()` (Engine mailbox) accept `Recipients` as a parameter. The current code passes `Recipients::All` at **14 call sites** in `p2p.rs`. Replacing with `Recipients::Some(peers)` requires only a peer lookup per service_id before each send.

**How `Recipients::Some` works in commonware-p2p:**
- The network layer iterates `Vec<P>` and sends to each connected peer in the list
- Peers not currently connected are silently skipped (same as `Recipients::All` for offline peers)
- The `send()` return value is `Vec<PublicKey>` -- the peers that actually received the message
- Rate limiting still applies per-peer via `LimitedSender::check()`

**Confidence:** HIGH -- read directly from commonware-p2p source code.

### 2. Subscription Protocol -- Application Layer Over Existing Channels

No commonware-level subscription mechanism exists. The subscription protocol is entirely application-level, running on the same two P2P channels already configured.

**Approach:** Introduce a new `P2pEnvelope` enum (or extend `P2pMessage`) that wraps both data messages and control messages:

```rust
enum P2pEnvelope {
    /// Submission data (existing P2pMessage content)
    Submission(P2pMessage),
    /// Subscription announcement
    Subscription(SubscriptionAnnouncement),
}

struct SubscriptionAnnouncement {
    /// Services this peer subscribes to (full set, not delta)
    service_ids: Vec<[u8; 32]>,
}
```

**Why full-set, not delta:** Simpler convergence. On reconnect or missed announcement, peers exchange full subscription lists. No need for sequence numbers or conflict resolution. Small payloads -- even 100 services is only 3.2KB.

**Why not a separate P2P channel:** Registering a third channel adds complexity. Subscription announcements are infrequent (on service add/remove and peer connect) and small. Sharing the existing channel with envelope-based multiplexing is simpler and the established pattern.

**Confidence:** HIGH -- this is standard application-layer protocol design with no commonware dependencies.

### 3. Subscription Map -- New Data Structure in Bridge Loop

The bridge loop (inside `run_lookup_network` and `run_discovery_network`) needs a new data structure:

```rust
/// Maps service_id -> set of peer public keys subscribed to that service
struct PeerSubscriptionMap {
    /// service_id bytes -> set of peer pubkeys
    subscriptions: HashMap<[u8; 32], HashSet<ed25519::PublicKey>>,
    /// reverse index: peer -> set of service_ids (for cleanup on disconnect)
    peer_services: HashMap<ed25519::PublicKey, HashSet<[u8; 32]>>,
}
```

**Why `HashMap` not `DashMap`:** The subscription map lives inside the single-threaded bridge loop (`tokio::select!`). No concurrent access. `HashMap` is simpler and faster.

**Why reverse index:** When a peer disconnects (detected via heartbeat timeout or `evict_untracked_peers` from the Provider subscription), we need to remove that peer from all service sets. Without the reverse index, this is O(services * peers). With it, O(services_for_that_peer).

**Confidence:** HIGH -- straightforward Rust data structures, no external dependencies.

### 4. Catch-Up Scoping -- Filtered Replay via Existing Engine

The broadcast Engine (commonware-broadcast 2026.3.0) caches messages per-peer in bounded deques. On peer reconnect, the Engine automatically replays cached messages. Currently, ALL cached messages are replayed regardless of service_id.

**Two approaches for per-service catch-up scoping:**

**Approach A (Recommended): Receiver-side filtering (no Engine changes)**
- The reconnecting peer already has a `ServiceRouter` that filters inbound messages by `should_accept()`
- Irrelevant replayed messages are filtered at the application layer
- The Engine replays everything, but only subscribed-service messages reach the Aggregator
- **Pro:** Zero changes to commonware-broadcast. Already works today.
- **Con:** Wastes bandwidth replaying messages the peer will filter out

**Approach B: Separate Engine instances per service**
- Create one broadcast Engine per service_id
- Each Engine only caches messages for its service
- Reconnecting peers only get catch-up for their subscribed services
- **Pro:** Perfect catch-up scoping, no wasted bandwidth
- **Con:** Major refactor. Multiple Engine instances means multiple P2P channels per service, requiring dynamic channel registration. commonware-p2p's `network.register()` must be called before `network.start()` -- channels cannot be added dynamically.

**Recommendation: Approach A.** The bandwidth cost of replaying filtered messages is negligible for realistic deployments (catch-up deque is bounded at 128 messages per peer, each message is small). The complexity cost of Approach B is prohibitive and would require upstream commonware changes for dynamic channel registration.

**Confidence:** HIGH for Approach A (verified from Engine source that replay is push-based, no filtering capability). MEDIUM for Approach B assessment (based on reading `network.register()` API -- channels appear to be registered before `start()`).

### 5. Heartbeat Integration -- Subscription Exchange on Connect

The existing heartbeat mechanism (2-second interval, all-zeros `HEARTBEAT_SERVICE_ID` sentinel) already probes the mesh and tracks connected peers via `connected_peers_tracker`. This is the natural place to exchange subscription announcements.

**When to send subscription announcements:**
1. **On service subscribe/unsubscribe** -- broadcast to all peers (since we don't yet know who cares)
2. **On new peer detected** -- send our subscription list to the newly connected peer
3. **Periodically (optional)** -- piggyback on heartbeat for convergence safety

New peer detection already happens in the inbound message handler when a peer_pubkey is first seen in `connected_peers_tracker`. This is the trigger point.

**Confidence:** HIGH -- leverages existing heartbeat and peer tracking code.

## What NOT to Add

### Crates to Avoid

| Crate | Why Considered | Why NOT |
|-------|---------------|---------|
| `libp2p` | GossipSub has native topic-based pub/sub | Already removed in v1.0. Going back would be a regression. commonware's `Recipients::Some` achieves the same goal. |
| `commonware-consensus` | Could provide consensus on subscription state | Massive overkill. Subscription state is soft/best-effort, not safety-critical. |
| `commonware-sync` | Could synchronize subscription state | Same reason. Application-level announcement convergence is sufficient. |
| `tokio-cron-scheduler` | Could schedule periodic subscription refresh | `tokio::time::interval` (already used for heartbeat) is sufficient. |
| Any pub/sub crate | Topic-based subscription | Re-implementing what we already have with `ServiceRouter` + `Recipients::Some`. |

### Patterns to Avoid

| Pattern | Why Tempting | Why Bad |
|---------|-------------|---------|
| Per-service P2P channels | Clean isolation | `network.register()` appears to require pre-start registration. Dynamic services would break this. Even if dynamic registration worked, N services = N channels = complexity explosion. |
| Delta-based subscription updates | Lower bandwidth per update | Requires sequence numbers, missed-message recovery, conflict resolution. Full-set announcements are small enough (32 bytes * num_services) that simplicity wins. |
| Persistent subscription storage | Survives restart | Subscriptions are re-announced on connect. Persisting adds complexity with zero benefit -- peers re-send their subscription lists when they reconnect. |
| Separate subscription channel (3rd P2P channel) | Clean separation of control and data | More channels = more complexity, more Engine instances. Envelope-based multiplexing on existing channels is simpler. |

## Migration Path (Existing -> v1.3)

### Step 1: New Message Types (non-breaking)
Add `P2pEnvelope` and `SubscriptionAnnouncement` types with `commonware_codec` derive. Add `PeerSubscriptionMap` struct. All additive, no existing behavior changes.

### Step 2: Envelope Wrapping (breaking internal wire format)
Wrap existing `P2pMessage` in `P2pEnvelope::Submission`. Update encode/decode paths. Old peers will fail to decode new messages and vice versa -- this is acceptable because v1.3 is a breaking P2P protocol change.

### Step 3: Subscription Protocol
On subscribe/unsubscribe, broadcast `P2pEnvelope::Subscription` to `Recipients::All`. On inbound subscription announcement, update `PeerSubscriptionMap`. On new peer detected, send own subscription list.

### Step 4: Targeted Delivery
Replace `Recipients::All` with `Recipients::Some(service_peers)` at publish call sites. Fall back to `Recipients::All` if no subscription data available (graceful degradation for mixed-version networks during rollout).

### Step 5: Status Endpoint Enhancement
Extend `P2pStatus` to include per-peer subscription info for observability.

## Wire Format Compatibility

v1.3 introduces a **breaking wire format change** (P2pMessage -> P2pEnvelope). This is acceptable because:
- WAVS is pre-production (v2.8.0, all operators in coordinated deployments)
- The existing P2P protocol has no versioning -- adding it now (via envelope discriminant byte) sets up future compatibility
- All operators in a deployment upgrade together

## Versions Summary

| Dependency | Current | v1.3 | Change |
|-----------|---------|------|--------|
| commonware-p2p | 2026.3.0 | 2026.3.0 | None |
| commonware-broadcast | 2026.3.0 | 2026.3.0 | None |
| commonware-codec | 2026.3.0 | 2026.3.0 | None |
| commonware-cryptography | 2026.3.0 | 2026.3.0 | None |
| commonware-runtime | 2026.3.0 | 2026.3.0 | None |
| commonware-utils | 2026.3.0 | 2026.3.0 | None |
| commonware-math | 2026.3.0 | 2026.3.0 | None |
| New external crates | -- | -- | None needed |

## Sources

- commonware-p2p 2026.3.0 source: `/Users/jacobhartnell/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/commonware-p2p-2026.3.0/src/lib.rs` (Recipients enum, Sender/Broadcaster traits) -- HIGH confidence
- commonware-broadcast 2026.3.0 source: `/Users/jacobhartnell/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/commonware-broadcast-2026.3.0/src/` (Engine cache behavior, Mailbox broadcast API) -- HIGH confidence
- WAVS p2p.rs source: `/Users/jacobhartnell/Dev/projects/Layer/WAVS/packages/wavs/src/subsystems/aggregator/p2p.rs` (current implementation, 14 `Recipients::All` call sites, ServiceRouter, P2pCommand enum) -- HIGH confidence
- [commonware-p2p on crates.io](https://crates.io/crates/commonware-p2p) -- version 2026.3.0 is latest
- [commonware-p2p docs](https://docs.rs/commonware-p2p/2026.3.0/commonware_p2p/) -- API reference
- [commonware GitHub monorepo](https://github.com/commonwarexyz/monorepo) -- source of truth for commonware crates
