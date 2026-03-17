# Phase 1: Secure Peer Connectivity - Research

**Researched:** 2026-03-17
**Domain:** Commonware P2P networking (Ed25519 identity, authenticated connections, Oracle authorization)
**Confidence:** HIGH

## Summary

Phase 1 establishes the authenticated peer connection layer for WAVS using commonware-p2p. Three components are built: (1) deterministic Ed25519 identity derivation from the operator's BIP-39 mnemonic, (2) commonware-p2p networking in both lookup mode (known addresses for local dev) and discovery mode (bootstrapper-based for production), and (3) Oracle-based operator authorization that rejects unauthorized peers at the connection level. Broadcast, message routing, the full P2pHandle API, and P2pStatus updates are explicitly out of scope -- those land in Phase 2/3.

The existing `packages/wavs/src/subsystems/aggregator/p2p.rs` (approximately 1,840 lines of libp2p code) is replaced in-place. The file currently contains the `P2pConfig` enum, `P2pHandle`, `P2pCommand`, the swarm builder, and the event loop. Phase 1 replaces the identity derivation and networking foundation; the event loop and broadcast logic remain for Phase 2.

**Primary recommendation:** Start with Ed25519 key derivation (pure function, zero networking), then build the commonware Runner-on-dedicated-thread scaffold, then wire up lookup mode for localhost testing, then discovery mode. Validate the Runner integration early -- it is the highest-risk component.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Replace `packages/wavs/src/subsystems/aggregator/p2p.rs` in-place (do not create a parallel module)
- Aggregator may be non-functional mid-migration; this is acceptable -- Phase 1 lives on a branch that does not merge to `main` until Phase 2 is complete
- Delete libp2p code as each component is replaced by its commonware equivalent -- forward progress only, no keeping dead code around
- Phase 1 tests live in `packages/wavs/tests/` (Rust integration tests)
- Tests spin up the P2P connection layer in isolation -- no Dispatcher or Aggregator involved
- Implement `lookup` mode (known peer addresses) first -- simpler, no bootstrapper node needed, localhost testing is trivial
- Implement `discovery` mode (bootstrapper-based) second
- Local dev uses `lookup` mode with explicit peer addresses (not `discovery::Config::local()`)
- New config field `authorized_peers` in the P2P section of `wavs.toml`
- Format: flat array of Ed25519 hex pubkeys -- `authorized_peers = ["aabbcc...", "ddeeff..."]`
- The local node's own pubkey is implicitly trusted -- operators do not need to list themselves

