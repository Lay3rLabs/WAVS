# Phase 2: Broadcast and Routing - Research

**Researched:** 2026-03-17
**Domain:** commonware-broadcast buffered Engine, Codec/Digestible traits, application-level service routing, retry queue
**Confidence:** HIGH

## Summary

Phase 2 wires up message broadcasting and service-level routing on top of the Phase 1 networking foundation. The core mechanism is `commonware_broadcast::buffered::Engine`, which provides broadcast-to-all-peers, per-peer message caching (bounded deque), and digest-based retrieval -- replacing both GossipSub and the custom catch-up protocol in a single component. The Engine consumes the `(Sender, Receiver)` pair already registered via `network.register()` in Phase 1's bridge loops.

The Submission type (which already has serde Serialize/Deserialize) needs a wrapper type `P2pMessage` that implements commonware's `Codec` (Write + EncodeSize + Read) and `Digestible` traits. Service-level message filtering happens at the application layer: a `ServiceRouter` (a `HashSet<ServiceId>`) filters inbound messages, and the `service_id` field in each `P2pMessage` enables per-service isolation without multiple P2P channels (per the locked decision from Phase 1). Failed publishes (no connected peers) are retried from a bounded `VecDeque` when the Sender reports zero recipients.

The P2pHandle API (publish, subscribe, unsubscribe, get_status) remains unchanged from the Aggregator's perspective. Phase 2 fills in the stub handlers in the bridge loops (the `Some(_) => { tracing::debug!("P2pCommand not yet implemented") }` branches) and also receives messages from the broadcast Engine to forward as `AggregatorCommand::Receive`.

**Primary recommendation:** Build the `P2pMessage` wrapper with Codec/Digestible first (pure types, no networking), then integrate the broadcast Engine into the bridge loops, then wire up service routing and retry logic.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BCAST-01 | Operator can broadcast signed submission to all connected peers | `Mailbox::broadcast(Recipients::All, msg)` sends to all peers. Engine handles network-level dissemination. See Broadcast Integration section. |
| BCAST-02 | Messages are deduplicated by cryptographic digest | `Engine::insert_message()` deduplicates by `M::Digest` -- if a digest already exists in a peer's deque, it is a no-op. See Engine Internals section. |
| BCAST-03 | Submission type implements commonware Codec and Digestible traits | `P2pMessage` wrapper implements `Write`, `EncodeSize`, `Read` (for Codec) and `Digestible` (SHA-256 of serialized submission bytes). See Code Examples section. |
| BCAST-04 | Failed publishes (no peers) are retried with bounded queue | `Sender::send()` returns `Vec<PublicKey>` of successful recipients. Empty vec means no peers received it. Bounded `VecDeque<P2pMessage>` retries when next broadcast reports successful peers. See Retry Queue section. |
| BCAST-05 | Per-service message isolation via application-level service_id filtering on single channel | `ServiceRouter` (a `HashSet<ServiceId>`) checks `msg.service_id` on every inbound message. Only messages for subscribed services are forwarded to the Aggregator. See Service Routing section. |
| CATCH-01 | Reconnecting peer retrieves missed submissions via buffered Engine digest-based caching | Engine's `items: BTreeMap<Digest, M>` cache persists messages. `Mailbox::subscribe(digest)` retrieves cached messages instantly or waits for them. Reconnecting peers receive messages from cache via the Engine's internal relay. See Catch-Up section. |
| CATCH-02 | Message storage is bounded per peer (configurable deque_size) | `Config.deque_size` controls `VecDeque` size per peer. When full, oldest digest is evicted. See Engine Internals section. |
| INT-01 | P2pHandle API (publish, subscribe, unsubscribe, get_status) preserved -- Aggregator sees no changes | P2pHandle struct, P2pCommand enum, and all public methods (publish, subscribe, unsubscribe, get_status, block_peer) remain unchanged. Internal bridge loop implementations change. See P2pHandle Preservation section. |
</phase_requirements>

## Standard Stack

### Core (New for Phase 2)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `commonware-broadcast` | 2026.3.0 | Message dissemination + caching | Replaces GossipSub + custom catch-up protocol. The `buffered::Engine` handles broadcast, per-peer message caching, and digest-based retrieval in a single component. |
| `commonware-cryptography` (sha256) | 2026.3.0 | SHA-256 hashing for Digestible | `Sha256::hash()` produces `sha256::Digest` which satisfies the `Digest` trait required by `Digestible`. Already in dependency tree from Phase 1. |

