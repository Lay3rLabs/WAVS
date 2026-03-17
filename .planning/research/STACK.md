# Technology Stack

**Project:** WAVS Commonware P2P Migration
**Researched:** 2026-03-17

## Recommended Stack

### Core Commonware Crates

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `commonware-p2p` | 2026.3.0 | Authenticated peer networking | Replaces libp2p 0.56. Provides encrypted connections between authenticated peers with discovery and lookup modes. The `authenticated::discovery` module maps directly to the current Kademlia+mDNS pattern. |
| `commonware-broadcast` | 2026.3.0 | Message dissemination + caching | Replaces GossipSub + the custom catch-up protocol. The `buffered::Engine` handles both broadcast and digest-based retrieval of missed messages, eliminating ~300 lines of custom catch-up code. |
| `commonware-cryptography` | 2026.3.0 | Ed25519 key generation and signing | Replaces libp2p's secp256k1 P2P identity. Ed25519 is commonware's native scheme; the `Signer` trait provides `sign(namespace, msg)` with cross-domain attack prevention. |
| `commonware-runtime` | 2026.3.0 | Async runtime abstraction layer | Required dependency of commonware-p2p. Provides the `Context` that satisfies `Spawner`, `Clock`, `Network`, `Storage`, `Metrics` traits needed by the p2p and broadcast engines. |
| `commonware-codec` | 2026.3.0 | Binary serialization | Transitive dependency. Used by commonware-p2p for message encoding. Submission types will need `commonware_codec::Encode + Decode` implementations. |

