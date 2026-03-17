# Feature Landscape

**Domain:** P2P networking layer migration (libp2p to commonware) for WAVS aggregator
**Researched:** 2026-03-17

## Table Stakes

Features users expect. Missing = migration breaks existing functionality.

### Message Broadcast (replaces GossipSub)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Per-service message broadcast | Operators must broadcast signed submissions to peers for quorum aggregation. Currently GossipSub with topics like `wavs/{service_id}/topic/v1`. Without this, multi-operator consensus is impossible. | Medium | `commonware-broadcast::buffered::Engine` replaces GossipSub. Messages are broadcast to `Recipients::All` on a registered channel. The broadcast engine handles caching and delivery. |
| Message deduplication | Current GossipSub deduplicates by content hash + source + topic. Without this, aggregator receives duplicate submissions, corrupting quorum counts. | Low | `commonware-broadcast::buffered` maintains per-peer bounded queues and deduplicates by message digest (`Digestible` trait). Built-in, not custom. |
| Message serialization | Submissions are currently JSON-serialized for GossipSub. Must encode/decode `Submission` structs over the wire. | Low | Commonware uses `commonware-codec::Codec` trait. WAVS `Submission` type needs to implement `Codec` and `Digestible`. May switch from JSON to binary encoding for efficiency. |
| Publish retry on no peers | Current system queues failed publishes (no subscribers on topic) with bounded retry. Without this, submissions are lost during mesh formation. | Medium | `commonware-broadcast::buffered::Engine` caches messages and retries delivery inherently. The bounded message queue (`deque_size` config) replaces the custom `PendingPublish` retry queue. |

### Peer Discovery (replaces Kademlia + mDNS)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Production peer discovery | Remote mode currently uses Kademlia DHT with bootstrap nodes. Operators must find each other across the internet. | Medium | `commonware-p2p::authenticated::discovery` with bootstrapper-based gossip. Peers exchange bit vectors of known addresses, gossiping unknown peer info. Replaces Kademlia DHT entirely. |
| Local development discovery | Local mode uses mDNS for zero-config peer discovery on LAN. Essential for `just start-dev` workflow. | Medium | No mDNS equivalent in commonware. Two options: (1) use `commonware-p2p::authenticated::lookup` with known addresses for dev, or (2) run a local bootstrapper. Lookup mode (addresses known upfront) is simpler for dev. |
| Bootstrap node support | Remote mode requires bootstrap node addresses for initial network entry. One node runs as bootstrap server. | Low | `commonware-p2p::authenticated::discovery::Config::bootstrappers` field accepts `Vec<Bootstrapper>` (public key + socket address pairs). Direct equivalent to current bootstrap_nodes config. |
| Periodic peer re-discovery | Current Kademlia runs `get_closest_peers` every 60s to find late joiners. Must handle peers joining after initial bootstrap. | Low | `commonware-p2p::authenticated::discovery` has `gossip_bit_vec_frequency` and `query_frequency` config options that drive continuous peer exchange. Automatic, not manual polling. |
| Reconnection to bootstrap on disconnect | When all peers drop, current system re-dials bootstrap nodes and retries Kademlia bootstrap. | Low | Commonware discovery handles this via `dial_frequency` config. Automatic reconnection to bootstrappers when peers are lost. |

### Peer Authentication & Identity (replaces libp2p identity)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Deterministic P2P identity from mnemonic | Current system derives secp256k1 keypair from `WAVS_SIGNING_MNEMONIC` at HD path m/44'/60'/0'/0/0. Peer ID is consistent across restarts. | Medium | Commonware uses Ed25519 keys (via `commonware-cryptography::ed25519`). Must derive Ed25519 key deterministically from the same mnemonic. The derivation path changes but the property (consistent identity from mnemonic) must be preserved. |
| Encrypted peer connections | libp2p uses Noise protocol over TCP with yamux multiplexing. All P2P traffic is encrypted. | Low | Commonware-p2p authenticated modules provide encrypted connections natively. TLS-like handshake with public key authentication. No separate Noise/yamux configuration needed. |
| Message authentication | GossipSub uses `MessageAuthenticity::Signed` - messages signed by sender's key. | Low | Commonware authenticated network validates peer identity on connection. Messages are from authenticated peers by definition. Channel-level authentication is implicit. |