### Existing (From Phase 1, Unchanged)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `commonware-p2p` | 2026.3.0 | Authenticated peer networking | Provides `Sender`/`Receiver` pairs consumed by broadcast Engine. Channel already registered in Phase 1. |
| `commonware-runtime` | 2026.3.0 | Async runtime abstraction | `BufferPooler`, `Clock`, `Spawner`, `Metrics` required by Engine. Provided by the existing Runner context. |
| `commonware-codec` | 2026.3.0 | Binary serialization traits | `Write`, `Read`, `EncodeSize` traits for `P2pMessage`. Already a direct dependency. |
| `serde` / `serde_json` | (workspace) | Submission serialization | Submission already implements Serialize/Deserialize. Use serde for encoding submission payload inside P2pMessage's Codec implementation. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| serde_json inside Codec Write/Read | `bincode` or `postcard` for binary encoding | serde_json is already in workspace, Submission already derives Serialize/Deserialize. Binary format would be smaller but adds a dependency. serde_json is simpler for a first pass; optimize later if message size is a concern. |
| `sha256::Digest` for Digestible | `blake3::Digest` | Blake3 is faster but sha256 is already used throughout WAVS for other hashing. Consistency > micro-optimization. |
| Application-level ServiceRouter | Multiple P2P channels per service | Channels must be registered before `network.start()`. Services are registered dynamically. Single channel + filter is the locked decision from Phase 1 planning. |

**Installation:**
```toml
# In packages/wavs/Cargo.toml [dependencies]
# ADD:
commonware-broadcast = "2026.3.0"
# serde_json already in workspace
# All other dependencies already present from Phase 1
```

**Version verification:** `commonware-broadcast` 2026.3.0 confirmed on crates.io (CalVer lockstep with other commonware crates).

## Architecture Patterns

### Recommended Changes to p2p.rs

```
packages/wavs/src/subsystems/aggregator/p2p.rs
  |-- [EXISTING] Identity section: ed25519_signer_from_mnemonic(), pubkey_from_mnemonic()
  |-- [EXISTING] Config section: P2pConfig enum (Disabled / Local / Remote)
  |-- [NEW] P2pMessage: wrapper around (ServiceId, Submission) with Codec + Digestible
  |-- [NEW] ServiceRouter: HashSet<ServiceId> for filtering inbound messages
  |-- [MODIFIED] P2pCommand enum: unchanged variants, but handlers now implemented
  |-- [MODIFIED] P2pHandle: unchanged public API, stores aggregator_tx for forwarding
  |-- [MODIFIED] run_lookup_network(): integrates broadcast Engine, handles all P2pCommands
  |-- [MODIFIED] run_discovery_network(): same Engine integration, same command handling
  |-- [NEW] RetryQueue: bounded VecDeque for failed publishes

packages/wavs/tests/
  |-- [EXISTING] p2p_identity_tests.rs
  |-- [EXISTING] p2p_connectivity_tests.rs
  |-- [NEW] p2p_broadcast_tests.rs: broadcast, service filtering, catch-up, retry
```

### Pattern 1: P2pMessage Wrapper with Codec + Digestible

**What:** A wrapper type that combines `ServiceId` and `Submission` into a single type that satisfies commonware's `Digestible + Codec` trait bounds for the broadcast Engine.

**When to use:** Every message sent/received over the P2P broadcast channel.

**Key insight:** The `Submission` type already derives `Serialize + Deserialize`. Rather than implementing Write/Read for the deeply nested Submission graph (which includes TriggerAction, Envelope, WavsSignature, etc.), serialize the entire Submission to JSON bytes and wrap that in a simple binary envelope with `service_id` prefix. This avoids touching 20+ types in `packages/types/`.

```rust
// Source: commonware-broadcast source code analysis + mocks.rs pattern
use commonware_codec::{EncodeSize, Error as CodecError, RangeCfg, Read, ReadRangeExt, Write};
use commonware_cryptography::{sha256, Digestible, Hasher, Sha256};
use commonware_runtime::{Buf, BufMut};

/// P2P message envelope for broadcast.
/// Wraps a ServiceId and serialized Submission for transmission over commonware-broadcast.
#[derive(Clone, Debug)]
pub struct P2pMessage {
    /// Service ID bytes (SHA-256 hash, 32 bytes)
    pub service_id_bytes: Vec<u8>,
    /// JSON-serialized Submission payload
    pub payload: Vec<u8>,
}

impl Digestible for P2pMessage {
    type Digest = sha256::Digest;
    fn digest(&self) -> sha256::Digest {
        // Hash the entire message for deduplication
        let mut data = Vec::with_capacity(self.service_id_bytes.len() + self.payload.len());
        data.extend_from_slice(&self.service_id_bytes);
        data.extend_from_slice(&self.payload);
        Sha256::hash(&data)
    }
}

impl Write for P2pMessage {
    fn write(&self, buf: &mut impl BufMut) {
        self.service_id_bytes.write(buf);
        self.payload.write(buf);
    }
}

impl EncodeSize for P2pMessage {
    fn encode_size(&self) -> usize {
        self.service_id_bytes.encode_size() + self.payload.encode_size()
    }
}

impl Read for P2pMessage {
    type Cfg = RangeCfg<usize>;

    fn read_cfg(buf: &mut impl Buf, range: &Self::Cfg) -> Result<Self, CodecError> {
        let service_id_bytes = Vec::<u8>::read_range(buf, *range)?;
        let payload = Vec::<u8>::read_range(buf, *range)?;
        Ok(Self {
            service_id_bytes,
            payload,
        })
    }
}
```

**Confidence:** HIGH -- pattern directly follows `TestMessage` in commonware-broadcast's own test mocks. The Codec traits are auto-derived from Write + EncodeSize + Read.