**Version scheme:** CalVer (YYYY.M.PATCH). All crates are published in lockstep from the monorepo at [github.com/commonwarexyz/monorepo](https://github.com/commonwarexyz/monorepo). Pin to `2026.3.0` -- this is the latest release as of 2026-03-09.

**ALPHA WARNING:** All commonware crates are self-described as ALPHA software. Expect breaking changes between CalVer releases. This is acceptable for WAVS because: (a) the P2P layer is already behind the `P2pHandle` abstraction, limiting blast radius; (b) WAVS itself is pre-1.0; (c) commonware is well-funded ($25M raise, Nov 2025) and actively maintained.

### Transitive Commonware Dependencies (pulled in automatically)

| Technology | Version | Purpose | Notes |
|------------|---------|---------|-------|
| `commonware-stream` | 2026.3.0 | Transport-layer message exchange | Used internally by commonware-p2p |
| `commonware-macros` | 2026.3.0 | Derive macros | Used for codec derivation |
| `commonware-parallel` | 2026.3.0 | Parallel fold operations | Used by cryptography internals |
| `commonware-utils` | 2026.3.0 | Shared utilities | Common helpers |
| `commonware-math` | 2026.3.0 | Math primitives | Used by cryptography |

### Existing WAVS Dependencies (unchanged)

| Technology | Version | Purpose | Notes |
|------------|---------|---------|-------|
| `tokio` | 1.47.1 | Async runtime | WAVS primary runtime. See runtime integration section below. |
| `crossbeam` | (workspace) | Inter-subsystem channels | Aggregator <-> Dispatcher communication unchanged |
| `serde` | (workspace) | Config serialization | `wavs.toml` parsing for new P2P config format |
| `tracing` | (workspace) | Structured logging | commonware also uses tracing internally |
| `axum` | (workspace) | HTTP API | `/p2p/status` endpoint updated but Axum stays |

### Dependencies to Remove

| Technology | Current Version | Why Remove |
|------------|----------------|------------|
| `libp2p` | 0.56 | Fully replaced by commonware-p2p + commonware-broadcast. All 13 feature flags (tokio, tcp, dns, noise, yamux, identify, ping, gossipsub, request-response, kad, mdns, autonat, secp256k1) become unnecessary. This is a significant dependency reduction. |

## Critical Architecture Decision: Runtime Integration

**Confidence: MEDIUM** -- verified from source code, but no production examples of this pattern exist.

### The Problem

`commonware-runtime::tokio::Runner` creates its own Tokio multi-thread runtime internally via `tokio::runtime::Builder::new_multi_thread()`. WAVS already has its own Tokio runtime (1.47.1 with `full` features). You cannot nest `block_on()` calls -- Tokio will panic with "Cannot start a runtime from within a runtime."

### The Solution: Dedicated Thread with Separate Runtime

Run `commonware-runtime::tokio::Runner::start()` on a dedicated OS thread via `std::thread::spawn`. The commonware runtime gets its own Tokio instance. Communication between the WAVS Tokio runtime and the commonware runtime happens over thread-safe channels (crossbeam or `tokio::sync::mpsc`).

```rust
// Pseudocode for the integration pattern
let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
let (event_tx, event_rx) = crossbeam::channel::unbounded();

let handle = std::thread::spawn(move || {
    let runner = commonware_runtime::tokio::Runner::default();
    runner.start(|context| async move {
        // Inside commonware's runtime context
        let (network, oracle) = discovery::Network::new(context.clone(), config);
        let (sender, receiver) = network.register(channel, rate_limit, backlog);
        let net_handle = network.start();

        // Bridge: read from cmd_rx, write to event_tx
        // This loop receives P2pCommands from WAVS and
        // translates them to commonware sender.send() calls
        loop {
            tokio::select! {
                Some(cmd) = cmd_rx.recv() => { /* handle command */ }
                Ok(msg) = receiver.recv() => { /* forward to event_tx */ }
            }
        }
    });
});
```

**Why this works:**
- `std::thread::spawn` creates a new OS thread with no Tokio context
- `Runner::start()` calls `Builder::new_multi_thread().build()` safely on that thread
- Channel-based bridging is the same pattern WAVS already uses (crossbeam channels between subsystems)
- The `P2pHandle` interface stays identical -- it still wraps an `mpsc::UnboundedSender<P2pCommand>`

**Why NOT alternative approaches:**
- **Cannot use `tokio::spawn`**: Runner::start() is blocking, would block a Tokio worker thread
- **Cannot use `tokio::task::spawn_blocking`**: That thread pool still has a Tokio context; nested runtime creation panics
- **Cannot construct Context directly**: `Context` and `Executor` have no public constructors; only `Runner::start()` creates them
- **Cannot implement runtime traits yourself**: Theoretically possible (commonware is trait-based), but impractical -- you'd need to implement `Spawner`, `Clock`, `Network`, `Storage`, `BufferPooler`, `ThreadPooler`, `Metrics`, `Resolver`, and `CryptoRngCore`. The commonware Tokio runner on a separate thread is vastly simpler.

### Runtime Config

```rust
let runner = commonware_runtime::tokio::Runner::new(
    commonware_runtime::tokio::Config::default()
        .with_worker_threads(2)           // Minimal: P2P doesn't need many
        .with_max_blocking_threads(4)     // For DNS resolution, etc.
        .with_tcp_nodelay(true)           // Low-latency messaging
);
```

Keep worker threads low (2-4). The commonware runtime handles only P2P networking; WAVS's main runtime handles everything else.

## Commonware P2P Architecture Mapping

### libp2p to commonware Concept Mapping

| libp2p Concept | commonware Equivalent | Notes |
|----------------|----------------------|-------|
| `Swarm` | `discovery::Network` | Main network lifecycle manager |
| `PeerId` (secp256k1) | `ed25519::PublicKey` | Peer identity type |
| `Keypair::generate_secp256k1()` | `ed25519::PrivateKey::from_seed(seed)` | Key generation. `from_seed` takes u64, so mnemonic must be hashed to u64 first, OR use `ed25519::PrivateKey::random()` with a seeded RNG. |
| `GossipSub` topic | `Channel` + `discovery::Sender/Receiver` | One channel per service for topic isolation |
| `Kademlia` DHT | `authenticated::discovery` with bootstrappers | Bootstrapper-based discovery replaces DHT |
| `mDNS` | `authenticated::lookup` OR `discovery` with local config | `lookup` works when peer addresses are known; `discovery::Config::local()` for dev-friendly settings |
| Request/Response (catch-up) | `buffered::Engine` digest retrieval | Built-in message caching eliminates custom catch-up protocol |
| `AutoNAT` + `Identify` | Not needed | commonware handles connection management internally |
| `NetworkBehaviour` derive | Not applicable | commonware uses channel registration, not behavior composition |

### Key Derivation Change

**Current (libp2p):** Mnemonic -> BIP39 seed -> secp256k1 keypair at HD path m/44'/60'/0'/0/0

**New (commonware):** Mnemonic -> SHA-256 hash of seed bytes -> first 8 bytes as u64 -> `ed25519::PrivateKey::from_seed(u64)` OR Mnemonic -> BIP39 seed -> 32 bytes -> construct ed25519 key via `commonware_codec::Read` trait

**Recommendation:** Use `ed25519::PrivateKey::random(&mut seeded_rng)` where the RNG is seeded from the BIP39 mnemonic's entropy. This gives deterministic key derivation with full 256-bit entropy, rather than truncating to u64 which loses entropy. The `from_seed(u64)` method is explicitly marked as "insecure" and for testing only.

```rust
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

let bip39_seed = /* derive from mnemonic */;
let mut rng = ChaCha20Rng::from_seed(bip39_seed[..32].try_into().unwrap());
let private_key = ed25519::PrivateKey::random(&mut rng);
```

**Confidence: HIGH** -- `random()` takes `impl CryptoRngCore`, ChaCha20Rng implements this, and seeding from mnemonic entropy is standard practice.

### Channel Registration Pattern

commonware-p2p requires all channels to be registered before `network.start()`. This is different from libp2p where GossipSub topics can be subscribed/unsubscribed dynamically.

**Impact:** Service subscribe/unsubscribe at runtime must be handled differently. Two approaches:

1. **Pre-register a single broadcast channel, multiplex services in application layer.** Register one `Channel` for all WAVS traffic, include `ServiceId` in the message envelope, filter on receive. Simple but loses network-level isolation.

2. **Pre-register a pool of channels, map services to channels dynamically.** Register N channels at startup (e.g., 256), assign services to channels via consistent hashing of ServiceId. Channels are reusable across service lifetimes.

**Recommendation:** Approach 1 (single channel with application-layer routing) for the initial migration. Rationale: it's simpler, maps well to the existing `P2pHandle` interface, and the `buffered::Engine` handles message caching per-sender regardless. Per-service isolation at the network level is a nice-to-have, not a requirement for correctness -- the aggregator already validates messages by service ID.

**Confidence: MEDIUM** -- this is an architectural choice, not a technical constraint. May revisit if message volume per-service becomes a concern.

### Oracle Peer Management

The `Oracle` struct manages authorized peer sets. Key methods:
- `oracle.track(index, peer_set)` -- register a set of authorized peers at a given index
- `oracle.subscribe()` -- get notified of peer set changes
- `oracle.block(peer)` -- block a misbehaving peer

Peers not explicitly tracked via the Oracle are rejected. This is more restrictive than libp2p's open gossip model. For WAVS, the operator set for each service is known from on-chain registration, so this maps well -- the aggregator already knows which operators are registered for which services.

## Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `rand_chacha` | 0.9+ | Deterministic RNG | Seeding Ed25519 key from mnemonic entropy |
| `sha2` | 0.10+ | SHA-256 hashing | Hashing mnemonic seed for key derivation (if needed) |
| `bip39` | (existing) | Mnemonic handling | Already in WAVS for mnemonic-based key derivation |
| `prometheus-client` | 0.24 | Metrics | commonware-runtime uses this internally; WAVS may want to expose commonware metrics via existing Prometheus endpoint |

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| P2P Framework | commonware-p2p | Keep libp2p 0.56 | Project requirement is to migrate to commonware. Also, commonware's authenticated model is a better fit for known-operator AVS networks. |
| P2P Framework | commonware-p2p | libp2p 0.57+ | Same as above; plus libp2p 0.57 has its own breaking changes |
| Broadcast | commonware-broadcast buffered | Custom over commonware-p2p raw channels | buffered::Engine provides message caching and digest retrieval for free, eliminating the need to rewrite the catch-up protocol |
| Runtime integration | Dedicated thread + separate runtime | Implement commonware traits on WAVS runtime | Implementing 10+ traits to wrap the existing Tokio runtime is high effort and fragile across commonware version bumps |
| Runtime integration | Dedicated thread + separate runtime | Single shared Tokio runtime | Not possible; Runner creates its own runtime with no public constructor for Context |
| Crypto identity | Ed25519 via commonware-cryptography | Wrap secp256k1 to satisfy commonware Signer trait | commonware's Signer trait expects associated PublicKey/Signature types; wrapping secp256k1 would fight the type system. Ed25519 is the native, tested path. |
| Discovery (dev) | `discovery::Config::local()` | `authenticated::lookup` | `lookup` requires knowing all peer addresses upfront. `Config::local()` with `allow_private_ips: true` gives automatic LAN discovery similar to mDNS. |
| Discovery (prod) | `discovery` with bootstrappers | `lookup` | `discovery` with bootstrappers is the direct replacement for Kademlia DHT. Bootstrapper nodes serve the same role as current bootstrap_nodes config. |

## Installation

```toml
# In packages/wavs/Cargo.toml (or workspace Cargo.toml)

[dependencies]
commonware-p2p = "2026.3.0"
commonware-broadcast = "2026.3.0"
commonware-cryptography = "2026.3.0"
commonware-runtime = "2026.3.0"
commonware-codec = "2026.3.0"
rand_chacha = "0.9"

# Remove:
# libp2p = { version = "0.56", features = [...] }
```

**Feature flags to consider:**
- `commonware-runtime`: no default features needed; the `tokio` module is always compiled for non-WASM targets
- `commonware-cryptography`: default `std` feature is sufficient; `mocks` feature useful for tests
- `commonware-p2p`: no optional features needed for production use; `arbitrary` for property testing

## Sources

- [commonware-p2p docs.rs](https://docs.rs/commonware-p2p/2026.3.0/commonware_p2p/) -- HIGH confidence, official API docs
- [commonware-broadcast docs.rs](https://docs.rs/commonware-broadcast/2026.3.0/commonware_broadcast/) -- HIGH confidence
- [commonware-cryptography docs.rs](https://docs.rs/commonware-cryptography/2026.3.0/commonware_cryptography/) -- HIGH confidence
- [commonware-runtime docs.rs](https://docs.rs/commonware-runtime/2026.3.0/commonware_runtime/) -- HIGH confidence
- [commonware-runtime source (tokio/runtime.rs)](https://docs.rs/crate/commonware-runtime/2026.3.0/source/src/tokio/runtime.rs) -- HIGH confidence, verified Runner creates own Tokio runtime
- [commonware GitHub monorepo](https://github.com/commonwarexyz/monorepo) -- HIGH confidence
- [commonware releases](https://github.com/commonwarexyz/monorepo/releases) -- HIGH confidence, v2026.3.0 is latest
- [commonware anti-framework philosophy](https://deepwiki.com/commonwarexyz/monorepo/1.1-anti-framework-philosophy) -- MEDIUM confidence (third-party wiki)
- [commonware-runtime blog post](https://commonware.xyz/blogs/commonware-runtime) -- HIGH confidence, official blog
- [commonware-chat example](https://docs.rs/crate/commonware-chat/latest/source/src/main.rs) -- HIGH confidence, shows usage patterns