### Catch-Up Protocol (replaces Request/Response)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Missed message retrieval on reconnect | When a peer reconnects, it requests recent submissions it may have missed. Critical for operators that restart or have network interruptions. | Medium | `commonware-broadcast::buffered::Mailbox::subscribe(digest)` retrieves specific messages by digest, returning cached content or waiting for network delivery. The `Engine` maintains bounded per-peer message queues. This replaces the custom `CatchUpRequest`/`CatchUpResponse` protocol entirely. |
| Bounded message storage for catch-up | Current system stores up to 500 submissions per service with 5-minute TTL for catch-up responses. Must not grow unbounded. | Low | `commonware-broadcast::buffered::Config::deque_size` sets max cached items per sender. Built-in bounded queue replaces custom `StoredSubmission` with TTL. |
| Rate-limited catch-up requests | Current system limits to 3 concurrent catch-up requests per service to prevent overwhelming peers. | Low | Commonware's rate limiting is built into the p2p layer: `allowed_connection_rate_per_peer` and per-channel rate quotas handle this. No custom rate limiting needed. |

### Topic/Channel Isolation

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Per-service message isolation | Operators only receive messages for services they're subscribed to. Current GossipSub topics enforce this. | High | Commonware uses "channels" (identified by `Channel` alias, a `u32`) instead of string topics. Must map service IDs to channel IDs. **Key architectural difference**: commonware channels are registered at network startup via `network.register()` and cannot be added after `network.start()`. This conflicts with WAVS's dynamic service subscription model where services are added/removed at runtime. |
| Dynamic subscription/unsubscription | Services can be added and removed at runtime. P2P must subscribe/unsubscribe to per-service topics dynamically. | High | **This is the hardest mapping problem.** Commonware channels are static (registered before `network.start()`). Options: (1) use a single broadcast channel for all services and filter by service ID in application code, (2) use peer set updates via Oracle to simulate topic filtering, (3) pre-register a pool of channels. Option 1 is simplest and recommended. |

### P2P Status Reporting

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| `/p2p/status` endpoint | Exposes peer ID, listen addresses, connected peers, subscribed topics, per-topic peer counts. Used for debugging and monitoring. | Low | Must reconstruct from commonware primitives. Peer count available from Oracle/Provider. Listen address is from config. Channel info is known at registration time. |
| External address discovery | AutoNAT + Identify discover the node's external address for NAT traversal. Reported in status endpoint. | Medium | **Commonware has no AutoNAT equivalent.** The `discovery` module's `dialable` config field requires the operator to specify their own dialable address. See Pitfalls section. |

### Configuration

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Disabled/Local/Remote modes | P2P can be disabled (single operator), local (mDNS dev), or remote (Kademlia prod). | Low | Maps to: Disabled (no network created), Local (use `lookup` with known addresses or local bootstrapper), Remote (use `discovery` with bootstrappers). Clean config break is already planned. |
| Configurable listen port | Operators set their P2P listen port. Current default: 9000 with fallback to OS-assigned. | Low | `discovery::Config::listen` accepts socket address. Direct equivalent. |
| Configurable timeouts and intervals | Retry durations, cleanup intervals, TTLs, queue sizes. | Low | Map to commonware Config fields: `deque_size`, `gossip_bit_vec_frequency`, `dial_frequency`, `handshake_timeout`, rate limiting quotas, etc. |

## Differentiators

