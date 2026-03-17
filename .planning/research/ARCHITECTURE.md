# Architecture Patterns: Commonware P2P Migration in WAVS Aggregator

**Domain:** P2P networking layer replacement (libp2p -> commonware)
**Researched:** 2026-03-17
**Overall confidence:** MEDIUM (commonware docs verified via docs.rs; runtime integration strategy needs prototype validation)

## Current Architecture (libp2p)

The P2P layer lives entirely in `packages/wavs/src/subsystems/aggregator/p2p.rs` (~1,840 lines). It is self-contained behind a `P2pHandle` abstraction that the `Aggregator` struct holds as `Arc<RwLock<Option<P2pHandle>>>`.

```
Dispatcher
  |
  v (crossbeam channel: AggregatorCommand)
Aggregator
  |-- p2p_handle: Arc<RwLock<Option<P2pHandle>>>
  |       |
  |       +-- command_tx: mpsc::UnboundedSender<P2pCommand>
  |       |       |
  |       |       v
  |       +-- [tokio::spawn] run_event_loop()
  |               |
  |               +-- Swarm<WavsBehaviour>
  |                       |
  |                       +-- gossipsub (per-service topic pub/sub)
  |                       +-- catchup (request/response for missed messages)
  |                       +-- mdns (local discovery) OR kademlia (prod discovery)
  |                       +-- identify (peer identification)
  |                       +-- autonat (NAT traversal)
  |
  +-- aggregator_to_self_tx (crossbeam, for loopback)
```

### Current Data Flow

1. **Outbound (Broadcast):** `Aggregator.handle_broadcast()` -> `P2pHandle.publish()` -> `P2pCommand::Publish` via mpsc -> event loop -> `swarm.gossipsub.publish(topic, data)`
2. **Inbound (Receive):** swarm event -> `handle_gossip_message()` -> `aggregator_tx.send(AggregatorCommand::Receive)` via crossbeam -> Aggregator
3. **Catch-up:** On connection established -> request catch-up for each subscribed service -> peer responds with stored submissions -> forwarded as `AggregatorCommand::Receive`
4. **Subscribe/Unsubscribe:** `AggregatorCommand::SubscribeService` -> `P2pHandle.subscribe()` -> `P2pCommand::Subscribe` -> event loop subscribes gossipsub topic

### Current Key Abstractions

| Abstraction | Type | Purpose |
|-------------|------|---------|
| `P2pHandle` | Clonable struct with mpsc sender | Facade for sending commands to the P2P event loop |
| `P2pCommand` | Enum (Publish, Subscribe, Unsubscribe, GetStatus) | Commands from application to P2P |
| `P2pConfig` | Enum (Disabled, Local, Remote) | Configuration variants |
| `EventLoopState` | Struct | Mutable state for the event loop (topics, pending publishes, stored submissions) |
| `Peer` | Enum (Me, Other(String)) | Identity of submission source |
| `AggregatorCommand` | Enum (Kill, Broadcast, Receive, Actions, SubscribeService, UnsubscribeService) | Commands to aggregator (unchanged by migration) |

---

## Recommended Architecture (commonware)

### Component Mapping

| libp2p Component | commonware Replacement | Notes |
|-----------------|----------------------|-------|
| `libp2p::Swarm` | `commonware_p2p::authenticated::discovery::Network` | Handles connections, encryption, peer management |
| GossipSub (per-service topics) | `commonware_broadcast::buffered::Engine` + per-service channels | One broadcast Engine per service, OR single Engine with message routing |
| Kademlia DHT (prod discovery) | `commonware_p2p::authenticated::discovery` (bootstrapper-based) | Bootstrappers replace DHT bootstrap nodes |
| mDNS (dev discovery) | `commonware_p2p::authenticated::lookup` | Known addresses for dev, provided by application |
| Request/Response (catch-up) | `commonware_broadcast::buffered` (built-in caching + digest retrieval) | Buffered engine caches messages and serves them on demand |
| libp2p secp256k1 identity | `commonware_cryptography::ed25519::Signer` | Ed25519 for P2P identity only (on-chain remains ECDSA) |
| Identify + AutoNAT | Not needed | commonware handles peer authentication natively |

### Component Boundaries