### Claude's Discretion
- Whether the node's own pubkey should appear in Oracle `track()` calls (likely no -- Oracle manages other peers)
- Exact Rust module structure inside `p2p.rs` as it is being replaced (helper functions, sub-structs)
- Ed25519 key derivation specifics: use `ChaCha20Rng::from_seed(bip39_seed[..32])` + `ed25519::PrivateKey::random(&mut rng)` per STACK.md recommendation; the domain/namespace labeling is Claude's call
- Integration test harness setup (port allocation, cleanup, test timeouts)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| IDEN-01 | P2P identity derived deterministically from `WAVS_SIGNING_MNEMONIC` as Ed25519 keypair via ChaCha20Rng | Ed25519 key derivation pattern verified: `PrivateKey::random(ChaCha20Rng::from_seed(...))`. See Code Examples section. |
| IDEN-02 | Peer ID is consistent across node restarts with same mnemonic | Same as IDEN-01 -- determinism guaranteed by `ChaCha20Rng::from_seed()` with identical 32-byte seed input |
| NET-01 | Operators discover peers via commonware-p2p discovery mode with bootstrappers (production) | `discovery::Network` with `Config::recommended()` takes `bootstrappers: Vec<Bootstrapper<PublicKey>>`. See Architecture section. |
| NET-02 | Operators connect to peers via commonware-p2p lookup mode with known addresses (local dev) | `lookup::Network` with `Config::local()` + `oracle.track(0, peer_map)` with address mappings. See Architecture section. |
| NET-03 | Peer connections are encrypted and authenticated by Ed25519 identity | Built-in to commonware-p2p -- all connections are TLS-encrypted and mutually authenticated by Ed25519 identity. No configuration needed. |
| NET-04 | Node reconnects to bootstrappers automatically when peers are lost | Built-in to `discovery::Network` -- configurable `dial_frequency` and `query_frequency` handle automatic reconnection. |
| SEC-01 | Oracle-based peer set management authorizes only known operators | `Oracle.track(index, peers)` registers authorized peers. Unauthorized peers are blocked automatically. See Oracle pattern below. |
| SEC-02 | Built-in per-peer and per-subnet rate limiting active on all connections | `Config` includes `allowed_connection_rate_per_peer`, `allowed_handshake_rate_per_ip`, `allowed_handshake_rate_per_subnet`. Auto-enforced. |
| SEC-03 | Misbehaving peers can be blocked by cryptographic identity | `Oracle.block(public_key)` disconnects and prevents future connections from that identity. |
</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `commonware-p2p` | 2026.3.0 | Authenticated peer networking | Direct replacement for libp2p. Provides encrypted connections with Ed25519 identity, discovery mode (bootstrapper-based), and lookup mode (known addresses). |
| `commonware-cryptography` | 2026.3.0 | Ed25519 key generation and signing | Provides `ed25519::PrivateKey` with `random(impl CryptoRngCore)` for deterministic key derivation. Native crypto scheme for commonware-p2p. |
| `commonware-runtime` | 2026.3.0 | Async runtime abstraction | Required by commonware-p2p. Provides `tokio::Runner` and `Context` that satisfies `Spawner`, `Clock`, `Network`, `Metrics` traits. |
| `rand_chacha` | 0.9 | Deterministic CSPRNG | Seeds Ed25519 key derivation from BIP-39 mnemonic entropy. Already in dependency tree (transitive via `rand` 0.9.2). |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `bip39` | 2.2.0 (existing) | Mnemonic handling | Already in workspace. Used to derive seed bytes from `WAVS_SIGNING_MNEMONIC`. |
| `sha2` | 0.10.9 (existing) | SHA-256 hashing | Already in workspace. Optional -- for hashing mnemonic seed if domain separation is needed. |
| `const-hex` | 1.16.0 (existing) | Hex encoding | Already in workspace. For encoding Ed25519 public keys as hex strings in `authorized_peers` config and logging. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `ed25519::PrivateKey::random(&mut rng)` | `ed25519::PrivateKey::from_seed(u64)` | `from_seed` truncates to 64 bits of entropy and is marked "insecure" / testing-only. `random` with seeded ChaCha20Rng uses full 256-bit entropy. |
| Dedicated OS thread for Runner | Custom runtime trait implementation | Implementing 10+ traits (`Spawner`, `Clock`, `Network`, `Storage`, `BufferPooler`, `ThreadPooler`, `Metrics`, `Resolver`, `CryptoRngCore`) is high effort, fragile across versions. Dedicated thread is simple and proven. |
| `lookup` mode for local dev | `discovery::Config::local()` | User decision: explicit peer addresses via lookup mode preferred over discovery auto-detection. Simpler, more deterministic for testing. |

**Installation:**
```toml
# In packages/wavs/Cargo.toml [dependencies]
commonware-p2p = "2026.3.0"
commonware-cryptography = "2026.3.0"
commonware-runtime = "2026.3.0"
rand_chacha = "0.9"
# rand = "0.9.2" already in workspace
# bip39 = "2.2.0" already in workspace
# sha2 = "0.10.9" already in workspace
```

**Note:** `commonware-broadcast` and `commonware-codec` are Phase 2 concerns. Phase 1 only needs p2p, cryptography, and runtime.