Features that commonware enables that libp2p does not provide. Not expected, but valued.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Built-in peer set management via Oracle | Commonware's Oracle lets you programmatically register/deregister authorized peers with `track()` at monotonically increasing indices. This maps naturally to on-chain operator registries -- when operators register/deregister on-chain, the Oracle can be updated to match. Currently WAVS has no concept of "authorized peers" at the P2P layer. | Medium | `Oracle::track()` accepts a new peer set at each index. Could integrate with on-chain service registry to only accept P2P connections from registered operators. Significant security improvement over current open GossipSub. |
| Digest-based message retrieval | `Mailbox::subscribe(digest)` lets you request a specific message by its cryptographic digest. If cached, returns immediately; if not, waits for network delivery. This enables precise "I need exactly this submission" requests instead of "give me everything you have." | Low | Replaces the coarse-grained catch-up protocol with fine-grained per-message retrieval. More efficient for targeted recovery. |
| Built-in per-peer rate limiting | Commonware p2p has sophisticated rate limiting at the connection, handshake, IP, and subnet levels. Currently WAVS has basic concurrent-request limiting but no IP-level or subnet-level protection. | Low | Config fields: `allowed_handshake_rate_per_ip`, `allowed_handshake_rate_per_subnet`, `allowed_connection_rate_per_peer`. Free DoS protection. |
| Peer blocking by identity | `block!` macro and `Blocker` trait allow blocking misbehaving peers by their cryptographic identity. Currently WAVS has no peer blocking capability. | Low | If a peer sends malformed submissions or exceeds rate limits, WAVS can block them by public key. Blocked peers cannot reconnect from any IP address. |
| Simulated networking for tests | `commonware-p2p::simulated` provides deterministic network simulation with configurable drops, latency, and corruption. Current P2P testing requires running actual libp2p nodes. | Medium | `commonware-runtime::deterministic` + `commonware-p2p::simulated` enables unit testing P2P logic without real network connections. Could significantly improve test speed and reliability. |
| Prometheus metrics built-in | Commonware Engine types require `Metrics` trait from runtime, indicating built-in observability. Current WAVS P2P has manual tracing but limited metrics. | Low | `commonware-runtime::tokio` implements `Metrics` trait. Broadcast engine exports metrics automatically. |
| Message priority support | `commonware-broadcast::buffered::Config::priority` and `Sender::send` priority flag allow marking messages as high-priority for preferential delivery. | Low | Could prioritize quorum-critical submissions over routine broadcasts. |
| Namespace-scoped replay protection | `discovery::Config::namespace` prefixes all signed messages to prevent replay attacks across different networks or environments. | Low | Prevents dev network messages from being replayed on prod. Currently WAVS relies on GossipSub topic naming only. |

## Anti-Features

Features to explicitly NOT build during migration.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Custom NAT traversal | Commonware does not provide AutoNAT or hole-punching. Building custom NAT traversal is a massive scope expansion and not needed for production operators who have public IPs. | Require operators to configure their `dialable` address explicitly. Document this as a deployment requirement. Most AVS operators already have public-facing infrastructure. |
| mDNS replacement for local dev | Building an mDNS-like zero-config discovery layer on top of commonware is unnecessary complexity. | Use `commonware-p2p::authenticated::lookup` with hardcoded local addresses for dev mode, or run a local bootstrapper. The dev experience changes slightly but remains functional. |
| Dynamic channel registration | Trying to work around commonware's static channel model by creating/destroying Network instances at runtime would be fragile and error-prone. | Use a single broadcast channel for all services. Filter messages by `service_id` in application code. This is simpler, works with dynamic services, and the filtering overhead is negligible. |
| Backward compatibility with libp2p peers | Supporting both libp2p and commonware peers simultaneously during migration would require maintaining two P2P stacks. | Clean break. All operators upgrade together. Coordinate via version announcement and migration window. This is already the planned approach per PROJECT.md. |
| Custom request/response protocol | Rebuilding the `CatchUpCodec` request/response pattern on top of commonware's channel model adds unnecessary complexity. | Let `commonware-broadcast::buffered` handle catch-up via its built-in message caching and digest-based retrieval. The Engine's bounded queue per peer replaces the entire custom catch-up protocol. |
| GossipSub mesh parameter tuning | Current P2P has extensive mesh_n, mesh_n_low, mesh_n_high, heartbeat, history tuning. These are GossipSub-specific concepts that do not exist in commonware. | Commonware's broadcast model is different (not mesh-based). Tune `deque_size`, `gossip_bit_vec_frequency`, and rate limiting quotas instead. Do not try to replicate GossipSub mesh semantics. |
| Multiaddr format | libp2p uses multiaddr (`/ip4/x.x.x.x/tcp/9000/p2p/12D3KooW...`). Commonware uses standard socket addresses + public keys. | Use commonware's native address format (socket address + Ed25519 public key). Update all config, docs, and status endpoints to use new format. |