### Pattern 2: Broadcast Engine Integration

**What:** Wire `commonware_broadcast::buffered::Engine` into the existing bridge loop, consuming the `(Sender, Receiver)` pair from `network.register()`.

**When to use:** During network initialization, after channel registration and before the bridge loop starts.

```rust
use commonware_broadcast::buffered::{Config as BroadcastConfig, Engine, Mailbox};

// Inside run_lookup_network() or run_discovery_network():

// 1. Register channel (already done in Phase 1)
let (sender, receiver) = network.register(0u64, Quota::per_second(NZU32!(100)), 1024);

// 2. Create broadcast Engine
let broadcast_config = BroadcastConfig {
    public_key: own_pubkey.clone(),
    mailbox_size: 256,          // max pending broadcast requests
    deque_size: 128,            // max cached messages per peer (CATCH-02)
    priority: false,            // normal priority
    codec_config: RangeCfg::new(0..=65536), // max message field size
    peer_provider: oracle.clone(), // Oracle implements Provider trait
};
let (engine, mailbox) = Engine::new(context.clone(), broadcast_config);

// 3. Start the network (before engine -- engine needs network running)
let _net_handle = network.start();

// 4. Start the broadcast engine (consumes sender/receiver from channel)
let _engine_handle = engine.start((sender, receiver));

// 5. Run bridge loop with mailbox for broadcasting
// mailbox is Clone, so it can be used in the bridge loop
```

**Confidence:** HIGH -- verified from Engine source code. `Engine::new()` returns `(Engine, Mailbox)`. `Engine::start()` takes `(impl Sender, impl Receiver)` and returns `Handle<()>`.

### Pattern 3: Service Router (Application-Level Filtering)

**What:** A lightweight filter that determines whether inbound P2P messages should be forwarded to the Aggregator based on subscribed service IDs.

**When to use:** On every inbound message from the broadcast Engine.

```rust
use std::collections::HashSet;
use wavs_types::ServiceId;

struct ServiceRouter {
    subscribed_services: HashSet<Vec<u8>>, // ServiceId as bytes for comparison
}

impl ServiceRouter {
    fn new() -> Self {
        Self { subscribed_services: HashSet::new() }
    }

    fn subscribe(&mut self, service_id: &ServiceId) {
        self.subscribed_services.insert(service_id_to_bytes(service_id));
    }

    fn unsubscribe(&mut self, service_id: &ServiceId) {
        self.subscribed_services.remove(&service_id_to_bytes(service_id));
    }

    fn should_accept(&self, msg: &P2pMessage) -> bool {
        self.subscribed_services.contains(&msg.service_id_bytes)
    }

    fn subscribed_topics(&self) -> Vec<String> {
        self.subscribed_services.iter()
            .map(|s| const_hex::encode(s))
            .collect()
    }
}
```

**Confidence:** HIGH -- simple application-level logic. The existing Aggregator already validates service IDs on receive; this is an early filter to avoid unnecessary deserialization.

### Pattern 4: Retry Queue for Failed Publishes (BCAST-04)

**What:** When `Mailbox::broadcast()` reports zero recipients (no connected peers), queue the message for retry.

**When to use:** In the bridge loop's Publish command handler.

```rust
use std::collections::VecDeque;

const MAX_RETRY_QUEUE_SIZE: usize = 64;

struct RetryQueue {
    queue: VecDeque<P2pMessage>,
}

impl RetryQueue {
    fn new() -> Self {
        Self { queue: VecDeque::with_capacity(MAX_RETRY_QUEUE_SIZE) }
    }

    fn push(&mut self, msg: P2pMessage) {
        if self.queue.len() >= MAX_RETRY_QUEUE_SIZE {
            self.queue.pop_front(); // Drop oldest
            tracing::warn!("Retry queue full, dropping oldest message");
        }
        self.queue.push_back(msg);
    }

    fn drain_all(&mut self) -> Vec<P2pMessage> {
        self.queue.drain(..).collect()
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
```

**Key behavior:** After each successful broadcast (at least one recipient), attempt to flush the retry queue by re-broadcasting queued messages. This piggybacks on the knowledge that at least one peer is now reachable.

**Confidence:** HIGH -- straightforward bounded queue. The `Sender::send()` / `Mailbox::broadcast()` response tells us who received; empty means no peers.

### Pattern 5: Bridge Loop with Engine Integration

**What:** The modified bridge loop that handles all P2pCommands and also receives messages from the broadcast Engine.

**When to use:** Replaces the Phase 1 stub bridge loop in both `run_lookup_network()` and `run_discovery_network()`.