```
Dispatcher
  |
  v (crossbeam channel: AggregatorCommand -- UNCHANGED)
Aggregator
  |-- p2p_handle: Arc<RwLock<Option<P2pHandle>>>  (INTERFACE PRESERVED)
  |       |
  |       +-- command_tx: mpsc::UnboundedSender<P2pCommand>  (PRESERVED)
  |       |       |
  |       |       v
  |       +-- [commonware runtime] P2P Coordinator Task
  |               |
  |               +-- CommonwareP2pNetwork (authenticated::discovery::Network OR lookup::Network)
  |               |       |
  |               |       +-- Oracle (manages authorized peer sets)
  |               |       +-- Channel: "wavs/broadcast" (main channel for broadcast Engine)
  |               |       +-- Channel: "wavs/control" (optional: peer status queries)
  |               |
  |               +-- BroadcastEngine (commonware_broadcast::buffered::Engine)
  |               |       |
  |               |       +-- Mailbox (implements Broadcaster trait)
  |               |       +-- Per-peer message caching (replaces stored_submissions)
  |               |       +-- Digest-based retrieval (replaces catch-up protocol)
  |               |
  |               +-- ServiceRouter (new: routes messages to correct service)
  |                       |
  |                       +-- Maps service_id -> message filtering
  |                       +-- Validates incoming messages
  |
  +-- aggregator_to_self_tx (crossbeam, for loopback -- UNCHANGED)
```

### Detailed Component Design

#### 1. CommonwareP2pNetwork (Connection Layer)

**Responsibility:** Authenticated peer connections, encryption, peer discovery.

**Key decisions:**
- Use `discovery::Network` for production (bootstrapper-based, replaces Kademlia)
- Use `lookup::Network` for local dev (address-known, replaces mDNS)
- Both implement the same channel-based Sender/Receiver interface, so the broadcast engine works with either

**Configuration mapping:**

```
P2pConfig::Disabled -> No network created (same as today)
P2pConfig::Local { listen_port } -> lookup::Network with known peer addresses
P2pConfig::Remote { listen_port, bootstrap_nodes } -> discovery::Network with bootstrappers
```

**Channel registration:**
Register a single channel with the P2P network before calling `start()`. The broadcast Engine consumes the Sender/Receiver from this channel.

```rust
// Pseudocode
let (mut network, mut oracle) = discovery::Network::new(context, p2p_config);
let (sender, receiver) = network.register(
    BROADCAST_CHANNEL,
    Quota::per_second(/* rate limit */),
    MESSAGE_BACKLOG,
);
// Pass sender/receiver to broadcast engine
```

#### 2. BroadcastEngine (Message Dissemination Layer)

**Responsibility:** Reliable broadcast of submissions, message caching, digest-based retrieval.

**This replaces three libp2p components at once:**
- GossipSub (broadcast)
- Stored submissions (caching)
- Request/Response catch-up protocol (digest-based retrieval)

**Key insight:** The buffered Engine already handles the catch-up problem. When a peer reconnects, it can request messages by digest from the Engine's cache. This eliminates the need for the entire `CatchUpRequest`/`CatchUpResponse`/`CatchUpCodec` implementation (~100 lines) and the `stored_submissions` tracking in `EventLoopState` (~80 lines).

**Message type:** Define a `WavsMessage` that wraps `Submission` and implements `Digestible + Codec`:

```rust
#[derive(Clone, Serialize, Deserialize)]
struct WavsMessage {
    service_id: ServiceId,
    submission: Submission,
}

impl Digestible for WavsMessage {
    fn digest(&self) -> [u8; 32] {
        // Hash of (service_id, event_id, signer_address) for deduplication
    }
}
```

#### 3. ServiceRouter (Message Routing Layer)

**Responsibility:** Per-service message filtering and validation (replaces GossipSub topic isolation).

**Why this is needed:** commonware-broadcast does not have per-topic isolation like GossipSub. All messages go through a single broadcast Engine. The ServiceRouter filters inbound messages by service_id and validates that the local node is subscribed to the relevant service.

**Design:**

```rust
struct ServiceRouter {
    subscribed_services: HashSet<ServiceId>,
}

impl ServiceRouter {
    fn should_accept(&self, msg: &WavsMessage) -> bool {
        self.subscribed_services.contains(&msg.service_id)
    }

    fn subscribe(&mut self, service_id: ServiceId) { ... }
    fn unsubscribe(&mut self, service_id: ServiceId) { ... }
}
```

**Tradeoff:** With libp2p GossipSub, operators only receive messages for their subscribed services (topic filtering happens at the network level). With commonware broadcast, all operators receive all messages and filter locally. For WAVS's expected scale (tens to low hundreds of operators, moderate message rates), this is acceptable. If scale becomes a concern, multiple broadcast channels per service could be introduced later.

#### 4. P2pHandle (Preserved Facade)

**Responsibility:** Unchanged external interface for the Aggregator.