## Feature Dependencies

```
Ed25519 Identity Derivation
  |
  v
Network Creation (discovery or lookup)
  |
  +---> Channel Registration (must happen before start)
  |
  v
Network Start
  |
  +---> Broadcast Engine Creation (requires network sender/receiver)
  |       |
  |       v
  |     Mailbox (for broadcast and digest-based retrieval)
  |
  +---> Oracle Peer Set Management (can happen after start)
  |
  v
P2pHandle API (publish, subscribe, unsubscribe, get_status)
  |
  v
Aggregator Integration (AggregatorCommand::Broadcast/Receive)
  |
  v
E2E Tests Pass
```

Key dependency chain:
- `Ed25519 Identity` --> `Network` --> `Broadcast Engine` --> `P2pHandle` --> `Aggregator`
- `Channel Registration` must happen before `Network Start` (static channels)
- `Oracle` peer management can happen after start (dynamic peer sets)
- `Single broadcast channel` decision eliminates the channel-per-service dependency

## MVP Recommendation

Prioritize:
1. **Ed25519 identity derivation from mnemonic** -- Foundation for everything else. Derive deterministic Ed25519 key from existing `WAVS_SIGNING_MNEMONIC`. Must be done first.
2. **Single-channel broadcast with service ID filtering** -- Replaces GossipSub + per-service topics. Use one commonware-broadcast channel, filter by `service_id` in application code. This sidesteps the static channel limitation entirely.
3. **Discovery mode with bootstrappers** -- Replaces Kademlia DHT. Configure bootstrappers in new P2P config format. This is a direct mapping.
4. **Lookup mode for local dev** -- Replaces mDNS. Hardcode local peer addresses for dev mode. Slightly different DX but functional.
5. **Buffered Engine for catch-up** -- Replaces entire custom catch-up protocol. The Engine's per-peer message cache and `Mailbox::subscribe(digest)` handle missed message retrieval automatically.
6. **P2pHandle API preservation** -- Keep `publish()`, `subscribe()`, `unsubscribe()`, `get_status()` interface stable. Internal implementation changes, external API stays the same.
7. **Oracle integration for authorized peers** -- New capability. Start simple (all configured peers authorized), plan for on-chain registry integration later.

Defer:
- **Simulated networking tests**: Nice to have but not needed for initial migration. Add after core P2P works.
- **Peer blocking**: Implement after migration is stable. Not critical for initial launch.
- **Priority message support**: Tune after observing production behavior.
- **On-chain operator registry integration with Oracle**: Phase 2 feature. Start with static peer sets.

## Sources

- [commonware-p2p crate documentation](https://docs.rs/commonware-p2p/latest/commonware_p2p/) -- HIGH confidence, official docs
- [commonware-p2p authenticated::discovery](https://docs.rs/commonware-p2p/latest/commonware_p2p/authenticated/discovery/index.html) -- HIGH confidence
- [commonware-p2p authenticated::lookup](https://docs.rs/commonware-p2p/latest/commonware_p2p/authenticated/lookup/index.html) -- HIGH confidence
- [commonware-broadcast buffered module](https://docs.rs/commonware-broadcast/latest/commonware_broadcast/buffered/index.html) -- HIGH confidence
- [commonware-cryptography](https://docs.rs/commonware-cryptography/latest/commonware_cryptography/) -- HIGH confidence
- [commonware-runtime](https://docs.rs/commonware-runtime/latest/commonware_runtime/) -- HIGH confidence
- [commonwarexyz/monorepo GitHub](https://github.com/commonwarexyz/monorepo) -- HIGH confidence
- [Commonware: the Anti-Framework blog post](https://commonware.xyz/blogs/commonware-the-anti-framework) -- MEDIUM confidence (design philosophy)
- [Your P2P demo runs locally. Now what?](https://commonware.xyz/blogs/commonware-deployer) -- MEDIUM confidence (deployment patterns)
- Current WAVS source: `packages/wavs/src/subsystems/aggregator/p2p.rs` (~1840 lines) -- HIGH confidence, direct code analysis