**Version verification:** Versions confirmed via [crates.io](https://crates.io/crates/commonware-p2p) and [docs.rs](https://docs.rs/commonware-p2p/2026.3.0/). All commonware crates use CalVer and are published in lockstep from the monorepo. `rand_chacha` 0.9.0 already exists in Cargo.lock as a transitive dependency.

## Architecture Patterns

### Recommended Project Structure

Phase 1 modifies `p2p.rs` in-place. The internal structure within the file:

```
packages/wavs/src/subsystems/aggregator/p2p.rs
  |-- Identity section: ed25519_signer_from_mnemonic(), pubkey_from_mnemonic()
  |-- Config section: P2pConfig enum (Disabled / Local / Remote) -- modified fields
  |-- P2pHandle section: unchanged public interface (publish, subscribe, unsubscribe, get_status)
  |-- P2pCommand enum: unchanged
  |-- Network builder: build_commonware_network() replaces build_swarm()
  |-- Runner scaffold: spawn_commonware_runtime() -- dedicated thread with Runner
  |-- Oracle management: authorized_peers config -> oracle.track()
  |
packages/wavs/tests/
  |-- p2p_identity_tests.rs: deterministic key derivation tests
  |-- p2p_connectivity_tests.rs: lookup and discovery mode connection tests
```

### Pattern 1: Deterministic Ed25519 Key Derivation

**What:** Derive a deterministic Ed25519 keypair from the operator's BIP-39 mnemonic.
**When to use:** Every time the P2P network is initialized (replaces `keypair_from_mnemonic()`).

```rust
use commonware_cryptography::ed25519;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

fn ed25519_signer_from_mnemonic(mnemonic: &str) -> Result<ed25519::PrivateKey, AggregatorError> {
    let mnemonic = bip39::Mnemonic::parse(mnemonic)
        .map_err(|e| AggregatorError::P2p(format!("Invalid mnemonic: {}", e)))?;

    // BIP-39 seed (64 bytes); take first 32 for ChaCha20Rng seed
    let seed = mnemonic.to_seed("");
    let rng_seed: [u8; 32] = seed[..32].try_into()
        .map_err(|_| AggregatorError::P2p("Seed too short".into()))?;

    let mut rng = ChaCha20Rng::from_seed(rng_seed);
    Ok(ed25519::PrivateKey::random(&mut rng))
}
```

**Confidence:** HIGH -- `PrivateKey::random()` accepts `impl CryptoRngCore`; `ChaCha20Rng` implements `CryptoRng + RngCore` which satisfies `CryptoRngCore`. Seeding from BIP-39 entropy is deterministic.

### Pattern 2: Commonware Runner on Dedicated OS Thread

**What:** Run `commonware_runtime::tokio::Runner` on a separate OS thread to avoid Tokio runtime nesting panics.
**When to use:** When initializing the P2P network (replaces `tokio::spawn(run_event_loop(...))`).

```rust
use commonware_runtime::tokio::{Config as RuntimeConfig, Runner};

fn spawn_commonware_runtime(
    private_key: ed25519::PrivateKey,
    p2p_config: P2pConfig,
    authorized_peers: Vec<ed25519::PublicKey>,
    command_rx: mpsc::UnboundedReceiver<P2pCommand>,
    aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let runner = Runner::new(
            RuntimeConfig::default()
                .with_worker_threads(2)
                .with_max_blocking_threads(4)
                .with_tcp_nodelay(Some(true))
        );
        runner.start(|context| async move {
            // Create network (discovery or lookup based on config)
            let (mut network, mut oracle) = create_network(
                context.clone(), &private_key, &p2p_config
            );

            // Register authorized peers
            register_authorized_peers(&mut oracle, &authorized_peers).await;

            // Register a single channel for future broadcast use (Phase 2)
            let (_sender, _receiver) = network.register(
                0u32, // channel ID
                governor::Quota::per_second(std::num::NonZeroU32::new(100).unwrap()),
                1024, // backlog
            );

            // Start the network
            let _net_handle = network.start();

            // Bridge loop: handle P2pCommands from WAVS main runtime
            // Phase 1 only needs to keep the network running; full command
            // bridging (publish, subscribe) is Phase 2
            loop {
                tokio::select! {
                    cmd = command_rx.recv() => {
                        match cmd {
                            Some(P2pCommand::GetStatus { response_tx }) => {
                                // Return basic status
                                let _ = response_tx.send(/* status */);
                            }
                            None => break, // channel closed, shutdown
                            _ => {} // Other commands handled in Phase 2
                        }
                    }
                }
            }
        });
    })
}
```

**Confidence:** MEDIUM -- pattern is sound (dedicated thread avoids nested runtime panic), but has not been validated in WAVS's specific context yet. This should be the first thing prototyped.

### Pattern 3: Oracle Peer Authorization

**What:** Configure the Oracle to accept only authorized operator peers.
**When to use:** During network initialization, after creating the Network.

For **discovery** mode (Oracle tracks `Set<PublicKey>`):
```rust
use commonware_p2p::authenticated::discovery;

async fn register_authorized_peers_discovery(
    oracle: &mut discovery::Oracle<ed25519::PublicKey>,
    authorized_peers: &[ed25519::PublicKey],
    own_pubkey: &ed25519::PublicKey,
) {
    // Build the peer set (include self + authorized peers)
    let mut peers = commonware_utils::Set::new();
    peers.insert(own_pubkey.clone());
    for peer in authorized_peers {
        peers.insert(peer.clone());
    }
    oracle.track(0, peers).await;
}
```

For **lookup** mode (Oracle tracks `Map<PublicKey, Address>`):
```rust
use commonware_p2p::authenticated::lookup;
use commonware_p2p::Address;

async fn register_authorized_peers_lookup(
    oracle: &mut lookup::Oracle<ed25519::PublicKey>,
    peer_addresses: &[(ed25519::PublicKey, std::net::SocketAddr)],
) {
    let mut peers = commonware_utils::Map::new();
    for (pubkey, addr) in peer_addresses {
        peers.insert(pubkey.clone(), Address::Network(addr.clone()));
    }
    oracle.track(0, peers).await;
}
```

**Key difference:** Discovery Oracle takes `Set<PublicKey>` (addresses discovered dynamically). Lookup Oracle takes `Map<PublicKey, Address>` (addresses must be known upfront).

**Confidence:** HIGH -- verified from docs.rs API signatures.

### Pattern 4: Discovery vs Lookup Network Creation

```rust
// Discovery mode (production -- bootstrapper-based)
fn create_discovery_network(
    context: impl Spawner + BufferPooler + Clock + CryptoRngCore + /*...*/,
    private_key: &ed25519::PrivateKey,
    listen_addr: std::net::SocketAddr,
    dialable_addr: impl Into<Ingress>,
    bootstrappers: Vec<(ed25519::PublicKey, Ingress)>,
    max_message_size: u32,
) -> (discovery::Network</*...*/>, discovery::Oracle<ed25519::PublicKey>) {
    let config = discovery::Config::recommended(
        private_key.clone(),
        b"wavs-p2p",  // namespace for replay protection
        listen_addr,
        dialable_addr,
        bootstrappers,
        max_message_size,
    );
    discovery::Network::new(context, config)
}

// Lookup mode (local dev -- known addresses)
fn create_lookup_network(
    context: impl Spawner + BufferPooler + Clock + CryptoRngCore + /*...*/,
    private_key: &ed25519::PrivateKey,
    listen_addr: std::net::SocketAddr,
    max_message_size: u32,
) -> (lookup::Network</*...*/>, lookup::Oracle<ed25519::PublicKey>) {
    let config = lookup::Config::local(
        private_key.clone(),
        b"wavs-p2p",
        listen_addr,
        max_message_size,
    );
    lookup::Network::new(context, config)
}
```

**Confidence:** HIGH -- factory method signatures verified from docs.rs.

### Anti-Patterns to Avoid

- **Implementing custom runtime traits:** Do not try to implement `Spawner`, `Clock`, `Network`, etc. for WAVS's existing Tokio runtime. Use the dedicated-thread approach instead.
- **Exposing commonware types outside p2p.rs:** Keep `ed25519::PublicKey`, `Sender`, `Receiver`, `Oracle` types inside the P2P module boundary. The rest of WAVS sees only `P2pHandle` and `P2pConfig`.
- **Using `ed25519::PrivateKey::from_seed(u64)`:** This is explicitly for testing. Use `random()` with a seeded CSPRNG for production key derivation.
- **Calling `tokio::task::spawn_blocking` for the Runner:** The blocking thread pool still has Tokio context; `Runner::start()` creating a second runtime will panic. Use `std::thread::spawn`.
- **Registering channels after `network.start()`:** All channels must be registered before start. Register at least one channel (for Phase 2 broadcast) during Phase 1 initialization.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Peer authentication | Custom TLS/noise handshake | commonware-p2p authenticated module | Built-in mutual Ed25519 authentication with encryption |
| Peer authorization | Custom allow-list checking | `Oracle.track()` + automatic blocking | Peers not in Oracle are automatically rejected |
| Rate limiting | Per-connection rate limiters | `Config` rate fields + channel `Quota` | Per-peer, per-IP, per-subnet rate limiting built into Config |
| Peer blocking | Manual connection tracking + deny list | `Oracle.block(public_key)` | Disconnects and prevents future connections atomically |
| NAT detection | Custom STUN/AutoNAT | `Ingress` address in Config | Operators configure their dialable address explicitly |
| Peer discovery (production) | Custom DHT or gossip discovery | `discovery::Network` with bootstrappers | Built-in bit-vector-based peer exchange with configurable gossip |

**Key insight:** commonware-p2p handles authentication, encryption, rate limiting, and peer management internally. Phase 1's job is configuration and integration, not reimplementing network security primitives.

## Common Pitfalls

### Pitfall 1: Runtime Nesting Panic (CRITICAL)
**What goes wrong:** Calling `commonware_runtime::tokio::Runner::start()` from within WAVS's existing Tokio runtime causes "Cannot start a runtime from within a runtime" panic.
**Why it happens:** `Runner::start()` creates its own `tokio::runtime::Builder::new_multi_thread()` internally. Tokio does not allow nested `block_on()` calls.
**How to avoid:** Always use `std::thread::spawn` to create a fresh OS thread with no Tokio context. The Runner gets its own isolated Tokio runtime on that thread.
**Warning signs:** Panic at startup with "cannot start a runtime from within a runtime" or "no reactor is running".

### Pitfall 2: Channel Registration After Start
**What goes wrong:** Attempting to call `network.register()` after `network.start()` fails silently or panics.
**Why it happens:** commonware-p2p requires all channels to be registered before the network starts. `start()` consumes `self` (takes ownership), preventing further registration.
**How to avoid:** Register at least one channel during Phase 1 initialization (for Phase 2 broadcast use). The channel ID, quota, and backlog can be configured but must exist before start.
**Warning signs:** Compile error (cannot call method on moved value) or runtime error when trying to register channels post-start.

### Pitfall 3: Lookup vs Discovery Oracle API Mismatch
**What goes wrong:** Using the wrong `track()` signature for the network mode. Discovery Oracle takes `Set<PublicKey>`, lookup Oracle takes `Map<PublicKey, Address>`.
**Why it happens:** Both are called `Oracle` and both have a `track()` method, but they implement different traits (`Manager` vs `AddressableManager`).
**How to avoid:** The `P2pConfig` enum variant determines which network type is created. Match the Oracle API to the network type. Lookup requires addresses; discovery does not.
**Warning signs:** Compile error about mismatched types in `track()` call.

### Pitfall 4: Forgetting the Node's Own Key in the Oracle
**What goes wrong:** The node does not add its own public key to the Oracle's tracked peer set, causing self-connection issues or other peers being unable to verify it.
**Why it happens:** The `authorized_peers` config is for *other* operators. The node's own key needs separate handling.
**How to avoid:** Always include the local node's public key in the Oracle's peer set. The user decision says "the local node's own pubkey is implicitly trusted" -- this means code must add it automatically.
**Warning signs:** Node logs "unknown peer" errors about its own identity, or other nodes cannot connect because they don't see it as authorized.

### Pitfall 5: BIP-39 Seed vs Entropy Confusion
**What goes wrong:** Using `mnemonic.to_entropy()` (16-32 bytes depending on word count) instead of `mnemonic.to_seed("")` (64 bytes), producing different key material.
**Why it happens:** BIP-39 has two outputs: entropy (raw bits) and seed (PBKDF2 stretched). They produce different byte sequences.
**How to avoid:** Use `mnemonic.to_seed("")` consistently (empty passphrase, matching current EVM derivation convention). Take the first 32 bytes for ChaCha20Rng seed.
**Warning signs:** Key derivation produces different results than expected; determinism test fails.

## Code Examples

### Ed25519 Key Derivation (IDEN-01, IDEN-02)
```rust
// Source: STACK.md recommendation + docs.rs/commonware-cryptography
use commonware_cryptography::ed25519;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

/// Derive a deterministic Ed25519 identity from a BIP-39 mnemonic.
/// Replaces keypair_from_mnemonic() which derived secp256k1 at HD path m/44'/60'/0'/0/0.
fn ed25519_signer_from_mnemonic(mnemonic: &str) -> Result<ed25519::PrivateKey, AggregatorError> {
    let mnemonic = bip39::Mnemonic::parse(mnemonic)
        .map_err(|e| AggregatorError::P2p(format!("Invalid mnemonic: {}", e)))?;

    // BIP-39 seed (64 bytes, PBKDF2-stretched, empty passphrase)
    let seed = mnemonic.to_seed("");

    // Use first 32 bytes as ChaCha20Rng seed (full 256-bit entropy)
    let rng_seed: [u8; 32] = seed[..32].try_into().unwrap();
    let mut rng = ChaCha20Rng::from_seed(rng_seed);

    Ok(ed25519::PrivateKey::random(&mut rng))
}

/// Get the P2P public key that would be derived from a given mnemonic.
fn pubkey_from_mnemonic(mnemonic: &str) -> Result<ed25519::PublicKey, AggregatorError> {
    let private_key = ed25519_signer_from_mnemonic(mnemonic)?;
    Ok(private_key.public_key())
}
```

### Runner on Dedicated Thread (NET-01, NET-02)
```rust
// Source: STACK.md runtime integration + docs.rs/commonware-runtime
use commonware_runtime::tokio::{Config as RuntimeConfig, Runner};

let join_handle = std::thread::spawn(move || {
    let runner = Runner::new(
        RuntimeConfig::default()
            .with_worker_threads(2)
            .with_max_blocking_threads(4)
            .with_tcp_nodelay(Some(true))
    );
    runner.start(|context| async move {
        // All commonware operations happen inside this closure
        // context satisfies Spawner + Clock + Network + etc.

        // ... create network, oracle, register channels, start ...
    });
});
```

### Oracle Peer Authorization (SEC-01, SEC-03)
```rust
// Source: docs.rs/commonware-p2p Oracle API
// Discovery mode:
let mut peer_set = Set::new();
peer_set.insert(own_pubkey.clone());
for hex_key in &config.authorized_peers {
    let pubkey = parse_hex_pubkey(hex_key)?;
    peer_set.insert(pubkey);
}
oracle.track(0, peer_set).await;

// To block a misbehaving peer (SEC-03):
oracle.block(misbehaving_pubkey).await;
```

### Config Parsing for authorized_peers
```rust
// New field in P2pConfig variants
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum P2pConfig {
    #[default]
    Disabled,
    Local {
        listen_port: u16,
        /// Known peer addresses for lookup mode: ["pubkey@host:port", ...]
        peer_addresses: Vec<String>,
        /// Authorized peer Ed25519 public keys (hex-encoded)
        #[serde(default)]
        authorized_peers: Vec<String>,
    },
    Remote {
        listen_port: u16,
        /// Bootstrapper addresses: ["pubkey@host:port", ...]
        bootstrappers: Vec<String>,
        /// Authorized peer Ed25519 public keys (hex-encoded)
        #[serde(default)]
        authorized_peers: Vec<String>,
    },
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| libp2p secp256k1 P2P identity | Ed25519 via commonware-cryptography | This migration | Peer IDs change for all operators. Breaking change requiring coordinated upgrade. |
| Kademlia DHT discovery | Bootstrapper-based discovery | This migration | Bootstrap nodes use `Bootstrapper<PublicKey>` tuple instead of multiaddr. Config format changes. |
| mDNS local discovery | Lookup mode with known addresses | This migration (user decision) | Explicit peer addresses replace automatic LAN discovery. More deterministic, less magical. |
| GossipSub open mesh | Oracle-authorized fixed peer set | This migration | Only explicitly authorized peers can connect. More restrictive but better security for known-operator networks. |
| Tokio spawn for P2P event loop | Dedicated OS thread for commonware Runner | This migration | Additional thread; cross-runtime channel bridging pattern. |

**Deprecated/outdated:**
- **`keypair_from_mnemonic()`**: Replaced by `ed25519_signer_from_mnemonic()`. HD path derivation no longer needed.
- **`peer_id_from_mnemonic()`**: Replaced by `pubkey_from_mnemonic()`. Returns Ed25519 public key instead of libp2p PeerId.
- **libp2p `SwarmBuilder`**: Replaced by `discovery::Network::new()` / `lookup::Network::new()`.
- **`WavsBehaviour` derive macro**: commonware has no behavior composition; channel registration replaces it.

## Open Questions

1. **Oracle own-key inclusion**
   - What we know: The Oracle blocks peers not explicitly tracked. The user says own pubkey is "implicitly trusted."
   - What is unclear: Whether commonware-p2p automatically authorizes the local node, or if the local node's pubkey must be in the Oracle's tracked set.
   - Recommendation: Include own pubkey in the Oracle's tracked set to be safe. If it turns out to be unnecessary, it is a no-op. Test by omitting it and observing behavior.

2. **Ingress type for `dialable` parameter**
   - What we know: `Ingress` is an enum in `commonware_p2p::types`. Discovery `Config::recommended()` takes `impl Into<Ingress>` for dialable address.
   - What is unclear: Exact variants of `Ingress`. Likely `SocketAddr`-based with possible DNS support (Config has `allow_dns` field).
   - Recommendation: Start with `SocketAddr` for Phase 1 (localhost testing). Investigate DNS-based Ingress if needed for production bootstrappers.

3. **Channel registration in Phase 1**
   - What we know: Channels must be registered before `network.start()`. Phase 1 does not need broadcast, but Phase 2 will.
   - What is unclear: Whether a network with zero registered channels can start and maintain connections.
   - Recommendation: Register one channel (ID 0, generous quota) during Phase 1. The sender/receiver can be held but unused until Phase 2 wires broadcast.

4. **Thread lifecycle and shutdown**
   - What we know: `std::thread::spawn` returns a `JoinHandle`. `Runner::start()` blocks until the provided future completes.
   - What is unclear: How to signal the commonware runtime to shut down cleanly when WAVS receives a kill signal.
   - Recommendation: Drop the `command_tx` (sender side of mpsc channel) to signal the bridge loop to break, which completes the future, which unblocks `Runner::start()`. Store the `JoinHandle` for clean join on shutdown.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework + `cargo test` |
| Config file | None -- uses `#[cfg(feature = "dev")]` gate per existing pattern |
| Quick run command | `cargo test -p wavs --features dev --test p2p_identity_tests -- --nocapture` |
| Full suite command | `cargo test -p wavs --features dev -- p2p_ --nocapture` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| IDEN-01 | Ed25519 keypair derived deterministically from mnemonic | unit | `cargo test -p wavs --features dev --test p2p_identity_tests::test_deterministic_derivation` | No -- Wave 0 |
| IDEN-02 | Same mnemonic produces same peer ID across invocations | unit | `cargo test -p wavs --features dev --test p2p_identity_tests::test_consistent_across_restarts` | No -- Wave 0 |
| NET-01 | Two nodes discover via bootstrappers | integration | `cargo test -p wavs --features dev --test p2p_connectivity_tests::test_discovery_mode` | No -- Wave 0 |
| NET-02 | Two nodes connect via lookup mode on localhost | integration | `cargo test -p wavs --features dev --test p2p_connectivity_tests::test_lookup_mode` | No -- Wave 0 |
| NET-03 | Connections encrypted and authenticated | integration | Verified implicitly by NET-01/NET-02 (commonware-p2p does not support unencrypted connections) | N/A |
| NET-04 | Auto-reconnect to bootstrappers | integration | `cargo test -p wavs --features dev --test p2p_connectivity_tests::test_reconnect` | No -- Wave 0 |
| SEC-01 | Oracle rejects unauthorized peers | integration | `cargo test -p wavs --features dev --test p2p_connectivity_tests::test_unauthorized_rejected` | No -- Wave 0 |
| SEC-02 | Rate limiting active | manual-only | Rate limiting is configured via `Config` fields; testing requires sustained high-rate connections. Verify via config inspection. | N/A |
| SEC-03 | Block peer by identity | integration | `cargo test -p wavs --features dev --test p2p_connectivity_tests::test_block_peer` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p wavs --features dev -- p2p_ --nocapture`
- **Per wave merge:** `cargo test -p wavs --features dev -- --nocapture`
- **Phase gate:** All p2p_ tests green before proceeding to Phase 2

### Wave 0 Gaps
- [ ] `packages/wavs/tests/p2p_identity_tests.rs` -- covers IDEN-01, IDEN-02
- [ ] `packages/wavs/tests/p2p_connectivity_tests.rs` -- covers NET-01, NET-02, NET-04, SEC-01, SEC-03
- [ ] Test helper: port allocator utility (avoid conflicts with existing `DEFAULT_P2P_BASE_PORT = 9000`)
- [ ] Test helper: mnemonic fixtures (two known mnemonics for two-node tests)
- [ ] Commonware dependency added: `cargo.toml` updated with commonware crates

## Sources

### Primary (HIGH confidence)
- [commonware-p2p 2026.3.0 docs.rs](https://docs.rs/commonware-p2p/2026.3.0/commonware_p2p/) -- Network, Config, Oracle, Sender, Receiver APIs
- [commonware-cryptography docs.rs](https://docs.rs/commonware-cryptography/latest/commonware_cryptography/) -- ed25519::PrivateKey API (random, from_seed, sign, public_key)
- [commonware-runtime tokio docs.rs](https://docs.rs/commonware-runtime/latest/commonware_runtime/tokio/) -- Runner, Config, Context APIs
- [commonware-p2p authenticated::discovery](https://docs.rs/commonware-p2p/latest/commonware_p2p/authenticated/discovery/) -- Network::new, Config::recommended, Config::local, Oracle::track
- [commonware-p2p authenticated::lookup](https://docs.rs/commonware-p2p/latest/commonware_p2p/authenticated/lookup/) -- Network::new, Config::local, Oracle::track with addresses
- [commonware-p2p crates.io](https://crates.io/crates/commonware-p2p) -- version 2026.3.0 confirmed
- WAVS source code: `p2p.rs`, `aggregator.rs`, `dispatcher.rs`, `config.rs`, `http.rs` -- direct code inspection

### Secondary (MEDIUM confidence)
- [commonware-chat example](https://docs.rs/crate/commonware-chat/latest/source/src/main.rs) -- usage pattern for key creation, discovery config, oracle tracking, channel registration
- `.planning/research/STACK.md` -- Ed25519 derivation pattern, runtime integration pseudocode
- `.planning/research/ARCHITECTURE.md` -- Component boundaries, build order, data flow diagrams
- `.planning/research/PITFALLS.md` -- Runtime ownership conflict (Pitfall 2), identity scheme change (Pitfall 4)

### Tertiary (LOW confidence)
- Ingress enum variants -- not fully documented on docs.rs; inferred to be SocketAddr-based from Config field types and `allow_dns` flag
- Channel behavior with zero registrations -- not documented; recommendation to register one channel is precautionary

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all crate versions verified on crates.io/docs.rs, API signatures confirmed
- Architecture: HIGH for identity derivation, MEDIUM for Runner integration (needs prototype validation)
- Pitfalls: HIGH -- runtime nesting panic well-documented in both commonware docs and Tokio docs

**Research date:** 2026-03-17
**Valid until:** 2026-04-17 (commonware is CalVer monthly; next release could change APIs)