```rust
// Inside the bridge loop (after engine.start()):
let mut service_router = ServiceRouter::new();
let mut retry_queue = RetryQueue::new();

loop {
    tokio::select! {
        cmd = command_rx.recv() => {
            match cmd {
                Some(P2pCommand::Publish { service_id, submission }) => {
                    let msg = P2pMessage::from_submission(&service_id, &submission);
                    let receiver = mailbox.broadcast(Recipients::All, msg.clone()).await;
                    match receiver.recv().await {
                        Some(recipients) if recipients.is_empty() => {
                            // No peers received -- queue for retry (BCAST-04)
                            retry_queue.push(msg);
                            tracing::warn!("No peers available, queued for retry");
                        }
                        Some(recipients) => {
                            tracing::debug!("Broadcast to {} peers", recipients.len());
                            // Flush retry queue since we have peers
                            if !retry_queue.is_empty() {
                                let queued = retry_queue.drain_all();
                                for queued_msg in queued {
                                    let _ = mailbox.broadcast(Recipients::All, queued_msg).await;
                                }
                            }
                        }
                        None => {
                            // Engine shut down
                            tracing::error!("Broadcast engine shut down");
                        }
                    }
                }
                Some(P2pCommand::Subscribe { service_id }) => {
                    service_router.subscribe(&service_id);
                    tracing::info!("Subscribed to service: {}", service_id);
                }
                Some(P2pCommand::Unsubscribe { service_id }) => {
                    service_router.unsubscribe(&service_id);
                    tracing::info!("Unsubscribed from service: {}", service_id);
                }
                Some(P2pCommand::GetStatus { response_tx }) => {
                    let status = P2pStatus {
                        enabled: true,
                        local_peer_id: Some(const_hex::encode(own_pubkey.as_ref())),
                        listen_addresses: vec![listen_addr.to_string()],
                        external_addresses: vec![],
                        connected_peers: 0, // Phase 3: query from network handle
                        peer_ids: vec![],
                        subscribed_topics: service_router.subscribed_topics(),
                        topic_peer_counts: Default::default(),
                    };
                    let _ = response_tx.send(status);
                }
                Some(P2pCommand::BlockPeer { pubkey_hex }) => {
                    // Same as Phase 1
                    match parse_authorized_peers(&[pubkey_hex.clone()]) {
                        Ok(keys) if !keys.is_empty() => {
                            oracle.block(keys[0].clone()).await;
                            tracing::info!("Blocked peer: {}", pubkey_hex);
                        }
                        _ => {
                            tracing::error!("Failed to parse pubkey: {}", pubkey_hex);
                        }
                    }
                }
                None => {
                    tracing::info!("P2P command channel closed, shutting down");
                    context.stop(0, None).await.ok();
                    break;
                }
            }
        }
        // NOTE: Messages from the broadcast Engine are handled internally
        // by the Engine's own run loop. The Engine receives from the P2P
        // Receiver and caches messages. To get inbound messages to the
        // Aggregator, we need a separate mechanism. See "Inbound Message
        // Forwarding" section below.
    }
}
```

**Confidence:** MEDIUM -- the bridge loop structure is proven from Phase 1, but the interaction between Engine's internal message handling and forwarding to the Aggregator needs careful design (see Open Questions).

### Anti-Patterns to Avoid

- **Implementing Codec for the entire Submission type graph:** Submission contains TriggerAction, Envelope, WavsSignature, ServiceManager, and 15+ other types. Implementing Write/Read for all of them is massive scope creep. Serialize with serde_json and wrap in P2pMessage instead.
- **Creating the Engine before `network.start()`:** The Engine's `start()` method needs the Sender/Receiver from the channel, which are valid after registration. However, the Engine itself does not need the network to be started first -- it uses the Sender/Receiver directly. The sequence should be: register channel -> create Engine -> start network -> start Engine.
- **Blocking on `broadcast().recv()` inside the bridge loop:** The broadcast response is async. Use `await` carefully or spawn the response handling to avoid blocking other command processing.
- **Storing `ServiceId` directly in ServiceRouter:** ServiceId is a macro-generated hash type that may not implement Hash for HashSet. Use the raw bytes representation.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Message broadcast to all peers | Custom peer iteration + send | `Mailbox::broadcast(Recipients::All, msg)` | Engine handles network delivery, retries at Sender level, and caching |
| Message deduplication | Custom seen-message set | Engine's built-in `items: BTreeMap<Digest, M>` + per-peer deque | Automatically deduplicates by digest; ref-counted across peers |
| Per-peer message caching | Custom `HashMap<PeerId, Vec<Message>>` | Engine's `deques: BTreeMap<P, VecDeque<Digest>>` | Bounded, eviction-aware, tied to Oracle peer set changes |
| Catch-up on reconnect | Custom request/response protocol | `Mailbox::subscribe(digest)` / `Mailbox::get(digest)` | Engine returns cached message instantly or waits for network delivery |
| Binary serialization | Custom byte packing | commonware-codec `Write`/`Read` traits | Consistent with all commonware types; auto-derives Codec when Write + EncodeSize + Read are implemented |
| Message digest computation | Custom hashing | `Sha256::hash()` -> `sha256::Digest` which implements commonware `Digest` trait | Consistent with commonware ecosystem |

**Key insight:** The broadcast Engine replaces three major components from the old libp2p implementation: GossipSub (broadcast), `stored_submissions` (caching), and the CatchUp request/response protocol (replay). This is the biggest simplification in the entire migration.

## Common Pitfalls