The `P2pHandle` struct, `P2pCommand` enum, and the `publish`/`subscribe`/`unsubscribe`/`get_status` methods remain the same. The internal implementation changes from sending to a libp2p swarm event loop to sending to the commonware coordinator task.

```rust
// P2pHandle interface remains identical:
impl P2pHandle {
    pub fn publish(&self, submission: &Submission) -> Result<(), AggregatorError>;
    pub fn subscribe(&self, service_id: &ServiceId) -> Result<(), AggregatorError>;
    pub fn unsubscribe(&self, service_id: &ServiceId) -> Result<(), AggregatorError>;
    pub async fn get_status(&self) -> Result<P2pStatus, AggregatorError>;
}
```

#### 5. Identity Layer (Ed25519)

**Responsibility:** Deterministic P2P identity from signing mnemonic.

**Key change:** Replace secp256k1 derivation with Ed25519 key generation.

The current `keypair_from_mnemonic()` derives a secp256k1 key from the mnemonic via the EVM HD path (m/44'/60'/0'/0/0). For commonware, derive an Ed25519 key instead:

```rust
fn ed25519_signer_from_mnemonic(mnemonic: &str) -> Result<ed25519::Signer, AggregatorError> {
    // Option A: Hash the mnemonic seed to get 32 bytes for Ed25519
    // Option B: Use commonware_cryptography::ed25519::Signer::from_seed()
    // The seed should be deterministic from the mnemonic
}
```

**Constraint:** On-chain signatures remain ECDSA (handled by SubmissionManager, out of scope). Ed25519 is only for P2P peer authentication.

---

## Runtime Integration Strategy

### The Challenge

commonware-runtime's `tokio::Runner` creates its own Tokio runtime via `tokio::runtime::Builder::new_multi_thread()`. WAVS already has its own Tokio runtime. Running two Tokio runtimes in the same process is technically possible but wasteful and can cause confusion.

### Recommended Approach: Dedicated commonware Runtime

**Confidence: MEDIUM -- needs prototype validation**

Run commonware's `tokio::Runner` as the owner of the P2P subsystem in its own thread. The WAVS aggregator communicates with it via the existing `mpsc::UnboundedSender<P2pCommand>` channel (which is cross-runtime safe since tokio mpsc channels work across runtime boundaries).

```
WAVS Main Tokio Runtime (existing)
  |
  +-- Aggregator thread
  |       |
  |       +-- P2pHandle (holds mpsc::UnboundedSender)
  |
  +-- [std::thread::spawn] Commonware Runtime Thread
          |
          +-- commonware::tokio::Runner::start(|ctx| async {
          |       let (network, oracle) = discovery::Network::new(ctx, cfg);
          |       let (sender, receiver) = network.register(...);
          |       let (engine, mailbox) = broadcast::buffered::Engine::new(ctx, broadcast_cfg);
          |       network.start();
          |       engine.start((sender, receiver));
          |       // Event loop: bridge mpsc commands to commonware
          |       loop {
          |           select! {
          |               cmd = command_rx.recv() => handle_command(cmd, &mailbox, &oracle),
          |               msg = broadcast_receiver.recv() => forward_to_aggregator(msg),
          |           }
          |       }
          |   });
```

**Why this works:**
- `mpsc::UnboundedSender` is `Send + Sync` and works across thread/runtime boundaries
- `crossbeam::channel::Sender` (for `AggregatorCommand`) is also cross-thread safe
- The boundary is clean: all commonware types stay inside the commonware runtime thread
- No need to implement custom `Spawner`/`Clock`/`Network` traits (complex, error-prone)

**Alternative (NOT recommended): Custom Runtime Traits**
Implement `Spawner`, `Clock`, `Network`, etc. for WAVS's existing Tokio context. This is technically possible (commonware is an "anti-framework" that supports this), but:
- Requires implementing 7+ traits with complex semantics
- Tightly couples WAVS to commonware's trait evolution
- Higher maintenance burden as commonware adds new required methods
- The dedicated-thread approach is simpler and achieves the same result

### Alternative (lower risk): Use `tokio::Handle::enter()` Guard

If the commonware `tokio::Runner` causes issues with two runtimes, investigate whether commonware's `Context` can be manually constructed using `tokio::Handle::current()` from the existing runtime. This would require reading commonware source more carefully.

**Confidence: LOW -- unverified, would need source code inspection**

---

## Data Flow (After Migration)

### Outbound (Broadcast Submission)

```
1. Aggregator.handle_broadcast()
2. P2pHandle.publish(&submission)
3. P2pCommand::Publish { service_id, submission }  --[mpsc channel]-->
4. Commonware Coordinator Task receives command
5. mailbox.broadcast(WavsMessage { service_id, submission })
6. Broadcast Engine disseminates to all connected peers
7. Peers receive, filter by service_id in their ServiceRouter
```

### Inbound (Receive Submission)

```
1. Broadcast Engine receives message from peer
2. Coordinator Task receives from broadcast receiver
3. ServiceRouter.should_accept(&msg) -- filter by subscribed services
4. If accepted: aggregator_tx.send(AggregatorCommand::Receive { submission, peer })
5. Aggregator processes submission (unchanged from current flow)
```

### Catch-up (Peer Reconnection)

```
1. Peer connects to network
2. Broadcast Engine's buffered cache automatically handles message retrieval
3. Peer can request messages by digest from any connected peer
4. No explicit catch-up protocol needed -- the buffered Engine handles this
```

This eliminates: `CatchUpRequest`, `CatchUpResponse`, `CatchUpCodec`, `request_catchup_from_peer()`, `handle_catchup_request()`, `handle_catchup_response()`, `stored_submissions`, `catchup_requested_peers`.

### Subscribe/Unsubscribe

```
1. AggregatorCommand::SubscribeService { service_id }
2. P2pHandle.subscribe(&service_id)
3. P2pCommand::Subscribe { service_id }  --[mpsc channel]-->
4. Coordinator Task: service_router.subscribe(service_id)
   (No network-level action needed -- filtering is local)
```

---

## Suggested Build Order

Dependencies flow top-down. Each layer can be tested independently before composing.

### Phase 1: Cryptographic Identity (no network needed)

**Build:** `ed25519_signer_from_mnemonic()` function
**Test:** Unit test that derives deterministic Ed25519 keys from known mnemonics
**Dependencies:** `commonware-cryptography` only
**Risk:** Low. Pure function, no async, no networking.

### Phase 2: P2P Network Skeleton (connections, no broadcast)

**Build:** `CommonwareP2pNetwork` wrapper that:
- Creates `discovery::Network` or `lookup::Network` based on config
- Sets up Oracle with authorized peer set
- Registers a single broadcast channel
- Runs inside `commonware::tokio::Runner`
- Bridges commands via mpsc channel

**Test:** Integration test: 2 nodes connect, verify peer discovery works
**Dependencies:** Phase 1 (Ed25519 identity), `commonware-p2p`, `commonware-runtime`
**Risk:** Medium. Runtime integration is the main unknown.

### Phase 3: Broadcast Integration (message dissemination)

**Build:** Wire `commonware_broadcast::buffered::Engine` to the P2P channel:
- Create Engine with appropriate config
- Start Engine with P2P sender/receiver
- Bridge Mailbox.broadcast() to P2pCommand::Publish
- Bridge Engine receiver to AggregatorCommand::Receive

**Test:** Integration test: 3 nodes, broadcast submission, verify all receive it
**Dependencies:** Phase 2 (working P2P connections)
**Risk:** Medium. Message serialization format, deduplication behavior.

### Phase 4: Service Routing and Full P2pHandle

**Build:** ServiceRouter + complete P2pHandle reimplementation:
- Subscribe/unsubscribe updates ServiceRouter filter
- GetStatus returns commonware peer state
- Pending publish retry logic (may be simpler with commonware's built-in buffering)

**Test:** Integration test: multiple services, verify topic isolation via filtering
**Dependencies:** Phase 3 (working broadcast)
**Risk:** Low. Application-level logic, well-understood.

### Phase 5: Config Migration and P2pStatus

**Build:** New `P2pConfig` format tailored to commonware:
- Replace `Local { listen_port }` with lookup-specific config
- Replace `Remote { listen_port, bootstrap_nodes }` with discovery-specific config
- Update P2pStatus to reflect commonware state (no GossipSub mesh, different peer info)

**Test:** Config parsing tests, status endpoint tests
**Dependencies:** Phase 4 (full P2pHandle)
**Risk:** Low. Configuration is well-defined.

### Phase 6: E2E Testing and libp2p Removal

**Build:** Run full e2e test suite, fix any issues, remove libp2p dependency
**Test:** `just test-wavs-e2e` passes
**Dependencies:** All previous phases
**Risk:** Medium. Integration issues may surface only in e2e.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Implementing Custom Runtime Traits
**What:** Implementing `Spawner`, `Clock`, `Network`, etc. to use WAVS's existing Tokio runtime.
**Why bad:** 7+ traits with complex semantics, tightly couples to commonware internals, high maintenance cost as traits evolve.
**Instead:** Run commonware in its own `tokio::Runner` thread and bridge via channels.

### Anti-Pattern 2: One Broadcast Engine Per Service
**What:** Creating a separate `commonware_broadcast::buffered::Engine` for each registered service.
**Why bad:** Each Engine needs its own P2P channel registration, and channels must be registered before `network.start()`. Services are registered dynamically after the node starts.
**Instead:** Use a single broadcast Engine for all services, with application-level routing in ServiceRouter.

### Anti-Pattern 3: Exposing commonware Types Outside P2P Module
**What:** Letting commonware's `PublicKey`, `Sender`, `Receiver` types leak into the Aggregator or Dispatcher.
**Why bad:** Couples the entire codebase to commonware. If commonware changes, the blast radius is huge.
**Instead:** Keep all commonware types inside `p2p.rs` (or a `p2p/` module). The `P2pHandle` facade isolates the rest of the codebase.

### Anti-Pattern 4: Trying to Preserve secp256k1 Identity
**What:** Wrapping secp256k1 keys to satisfy commonware's Ed25519 `Signer` trait.
**Why bad:** commonware expects Ed25519 natively. Wrapping adds complexity and may break authentication.
**Instead:** Clean break to Ed25519 for P2P identity. On-chain signing stays ECDSA (separate concern).

---

## Scalability Considerations

| Concern | Current (libp2p) | After (commonware) | Notes |
|---------|------------------|---------------------|-------|
| Per-service isolation | GossipSub topics (network-level) | ServiceRouter filter (application-level) | Acceptable at WAVS scale (<1000 operators). All operators receive all messages. |
| Message caching | Manual `stored_submissions` HashMap | Buffered Engine built-in cache | Engine handles eviction, per-peer queues, capacity limits |
| Catch-up on reconnect | Custom request/response protocol | Engine's digest-based retrieval | Simpler, built-in, no custom protocol code |
| Peer discovery | Kademlia DHT (decentralized) | Bootstrapper-based (semi-centralized) | Bootstrappers must be available. Multiple bootstrappers for redundancy. |
| NAT traversal | AutoNAT + Identify | Not built-in to commonware | May need external STUN/TURN or relay if operators are behind NAT. Flag for research. |

---

## Open Questions Requiring Phase-Specific Research

1. **Runtime integration:** Does running `commonware::tokio::Runner` in a separate `std::thread` work cleanly alongside WAVS's existing Tokio runtime? (Validate in Phase 2)

2. **NAT traversal:** commonware does not include AutoNAT or relay protocols. How do operators behind NAT connect? (Research in Phase 2)

3. **Dynamic peer sets:** The Oracle's `track()` method registers peer sets at indices. How does this work when operators join/leave dynamically? Does WAVS need to manage a monotonically increasing index? (Research in Phase 2)

4. **Message size limits:** commonware has configurable `max_message_size`. What is the typical size of a serialized `Submission`? Does it fit within reasonable limits? (Validate in Phase 3)

5. **Ed25519 seed derivation:** What is the best way to deterministically derive an Ed25519 seed from a BIP-39 mnemonic? (Validate in Phase 1)

---

## Sources

- [commonware-p2p docs.rs](https://docs.rs/commonware-p2p/latest/commonware_p2p/) -- HIGH confidence (official docs)
- [commonware-p2p authenticated::discovery](https://docs.rs/commonware-p2p/latest/commonware_p2p/authenticated/discovery/index.html) -- HIGH confidence
- [commonware-p2p authenticated::lookup](https://docs.rs/commonware-p2p/latest/commonware_p2p/authenticated/lookup/index.html) -- HIGH confidence
- [commonware-broadcast buffered](https://docs.rs/commonware-broadcast/latest/commonware_broadcast/buffered/index.html) -- HIGH confidence
- [commonware-cryptography](https://docs.rs/commonware-cryptography/latest/commonware_cryptography/) -- HIGH confidence
- [commonware-runtime tokio](https://docs.rs/commonware-runtime/latest/commonware_runtime/tokio/index.html) -- HIGH confidence
- [commonwarexyz/monorepo GitHub](https://github.com/commonwarexyz/monorepo) -- HIGH confidence
- [commonware chat example](https://github.com/commonwarexyz/monorepo/blob/main/examples/chat/README.md) -- MEDIUM confidence (example, not WAVS-specific)
- [WAVS source code: p2p.rs, aggregator.rs, dispatcher.rs](packages/wavs/src/subsystems/aggregator/) -- HIGH confidence (direct code reading)