### Pitfall 1: Engine Message Flow is One-Way to Cache (CRITICAL)
**What goes wrong:** The broadcast Engine receives messages from the P2P Receiver and caches them internally, but does NOT automatically forward them to the application. There is no callback or channel from Engine to application for inbound messages.
**Why it happens:** The Engine is designed as a cache-and-relay system. Applications retrieve messages by calling `mailbox.subscribe(digest)` or `mailbox.get(digest)` -- they must know the digest to ask for.
**How to avoid:** Two approaches: (a) Have the bridge loop also read from a **separate P2P channel** dedicated to direct application delivery (not the Engine's channel), or (b) Use the Oracle's `Provider::subscribe()` method which fires on peer set changes, combined with periodic `mailbox.get()` calls. However, the cleanest approach for WAVS is: do NOT use the broadcast Engine for inbound message delivery. Instead, use the raw `Sender`/`Receiver` directly in the bridge loop for the broadcast-and-receive flow, and use the Engine only for catch-up. **OR**: Register two channels -- one for the Engine (for caching/catch-up) and one for direct application messaging (for real-time inbound). **Recommended**: Use the Mailbox for outbound broadcast (which also caches locally) and monitor the `Receiver` directly for inbound messages (bypassing the Engine's internal receive). This requires registering the channel and passing only the Sender to the Engine while keeping the Receiver in the bridge loop. **UPDATE after source analysis:** The Engine's `start()` takes BOTH sender and receiver. The Engine's `run()` loop internally processes received messages via `receiver.recv()` and stores them. The application retrieves them via `Mailbox::subscribe(digest)` or `Mailbox::get(digest)`. For real-time forwarding, the approach is to **not use the Engine** and instead handle send/receive directly. OR, register TWO channels and use one for Engine and one for direct messaging.
**Warning signs:** Messages are sent but never arrive at the Aggregator. Broadcast works outbound but inbound is silent.

### Pitfall 2: Channel Registration Must Happen Before network.start()
**What goes wrong:** Attempting to register a second channel after `network.start()` for the two-channel approach.
**Why it happens:** `network.start()` consumes `self` (moves ownership). No further `register()` calls are possible.
**How to avoid:** Register ALL needed channels before calling `network.start()`. If using two channels (one for Engine, one for direct messaging), register both upfront during initialization.
**Warning signs:** Compile error -- "use of moved value: `network`".

### Pitfall 3: Codec RangeCfg Must Match Max Message Size
**What goes wrong:** Deserialization fails because the `RangeCfg` upper bound in `Read::Cfg` is too small for the actual message payload.
**Why it happens:** The `Read::read_range()` method validates the length of variable-length fields against the `RangeCfg`. If the serialized Submission exceeds the configured range, it returns `CodecError`.
**How to avoid:** Set the `codec_config` in `BroadcastConfig` to match or exceed the `max_message_size` (currently 65536 bytes). Verify that serialized Submissions fit within this limit.
**Warning signs:** `failed to decode message` errors in logs. Messages broadcast successfully but never decoded on the receiving side.

### Pitfall 4: Oracle Clone for peer_provider
**What goes wrong:** The `BroadcastConfig.peer_provider` requires a type implementing `Provider<PublicKey = P>`. The Oracle implements this, but it needs to be cloned or shared.
**Why it happens:** Both the bridge loop (for `oracle.block()`) and the Engine (for peer set tracking) need access to the Oracle.
**How to avoid:** The Oracle from `lookup::Network::new()` / `discovery::Network::new()` likely implements `Clone`. Verify this. If not, use an Arc wrapper.
**Warning signs:** Borrow checker errors when trying to move Oracle into both the Config and keep it for the bridge loop.

### Pitfall 5: Mailbox::broadcast Returns Async Receiver
**What goes wrong:** Calling `mailbox.broadcast()` and expecting it to block until delivery. It returns `oneshot::Receiver<Vec<P>>` that must be `.await`ed or `.recv()`ed to get the result.
**Why it happens:** Broadcast is non-blocking by design -- the Engine processes it asynchronously.
**How to avoid:** `.await` the receiver to get the list of peers who received the message. Handle the `None` case (Engine shut down) and the empty-list case (no peers available).
**Warning signs:** Retry queue never fills because the response is never checked.

### Pitfall 6: ServiceId Bytes Representation
**What goes wrong:** The `ServiceId` type is a macro-generated hash ID type (via `new_hash_id_type!`). It may not directly expose its raw bytes in a way compatible with the P2pMessage's `service_id_bytes` field.
**Why it happens:** `ServiceId` is defined with `new_hash_id_type!(ServiceId, true)` which generates a wrapper around `[u8; 32]` (SHA-256 hash). The inner bytes access pattern needs investigation.
**How to avoid:** Check how ServiceId serializes. Since it derives `Serialize` with serde, `serde_json::to_vec(&service_id)` works. Or if it implements `AsRef<[u8]>` or `Display`, use that. The bytes representation must be consistent between sender and receiver for ServiceRouter filtering.
**Warning signs:** ServiceRouter never matches because sender and receiver use different byte representations for the same ServiceId.

## Code Examples

### P2pMessage Construction from Submission

```rust
impl P2pMessage {
    fn from_submission(
        service_id: &ServiceId,
        submission: &Submission,
    ) -> Result<Self, AggregatorError> {
        let service_id_bytes = serde_json::to_vec(service_id)
            .map_err(|e| AggregatorError::P2p(format!("Failed to serialize service_id: {}", e)))?;
        let payload = serde_json::to_vec(submission)
            .map_err(|e| AggregatorError::P2p(format!("Failed to serialize submission: {}", e)))?;
        Ok(Self { service_id_bytes, payload })
    }

    fn to_submission(&self) -> Result<(ServiceId, Submission), AggregatorError> {
        let service_id: ServiceId = serde_json::from_slice(&self.service_id_bytes)
            .map_err(|e| AggregatorError::P2p(format!("Failed to deserialize service_id: {}", e)))?;
        let submission: Submission = serde_json::from_slice(&self.payload)
            .map_err(|e| AggregatorError::P2p(format!("Failed to deserialize submission: {}", e)))?;
        Ok((service_id, submission))
    }
}
```

### Broadcast Engine Setup (Inside Bridge Loop Init)

```rust
use commonware_broadcast::buffered::{Config as BroadcastConfig, Engine, Mailbox};
use commonware_codec::RangeCfg;

// After network and oracle are created, before network.start():

// Register TWO channels:
// Channel 0: for broadcast Engine (handles caching, digest retrieval)
let (engine_sender, engine_receiver) = network.register(
    0u64,
    Quota::per_second(NZU32!(100)),
    1024,
);

// Channel 1: for direct message forwarding to Aggregator
let (direct_sender, mut direct_receiver) = network.register(
    1u64,
    Quota::per_second(NZU32!(100)),
    1024,
);

let broadcast_config = BroadcastConfig {
    public_key: own_pubkey.clone(),
    mailbox_size: 256,
    deque_size: 128,                        // CATCH-02
    priority: false,
    codec_config: RangeCfg::new(0..=65536), // max field size for Read
    peer_provider: oracle.clone(),
};

let (engine, mailbox) = Engine::<_, _, P2pMessage, _>::new(
    context.clone(),
    broadcast_config,
);

let _net_handle = network.start();
let _engine_handle = engine.start((engine_sender, engine_receiver));
```

### Inbound Message Forwarding via Direct Channel

```rust
// In the bridge loop's select!:
msg = direct_receiver.recv() => {
    let (peer, raw_msg) = match msg {
        Ok(r) => r,
        Err(err) => {
            tracing::error!("Direct receiver error: {:?}", err);
            break;
        }
    };

    // Decode the P2pMessage from raw bytes
    let p2p_msg: P2pMessage = match commonware_codec::Decode::decode_cfg(
        raw_msg, &RangeCfg::new(0..=65536)
    ) {
        Ok(msg) => msg,
        Err(err) => {
            tracing::warn!("Failed to decode P2P message from {:?}: {:?}", peer, err);
            continue;
        }
    };

    // Service filtering (BCAST-05)
    if !service_router.should_accept(&p2p_msg) {
        tracing::debug!("Filtered message for unsubscribed service");
        continue;
    }

    // Deserialize submission
    match p2p_msg.to_submission() {
        Ok((_service_id, submission)) => {
            let peer_id = const_hex::encode(peer.as_ref());
            aggregator_tx.send(AggregatorCommand::Receive {
                submission,
                peer: Peer::Other(peer_id),
            }).ok();
        }
        Err(err) => {
            tracing::warn!("Failed to deserialize submission: {:?}", err);
        }
    }
}
```

## Engine Internals (from Source Code Analysis)

### How Deduplication Works (BCAST-02)

The Engine maintains three data structures:
1. `items: BTreeMap<Digest, M>` -- all cached messages by digest (global)
2. `deques: BTreeMap<P, VecDeque<Digest>>` -- per-peer LRU cache of recent digests
3. `counts: BTreeMap<Digest, usize>` -- reference count of each digest across peer deques

When `insert_message(peer, msg)` is called:
- Compute `digest = msg.digest()`
- If digest already in peer's deque: move to front (LRU update), return false (duplicate)
- If new: push to front of deque, increment count, store message if count becomes 1
- If deque exceeds `deque_size`: pop oldest, decrement count, remove message if count becomes 0

This means: messages are deduplicated globally by digest, but tracked per-peer for eviction. A message stays cached as long as ANY peer's deque references it.

### How Catch-Up Works (CATCH-01)

When a peer reconnects:
- The reconnecting peer's application knows which digests it needs (from its own state)
- It calls `mailbox.subscribe(digest)` for each needed message
- If the digest is in `items` cache: returned immediately
- If not: a Waiter is registered, and the message is delivered when received from the network

**Important nuance for WAVS:** The catch-up mechanism is pull-based, not push-based. The reconnecting peer must know which digests to request. In the previous libp2p implementation, catch-up was push-based (on connection, request all recent messages for a service). With commonware-broadcast, the pattern shifts:
- On reconnect, the node can subscribe to known digests from its quorum queue state
- The broadcast Engine automatically caches messages from all connected peers
- New messages after reconnection are received normally through the broadcast channel

### Peer Eviction

When `Provider::subscribe()` fires with updated tracked peers, the Engine calls `evict_untracked_peers()`:
- For each peer NOT in the new tracked set, remove their deque
- Decrement reference counts for all their digests
- Remove messages that have zero references

This ties cache management to Oracle peer set changes -- when a peer is blocked or removed, their cached messages are eventually cleaned up.

## Inbound Message Strategy (CRITICAL DESIGN DECISION)

After reading the Engine source code, there are three viable approaches for getting inbound messages to the Aggregator:

### Option A: Two Channels (Recommended)
Register two P2P channels:
- **Channel 0**: Consumed by the broadcast Engine (for caching + catch-up)
- **Channel 1**: Read directly in the bridge loop (for real-time forwarding to Aggregator)

On outbound: broadcast on BOTH channels (Engine handles channel 0 caching; direct send on channel 1 for all peers).
On inbound: Channel 1 delivers to bridge loop which forwards to Aggregator via `aggregator_tx`.

**Pros:** Clean separation. Engine handles cache/catch-up. Bridge loop handles real-time forwarding. No polling.
**Cons:** Two network sends per broadcast. Slightly higher bandwidth.

### Option B: Single Channel + Periodic Polling
Use a single channel consumed by the Engine. Periodically poll `mailbox.get(digest)` for known digests.

**Pros:** Simple setup.
**Cons:** Requires knowing digests upfront. Does not support real-time message delivery. Unsuitable for WAVS where messages arrive unpredictably.

### Option C: Skip Engine, Use Raw Sender/Receiver
Do not use commonware-broadcast at all. Use `Sender::send()` and `Receiver::recv()` directly in the bridge loop. Implement caching manually.

**Pros:** Full control. Real-time delivery. No Engine complexity.
**Cons:** Must hand-roll caching, deduplication, and catch-up. Loses all Engine benefits. Goes against the architecture decision to use commonware-broadcast.

**Recommendation: Option A (Two Channels).** This gives both real-time inbound delivery and Engine-based caching for catch-up. The dual-send overhead is negligible for WAVS's message rates (tens of messages per second at most).

**Alternative Recommendation: Option C (Skip Engine, use raw Sender/Receiver).** If the two-channel approach proves too complex, the raw Sender/Receiver approach is simpler and still meets all requirements. The retry queue and ServiceRouter handle BCAST-04 and BCAST-05. For CATCH-01/CATCH-02, implement a simple bounded `HashMap<Digest, P2pMessage>` for caching -- much simpler than the Engine but less feature-complete. This may be the pragmatic choice given WAVS's scale.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| GossipSub per-service topics | Single broadcast channel + ServiceRouter filter | This phase | All operators receive all messages; filtering is local. Acceptable at WAVS scale. |
| Custom CatchUp protocol (request/response) | Engine digest-based cache OR simple bounded HashMap | This phase | Eliminates ~300 lines of CatchUpRequest/Response/Codec code. |
| Manual `stored_submissions` HashMap | Engine's `items: BTreeMap<Digest, M>` | This phase | Built-in eviction, per-peer tracking, reference counting. |
| `PendingPublish` struct with retry timer | `RetryQueue` (bounded VecDeque) + piggyback on successful broadcasts | This phase | Simpler retry: no timer, just queue and flush when peers appear. |

## Open Questions

1. **Oracle implements Clone and Provider?**
   - What we know: `BroadcastConfig.peer_provider` requires `Provider<PublicKey = P>`. Both `lookup::Oracle` and `discovery::Oracle` are listed as implementing `Provider`. Clone is likely supported.
   - What is unclear: Whether `Oracle::clone()` gives a shallow reference (shared state) or deep copy.
   - Recommendation: Test in code. If Oracle does not Clone, wrap in Arc. The Engine needs its own reference.

2. **Two-Channel vs Single-Channel Approach**
   - What we know: `network.register()` can be called multiple times before `start()`, each with a different channel ID (u64). Each returns independent `(Sender, Receiver)` pairs.
   - What is unclear: Whether sending the same message on two channels doubles bandwidth or if the underlying transport deduplicates.
   - Recommendation: Start with Option A (two channels). If it causes issues, fall back to Option C (skip Engine, raw Sender/Receiver).

3. **ServiceId Byte Representation**
   - What we know: ServiceId is `new_hash_id_type!(ServiceId, true)` which wraps `[u8; 32]`.
   - What is unclear: Exact `AsRef<[u8]>` or Display implementation. Need to verify how to extract raw bytes for P2pMessage and ServiceRouter.
   - Recommendation: Check `ServiceId`'s trait implementations at plan time. If it has `AsRef<[u8]>`, use that. Otherwise serialize with serde.

4. **commonware oneshot Channel Compatibility**
   - What we know: The Engine uses `commonware_utils::channel::oneshot`, not `tokio::sync::oneshot`.
   - What is unclear: Whether `commonware_utils::channel::oneshot::Receiver` is compatible with `tokio::select!`.
   - Recommendation: The Engine's internal select loop uses `commonware_macros::select_loop!`. The bridge loop uses `tokio::select!`. These are in different runtimes (commonware's Runner wraps Tokio). The Mailbox's async methods should work from within the commonware runtime context.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework + `cargo test` |
| Config file | None -- uses `#[tokio::test(flavor = "multi_thread")]` |
| Quick run command | `cargo test -p wavs --test p2p_broadcast_tests -- --nocapture` |
| Full suite command | `cargo test -p wavs -- p2p_ --nocapture` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BCAST-01 | Broadcast submission delivered to all peers | integration | `cargo test -p wavs --test p2p_broadcast_tests::test_broadcast_to_all_peers` | No -- Wave 0 |
| BCAST-02 | Duplicate messages deduplicated by digest | integration | `cargo test -p wavs --test p2p_broadcast_tests::test_deduplication_by_digest` | No -- Wave 0 |
| BCAST-03 | P2pMessage implements Codec + Digestible | unit | `cargo test -p wavs --test p2p_broadcast_tests::test_codec_roundtrip` | No -- Wave 0 |
| BCAST-04 | Failed publishes retried from bounded queue | integration | `cargo test -p wavs --test p2p_broadcast_tests::test_retry_queue_on_no_peers` | No -- Wave 0 |
| BCAST-05 | Service filtering delivers only subscribed | integration | `cargo test -p wavs --test p2p_broadcast_tests::test_service_filtering` | No -- Wave 0 |
| CATCH-01 | Reconnecting peer retrieves cached messages | integration | `cargo test -p wavs --test p2p_broadcast_tests::test_catchup_after_reconnect` | No -- Wave 0 |
| CATCH-02 | Message cache bounded by deque_size | unit | `cargo test -p wavs --test p2p_broadcast_tests::test_cache_bounded_deque_size` | No -- Wave 0 |
| INT-01 | P2pHandle API unchanged from Aggregator view | integration | `cargo test -p wavs --test p2p_broadcast_tests::test_p2p_handle_api_preserved` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p wavs -- p2p_ --nocapture`
- **Per wave merge:** `cargo test -p wavs -- --nocapture`
- **Phase gate:** All p2p_ tests green before proceeding to Phase 3

### Wave 0 Gaps
- [ ] `packages/wavs/tests/p2p_broadcast_tests.rs` -- covers all BCAST-*, CATCH-*, INT-01
- [ ] `commonware-broadcast` dependency added to `packages/wavs/Cargo.toml`
- [ ] Test helpers: mock Submission creation for P2pMessage round-trip tests
- [ ] Test helpers: two-node or three-node setup with broadcast verification (can extend existing test patterns from p2p_connectivity_tests.rs)

## Sources

### Primary (HIGH confidence)
- [commonware-broadcast 2026.3.0 source code](~/.cargo/registry/src/index.crates.io-*/commonware-broadcast-2026.3.0/) -- Direct source code analysis of Engine, Mailbox, Config structs. Verified: Engine::new(), Engine::start(), Mailbox::broadcast(), insert_message() deduplication, deque eviction, peer eviction.
- [commonware-codec 2026.3.0 source code](~/.cargo/registry/src/index.crates.io-*/commonware-codec-2026.3.0/) -- Write, Read, EncodeSize, Codec traits and auto-implementations. Verified: Codec = Encode + Decode = (Write + EncodeSize) + Read.
- [commonware-broadcast buffered::mocks](source) -- TestMessage pattern for implementing Digestible + Codec. Verified: sha256::Digest as Digestible::Digest, RangeCfg as Read::Cfg, Vec<u8> fields with read_range().
- [docs.rs commonware-broadcast](https://docs.rs/commonware-broadcast/2026.3.0/) -- Config fields, Engine/Mailbox public API
- [docs.rs commonware-p2p](https://docs.rs/commonware-p2p/2026.3.0/) -- Sender, Receiver, Recipients, Provider traits
- [docs.rs commonware-cryptography](https://docs.rs/commonware-cryptography/2026.3.0/) -- Digestible trait, sha256::Digest type
- WAVS source code: `p2p.rs`, `aggregator.rs`, `submission.rs`, `http.rs` -- direct code inspection on branch `commonware`

### Secondary (MEDIUM confidence)
- [commonware chat example](https://docs.rs/crate/commonware-chat/2026.3.0/) -- Usage pattern for network.register() + sender/receiver + broadcast
- `.planning/research/STACK.md` -- Phase 1 research: channel registration pattern, single-channel decision
- `.planning/research/ARCHITECTURE.md` -- Data flow diagrams, component boundaries

### Tertiary (LOW confidence)
- Oracle Clone behavior -- inferred from API design (Provider requires Clone), not verified by source inspection
- Two-channel bandwidth impact -- architectural reasoning, not measured
- ServiceId byte representation -- inferred from `new_hash_id_type!` macro, not traced through expansion

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- commonware-broadcast version verified, API confirmed from source code
- Architecture: HIGH for Codec/Digestible implementation (follows official test mocks pattern), MEDIUM for two-channel vs single-channel decision (needs prototype validation)
- Pitfalls: HIGH -- identified from direct source code reading of Engine internals

**Research date:** 2026-03-17
**Valid until:** 2026-04-17 (commonware CalVer monthly releases; API may change)
